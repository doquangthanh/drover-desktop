//! The app's root view: daemon client + state store + one emulator per attached pane; selected space and tab.
//! Keys and IME text go to the focused pane (terminal_input.rs); panes are sized to their frame (layout_view.rs).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use alacritty_terminal::grid::Dimensions;
use drover_core::screen::PaneScreen;
use drover_proto::client::{self, Client, Command, Incoming};
use drover_proto::store::Store;
use gpui::{Bounds, Context, FocusHandle, Pixels, Window, div, prelude::*, rgb};

use crate::terminal_paint::TermFont;
use crate::{layout_view, sidebar, theme, topbar};

pub type Screens = Arc<Mutex<HashMap<String, PaneScreen>>>;
const HISTORY: usize = 5000;

pub struct DroverApp {
    pub client: Client,
    pub store: Store,
    pub screens: Screens,
    pub space: Option<String>,
    pub tab: Option<String>,
    pub connected: bool,
    pub status: String,
    /// Terminal font: the system terminal's, as the daemon reports it (`terminal_font` in state), like the web UI.
    pub term_font: TermFont,
    /// DROVER_NATIVE_TRACE=1: when the last space/tab switch was clicked, logged once it is painted.
    pub switched_at: Arc<Mutex<Option<std::time::Instant>>>,
    last_switch: Option<std::time::Instant>,
    /// The pane keys go to, and the focus handle of the terminal area.
    pub focused: Option<String>,
    pub term_focus: FocusHandle,
    /// Text the input method is still composing (drawn at the cursor, not sent yet).
    pub marked: Option<String>,
    /// Where the focused pane's cursor was painted: the IME puts its candidate window there.
    pub cursor_bounds: Arc<Mutex<Option<Bounds<Pixels>>>>,
    /// Last cols x rows this window asked the daemon for, per pane. Cleared to take the size back.
    pub sizes: Arc<Mutex<HashMap<String, (u16, u16)>>>,
    /// Replies to act on (e.g. focus the pane a button just created), and a pane to show once the store has it.
    pub pending: Vec<crate::actions::Pending>,
    pub show_when_known: Option<String>,
    pub confirm: Option<crate::actions::Confirm>,
    pub sidebar_open: bool,
    /// Where each pane's terminal grid was painted (mouse position -> cell), the pane a drag is selecting in,
    /// and the link under the mouse.
    pub term_bounds: Arc<Mutex<HashMap<String, Bounds<Pixels>>>>,
    pub selecting: Option<String>,
    pub hover_link: Option<crate::terminal_selection::HoverLink>,
}

impl DroverApp {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let client = client::start(|| {});
        // The client thread fills a channel; drain it about 60 times a second, repaint only on change.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(16)).await;
                let Ok(changed) = this.update(cx, |app, _| app.drain()) else { break };
                if changed {
                    let _ = this.update(cx, |_, cx| cx.notify());
                }
            }
        })
        .detach();
        let family = std::env::var("DROVER_NATIVE_FONT").unwrap_or_else(|_| theme::DEFAULT_TERMINAL_FONT.into());
        DroverApp { client, store: Store::default(), screens: Arc::default(), space: None, tab: None, connected: false,
                    status: "connecting…".into(), term_font: TermFont::new(&family, 13.0), switched_at: Arc::default(), last_switch: None,
                    focused: None, term_focus: cx.focus_handle(), marked: None, cursor_bounds: Arc::default(),
                    sizes: Arc::default(), pending: vec![], show_when_known: None, confirm: None, sidebar_open: true,
                    term_bounds: Arc::default(), selecting: None, hover_link: None }
    }

    fn drain(&mut self) -> bool {
        let mut changed = false;
        while let Ok(msg) = self.client.incoming.try_recv() {
            changed = true;
            match msg {
                Incoming::Connected => { self.connected = true; self.status = "drover-server up".into(); }
                Incoming::Disconnected(why) => { self.connected = false; self.status = format!("reconnecting… ({why})"); }
                Incoming::Event { name, data } => {
                    if matches!(name.as_str(), "state" | "resync") {
                        self.follow_terminal_font(&data["terminal_font"]);
                    }
                    self.store.apply(&name, &data);
                    self.fit_screens();
                }
                Incoming::Output { pane, data, replay } => self.feed(&pane, &data, replay),
            }
        }
        changed |= self.take_replies();
        if let Some(id) = self.show_when_known.clone().filter(|id| self.store.pane(id).is_some()) {
            self.show_when_known = None;
            self.show_pane(&id);
        }
        if changed {
            self.keep_selection_valid();
        }
        changed
    }

    fn feed(&mut self, pane: &str, data: &str, replay: bool) {
        let Some(p) = self.store.pane(pane) else { return };
        let mut screens = self.screens.lock().unwrap();
        if replay || !screens.contains_key(pane) {
            screens.insert(pane.to_string(), PaneScreen::new(p.cols.max(2) as usize, p.rows.max(2) as usize, HISTORY));
        }
        if let Some(s) = screens.get_mut(pane) {
            s.feed(data.as_bytes());
        }
        if replay && let Some(t) = self.last_switch {
            eprintln!("trace: replay of {pane} ({} KB) fed {:.1} ms after click", data.len() / 1024, t.elapsed().as_secs_f64() * 1000.0);
        }
    }

    /// The daemon owns each pane's size (shared by all viewers): follow it.
    fn fit_screens(&mut self) {
        let mut screens = self.screens.lock().unwrap();
        for p in &self.store.panes {
            if let Some(s) = screens.get_mut(&p.id) {
                let g = s.term().grid();
                if (g.columns(), g.screen_lines()) != (p.cols as usize, p.rows as usize) && p.cols > 1 && p.rows > 1 {
                    s.resize(p.cols as usize, p.rows as usize);
                }
            }
        }
    }

    /// {family, size} from the daemon (0 / "" = unknown: keep ours). A new font re-sizes every pane.
    fn follow_terminal_font(&mut self, v: &serde_json::Value) {
        let family = v["family"].as_str().filter(|f| !f.is_empty()).map(String::from)
            .unwrap_or_else(|| self.term_font.font.family.to_string());
        let size = v["size"].as_f64().filter(|s| *s > 0.0).map(|s| s as f32).unwrap_or(self.term_font.size);
        if family != self.term_font.font.family.as_ref() || size != self.term_font.size {
            self.term_font = TermFont::new(&family, size);
            self.sizes.lock().unwrap().clear();
        }
    }

    fn keep_selection_valid(&mut self) {
        let space_ok = self.space.as_ref().is_some_and(|s| self.store.spaces.iter().any(|x| &x.id == s));
        if !space_ok {
            self.space = self.store.spaces.first().map(|s| s.id.clone());
            self.tab = None;
        }
        let tab_ok = self.tab.as_ref().is_some_and(|t| self.store.tab(t).is_some_and(|t| Some(&t.space_id) == self.space.as_ref()));
        if !tab_ok {
            self.tab = self.space.as_ref().and_then(|s| self.store.tabs_of(s).next()).map(|t| t.id.clone());
        }
        let in_tab = |p: &String| self.store.pane(p).is_some_and(|p| Some(&p.tab_id) == self.tab.as_ref());
        if !self.focused.as_ref().is_some_and(in_tab) {
            self.focused = self.tab.as_ref().and_then(|t| self.store.tab(t)).and_then(|t| t.focused.clone());
            self.sizes.lock().unwrap().clear(); // a new tab on screen: size its panes for this window again
        }
        self.attach_visible();
    }

    /// Attach the shown tab's panes. Panes stay attached afterwards, so coming back is instant.
    fn attach_visible(&mut self) {
        let Some(view) = self.tab.as_ref().and_then(|t| self.store.tab_view(t)) else { return };
        let ids = drover_core::layout_tree::pane_ids(Some(&view));
        let screens = self.screens.lock().unwrap();
        for id in ids.into_iter().filter(|id| !screens.contains_key(id)) {
            let _ = self.client.commands.send(Command::Attach(id));
        }
    }

    fn mark_switch(&mut self) {
        if std::env::var_os("DROVER_NATIVE_TRACE").is_some() {
            let now = std::time::Instant::now();
            *self.switched_at.lock().unwrap() = Some(now);
            self.last_switch = Some(now);
        }
    }

    pub fn select_space(&mut self, id: String, cx: &mut Context<Self>) {
        self.mark_switch();
        self.space = Some(id);
        self.tab = None;
        self.keep_selection_valid();
        cx.notify();
    }

    pub fn select_tab(&mut self, id: String, cx: &mut Context<Self>) {
        self.mark_switch();
        self.tab = Some(id);
        self.keep_selection_valid();
        cx.notify();
    }

    /// Keys go to `pane` from now on. Like Herdr, looking at a pane takes its size back for this window.
    pub fn focus_pane(&mut self, pane: String, window: &mut Window, cx: &mut Context<Self>) {
        self.sizes.lock().unwrap().remove(&pane);
        if self.focused.as_deref() != Some(pane.as_str()) {
            self.rpc("pane.focus", serde_json::json!({"pane": pane}));
        }
        self.focused = Some(pane);
        window.focus(&self.term_focus);
        cx.notify();
    }
}

impl Render for DroverApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let main = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .track_focus(&self.term_focus)
            .on_key_down(cx.listener(Self::on_key))
            .child(topbar::tab_bar(self, cx))
            .child(layout_view::tab_body(self, cx));
        let overlay = crate::dialogs::confirm_overlay(self, cx);
        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(theme::CANVAS))
            .text_color(rgb(theme::FG))
            .font_family(theme::UI_FONT)
            .text_size(gpui::px(13.))
            .child(div().flex().flex_row().flex_1().min_h_0()
                .when(self.sidebar_open, |d| d.child(sidebar::sidebar(self, cx))).child(main))
            .child(topbar::status_bar(self, cx))
            .children(overlay)
    }
}
