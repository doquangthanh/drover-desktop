//! The app's root view: daemon client + state store + one emulator per attached pane; selected space and tab.
//! Keys and IME text go to the focused pane (terminal_input.rs); panes are sized to their frame (layout_view.rs).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use alacritty_terminal::grid::Dimensions;
use drover_core::screen::PaneScreen;
use drover_proto::client::{self, Client, Command, Incoming};
use drover_proto::store::Store;
use gpui::{Bounds, Context, Entity, FocusHandle, Pixels, Window, div, prelude::*, rgb};

use crate::terminal_paint::TermFont;
use crate::views::{View, text_input::TextInput};
use crate::{layout_view, sidebar, theme, topbar, views};

pub type Screens = Arc<Mutex<HashMap<String, PaneScreen>>>;
const HISTORY: usize = 5000;

/// Which kind of thing was picked last; F2 renames it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Area { Pane, Tab, Space }

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
    /// When this window last sent pane.resize, per pane: until the daemon reports that size, events it sent
    /// before handling the resize still carry the old one, and following them would shrink the screen.
    pub resize_sent: Arc<Mutex<HashMap<String, std::time::Instant>>>,
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
    /// What the main area shows: the terminals or one of the views (views/mod.rs).
    pub view: View,
    pub agents_filter: Entity<TextInput>,
    pub agents_send: Entity<TextInput>,
    /// The filter box of the shortcuts sheet.
    pub shortcuts_filter: Entity<TextInput>,
    /// Finished-agent banners the user closed: pane id -> the `since` of that "done" (review_banner.rs).
    pub banner_dismissed: HashMap<String, i64>,
    pub agents: crate::views::agents::AgentsState,
    pub changes: crate::views::changes::ChangesState,
    pub changes_in: crate::views::changes::ChangesInputs,
    pub files: crate::views::files::FilesState,
    pub inbox_filter: crate::views::inbox::Filter,
    pub settings: crate::views::settings::SettingsState,
    pub machines_form: crate::views::machines::MachinesForm,
    pub rules: crate::views::settings_rules::RulesState,
    pub whats_new: crate::views::whats_new::WhatsNewState,
    pub feedback: crate::views::feedback::FeedbackForm,
    /// The font code is drawn with in the views (diffs, files): the terminal's, as `terminal_paint::resolved` picks it.
    pub code_family: gpui::SharedString,
    /// The right panel beside the terminals (Agents, Files or Changes of the focused pane), as in Herdr.
    /// Which panel (Agents, Files or Changes) is open beside each pane's terminal: per pane, so another pane,
    /// agent or space shows none until you open it there.
    pub panels: HashMap<String, View>,
    pub panel_width: f32,
    pub sidebar_width: f32,
    pub drag: Option<crate::drag::Drag>,
    pub split_bounds: crate::drag::SplitBounds,
    /// Ctrl+K was pressed: the next key finishes the chord (keys.rs).
    pub chord: bool,
    pub show_shortcuts: bool,
    pub picker: Option<crate::picker::Picker>,
    /// The pane shown in the peek pop-up (peek.rs): looked at, not focused.
    pub peek: Option<String>,
    /// Give the keys back to the terminals on the next frame (a tab or pane was shown, a pop-up closed). Only then:
    /// a field in a panel keeps its focus while you type.
    pub want_term_focus: bool,
    /// What was picked last (a space in the sidebar, a tab, a pane): F2 renames that.
    pub area: Area,
    /// The right-click menu that is open (context_menu.rs).
    pub menu: Option<crate::context_menu::Menu>,
    /// Keyboard selection over the focused pane (copy_mode.rs).
    pub copy_mode: Option<crate::copy_mode::CopyMode>,
    /// Your shortcuts from keys.toml (keymap.rs), and whether they were asked for yet.
    pub keymap: crate::keymap::Keymap,
    pub keymap_loaded: bool,
    /// Sidebar rows and window title as set in keys.toml (ui_config.rs), and the title last given to the window.
    pub ui: crate::ui_config::UiConfig,
    pub window_title: String,
    /// Mouse wheel: the part of a line not yet scrolled (kept for the next event).
    pub wheel_rest: f32,
    /// Kitty graphics per pane (kitty_graphics.rs), and the size of a terminal cell in pixels as last painted.
    pub graphics: Arc<Mutex<HashMap<String, crate::kitty_graphics::Graphics>>>,
    pub cell_size: (f32, f32),
    /// Plugins found in the plugins folder (plugins.rs, plugin_actions.rs).
    pub plugins: Vec<crate::plugins::Plugin>,
    /// In-app toasts (toasts.rs) and whether this window is the active one (set while painting).
    pub toasts: crate::toasts::Toasts,
    /// Replies of fire-and-forget requests: only their errors matter (toasts).
    pub fired: Mutex<Vec<std::sync::mpsc::Receiver<Result<serde_json::Value, String>>>>,
    /// Inbox machine filter: "" = all, "local", or a machine alias.
    pub inbox_machine: String,
    pub window_active: bool,
    /// Tabs put away on this screen (tab settle): hidden from the tab bar and skipped by next / previous, still running.
    pub settled: std::collections::HashSet<String>,
    /// Sidebar groups the user folded (true) or opened (false), by key (sidebar_fold.rs).
    pub folds: std::collections::HashMap<String, bool>,
    /// Scroll positions of the sidebar's Spaces (0) and Agents (1) lists, for their scrollbars (scrollbar.rs).
    pub sidebar_scroll: [gpui::ScrollHandle; 2],
    /// Terminal text zoom: the size the daemon / default gives, plus the points added with Ctrl+= / Ctrl+-.
    pub base_size: f32,
    /// This device's own font (font_prefs.rs), what the daemon last reported, and the Settings → Appearance form.
    pub font_own: crate::font_prefs::OwnFont,
    /// The pane whose left button press went to the program (mouse tracking) and the cell it was last reported at.
    pub mouse_reporting: Option<String>,
    /// The cell the mouse was last reported at, so a motion inside one cell is not sent again.
    pub mouse_last: Option<(String, usize, usize)>,
    /// A press in a program that tracks clicks only, held until release: sent as a click if nothing got selected.
    pub mouse_click: Option<(String, usize, usize)>,
    /// Text a program copied with OSC 52, waiting for the drain loop to put it on the clipboard.
    pub copied_out: Option<String>,
    /// The in-app update succeeded: the drain loop starts the new program and closes this window.
    pub relaunch: bool,
    pub daemon_font: (String, f32),
    pub font_form: crate::views::settings_look::FontForm,
    /// The multi-line editor of Settings (one file at a time).
    pub settings_editor: Entity<TextInput>,
    /// The theme name last reported by the daemon or picked in Settings ("dark", "light", "midnight", "auto").
    pub theme_choice: String,
    pub daemon_theme: String,
    pub zoom: f32,
}

impl DroverApp {
    /// After an in-app update this program's file on disk is the new version: start it, then close this window. The
    /// new window waits for the restarting daemon like any reconnect. If it cannot start, say how to finish by hand.
    fn relaunch_updated(&mut self, cx: &mut Context<Self>) {
        match std::env::current_exe().and_then(|exe| std::process::Command::new(exe).spawn()) {
            Ok(_) => cx.quit(),
            Err(e) => self.settings.update = Some(Err(format!("Updated. Close and reopen Drover to use the new version ({e})."))),
        }
    }

    pub fn new(cx: &mut Context<Self>) -> Self {
        let client = client::start(|| {});
        // The client thread fills a channel; drain it about 60 times a second, repaint only on change.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(16)).await;
                let Ok(changed) = this.update(cx, |app, cx| {
                    let changed = app.drain();
                    if let Some(text) = app.copied_out.take() {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                    }
                    if std::mem::take(&mut app.relaunch) {
                        app.relaunch_updated(cx);
                    }
                    changed
                }) else { break };
                if changed {
                    let _ = this.update(cx, |_, cx| cx.notify());
                }
            }
        })
        .detach();
        let agents_filter = cx.new(|cx| TextInput::new("Filter agents", cx));
        cx.observe(&agents_filter, |_, _, cx| cx.notify()).detach();
        let shortcuts_filter = cx.new(|cx| TextInput::new("Filter by name or key", cx));
        cx.observe(&shortcuts_filter, |_, _, cx| cx.notify()).detach();
        let agents_send = cx.new(|cx| TextInput::new("Send to this agent…", cx));
        cx.subscribe(&agents_send, |app, input, ev, cx| {
            if let crate::views::text_input::InputEvent::Submit = ev {
                let text = input.read(cx).text.clone();
                app.send_to_selected_agent(text);
                input.update(cx, |i, cx| i.set_text("", cx));
            }
        }).detach();
        let changes_in = crate::views::changes::ChangesInputs::new(cx);
        let machines_form = crate::views::machines::MachinesForm::new(cx);
        let font_form = crate::views::settings_look::FontForm::new(cx);
        let feedback = crate::views::feedback::FeedbackForm::new(cx);
        let settings_editor = cx.new(|cx| TextInput::new_multiline(cx));
        cx.subscribe(&settings_editor, |app, _, ev, cx| {
            if let crate::views::text_input::InputEvent::Submit = ev { app.save_setting_file(cx); }
        }).detach();
        let family = std::env::var("DROVER_NATIVE_FONT").unwrap_or_else(|_| theme::DEFAULT_TERMINAL_FONT.into());
        DroverApp { client, store: Store::default(), screens: Arc::default(), space: None, tab: None, connected: false,
                    status: "connecting…".into(), term_font: TermFont::new(&family, 13.0), switched_at: Arc::default(), last_switch: None,
                    focused: None, term_focus: cx.focus_handle(), marked: None, cursor_bounds: Arc::default(),
                    sizes: Arc::default(), resize_sent: Arc::default(), pending: vec![], show_when_known: None, confirm: None, sidebar_open: true,
                    term_bounds: Arc::default(), selecting: None, hover_link: None, view: View::Terminals, agents_filter, agents_send, shortcuts_filter, banner_dismissed: HashMap::new(),
                    agents: Default::default(), changes: Default::default(), changes_in,
                    files: Default::default(), inbox_filter: Default::default(),
                    settings: Default::default(), machines_form, rules: Default::default(), whats_new: Default::default(), feedback,
                    code_family: crate::font_fallback::BUNDLED_MONO.into(), panels: HashMap::new(),
                    panel_width: 440., sidebar_width: crate::theme_data::layout::SIDEBAR_WIDTH, drag: None, split_bounds: Arc::default(),
                    chord: false, show_shortcuts: false, picker: None, peek: None, want_term_focus: true, area: Area::Pane, menu: None, copy_mode: None, keymap: Default::default(), keymap_loaded: false, ui: Default::default(), window_title: String::new(), wheel_rest: 0.0, graphics: Default::default(), cell_size: (8., 17.), plugins: vec![],
                    toasts: Default::default(), fired: Default::default(), inbox_machine: String::new(), window_active: true, settled: Default::default(), folds: Default::default(), sidebar_scroll: [gpui::ScrollHandle::new(), gpui::ScrollHandle::new()], base_size: 13.0, font_own: crate::font_prefs::load_from(&crate::font_prefs::path()), mouse_reporting: None, mouse_last: None, mouse_click: None, copied_out: None, relaunch: false, daemon_font: (String::new(), 0.0), font_form, settings_editor, theme_choice: "dark".into(), daemon_theme: String::new(), zoom: 0.0 }
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
                        // Follow the daemon only when its theme changes: it reloads settings.toml a moment after a pick made here,
                        // and the state events in between still carry the old one.
                        if let Some(t) = data["theme"].as_str().filter(|t| *t != self.daemon_theme) {
                            self.daemon_theme = t.to_string();
                            crate::theme::set_theme(t);
                            self.theme_choice = t.to_string();
                        }
                    }
                    self.toast_for_event(&name, &data);
                    self.store.apply(&name, &data);
                    self.fit_screens();
                }
                Incoming::Output { pane, data, replay } => self.feed(&pane, &data, replay),
            }
        }
        changed |= self.toasts.tick();
        self.check_update_when_due();
        changed |= self.take_replies();
        changed |= crate::views::files::tick(self);
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
        let mut graphics = self.graphics.lock().unwrap();
        if replay || !screens.contains_key(pane) {
            graphics.remove(pane); // a replay draws its images again
            // Prefer the size this window measured and sent (pane.resize): the store can still hold the size the pane
            // was created at (120x32), and output written for more rows would scroll blank lines into history, which
            // the next grow pulls back on screen, pushing the content down.
            let (cols, rows) = self.sizes.lock().unwrap().get(pane).copied().unwrap_or((p.cols, p.rows));
            screens.insert(pane.to_string(), PaneScreen::new(cols.max(2) as usize, rows.max(2) as usize, HISTORY));
        }
        if let Some(s) = screens.get_mut(pane) {
            let replies = graphics.entry(pane.to_string()).or_default().feed(s, data, self.cell_size);
            // Answers go back to the program, except while replaying old output (the question is long past).
            for data in replies.into_iter().filter(|_| !replay) {
                let _ = self.client.commands.send(Command::Input { pane: pane.to_string(), data });
            }
            // Text the program copied (OSC 52: Claude Code copies its own mouse selection that way); the drain loop
            // puts it on the clipboard. A replay repeats copies made long ago: those are skipped.
            if let Some(text) = s.take_copied().filter(|_| !replay) {
                self.copied_out = Some(text);
            }
        }
        if replay && let Some(t) = self.last_switch {
            eprintln!("trace: replay of {pane} ({} KB) fed {:.1} ms after click", data.len() / 1024, t.elapsed().as_secs_f64() * 1000.0);
        }
    }

    /// The daemon owns each pane's size (shared by all viewers): follow it.
    fn fit_screens(&mut self) {
        let mut screens = self.screens.lock().unwrap();
        let sizes = self.sizes.lock().unwrap();
        let sent = self.resize_sent.lock().unwrap();
        for p in &self.store.panes {
            // Our own resize is on its way: shrinking to the old size would scroll blank lines into history, which the
            // grow that follows pulls back on screen (the prompt lands several lines down).
            let waiting = sizes.get(&p.id).is_some_and(|&want| want != (p.cols, p.rows))
                && sent.get(&p.id).is_some_and(|t| t.elapsed() < Duration::from_secs(2));
            if waiting {
                continue;
            }
            if let Some(s) = screens.get_mut(&p.id) {
                let g = s.term().grid();
                if (g.columns(), g.screen_lines()) != (p.cols as usize, p.rows as usize) && p.cols > 1 && p.rows > 1 {
                    crate::actions::trace(&format!("fit {} to {}x{}", p.id, p.cols, p.rows));
                    s.resize(p.cols as usize, p.rows as usize);
                }
            }
        }
    }

    /// {family, size} from the daemon (0 / "" = unknown: keep ours). A new font re-sizes every pane.
    fn follow_terminal_font(&mut self, v: &serde_json::Value) {
        self.daemon_font = (v["family"].as_str().unwrap_or("").to_string(), v["size"].as_f64().unwrap_or(0.0) as f32);
        self.refresh_font();
    }

    /// The font in use: this device's own choice, else what the daemon reports, else what we had.
    pub fn refresh_font(&mut self) {
        let family = if !self.font_own.family.is_empty() { self.font_own.family.clone() }
            else if !self.daemon_font.0.is_empty() { self.daemon_font.0.clone() }
            else { self.term_font.font.family.to_string() };
        let size = if self.font_own.size > 0.0 { self.font_own.size } else if self.daemon_font.1 > 0.0 { self.daemon_font.1 } else { self.base_size };
        self.base_size = size;
        self.apply_term_font(&family);
    }

    /// Whole-interface zoom (Ctrl+Alt+= / Ctrl+Alt+- / Ctrl+Alt+0): `by` +1 / -1 steps, 0 = 100 %. Kept on this device.
    pub fn zoom_app(&mut self, by: i32, cx: &mut Context<Self>) {
        let next = crate::ui_scale::step(crate::ui_scale::zoom(), by);
        crate::ui_scale::set(next);
        if let Err(e) = crate::ui_scale::save_to(&crate::ui_scale::path(), next) {
            self.toasts.error(format!("Could not save the interface size: {e}"));
        }
        self.sizes.lock().unwrap().clear(); // the panes are re-measured at the new size
        cx.notify();
    }

    /// Terminal text zoom (Ctrl+= / Ctrl+- / Ctrl+0): `by` points, 0 = back to the daemon's size. Only the text;
    /// the window's own chrome keeps its size.
    pub fn zoom_terminal(&mut self, by: f32) {
        self.zoom = if by == 0.0 { 0.0 } else { (self.zoom + by).clamp(-5.0, 20.0) };
        let family = self.term_font.font.family.to_string();
        self.apply_term_font(&family);
    }

    fn apply_term_font(&mut self, family: &str) {
        let size = (self.base_size + self.zoom).clamp(8.0, 40.0);
        if family != self.term_font.font.family.as_ref() || size != self.term_font.size {
            self.term_font = TermFont::new(family, size);
            self.sizes.lock().unwrap().clear();
        }
    }

    fn keep_selection_valid(&mut self) {
        let store = &self.store;
        self.panels.retain(|pane, _| store.pane(pane).is_some()); // a closed pane keeps no panel
        if self.peek.as_ref().is_some_and(|p| store.pane(p).is_none()) {
            self.peek = None; // the peeked pane closed
        }
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
        self.view = View::Terminals;
        self.want_term_focus = true;
        self.area = Area::Space;
        self.space = Some(id);
        self.tab = None;
        self.keep_selection_valid();
        // Keys go to the space's focused pane, and the daemon marks it as seen.
        if let Some(pane) = self.focused.clone() {
            self.rpc("pane.focus", serde_json::json!({"pane": pane}));
        }
        cx.notify();
    }

    pub fn select_tab(&mut self, id: String, cx: &mut Context<Self>) {
        self.mark_switch();
        self.view = View::Terminals;
        self.want_term_focus = true;
        self.area = Area::Tab;
        self.settled.remove(&id); // showing a settled tab brings it back
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
        self.area = Area::Pane;
        window.focus(&self.term_focus);
        cx.notify();
    }
}

impl Render for DroverApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.window_active = window.is_window_active();
        let resolved = crate::terminal_paint::resolved(&self.term_font, window);
        self.cell_size = (f32::from(crate::terminal_paint::cell_width(&resolved, window)), resolved.line_height);
        self.code_family = resolved.font.family;
        // Terminals shown, or a question open: keys belong to the app, not to a view's field (e.g. after a tab
        // click left a view, or a close was asked from the sidebar while a field had focus).
        if (self.confirm.is_some() || (self.want_term_focus && self.view == View::Terminals)) && self.picker.is_none() {
            self.want_term_focus = false;
            if !self.term_focus.is_focused(window) {
                window.focus(&self.term_focus);
            }
        }
        match self.view {
            View::Agents => crate::views::agents::refresh(self),
            View::Settings => crate::views::settings::refresh(self, cx),
            View::WhatsNew => crate::views::whats_new::refresh(self),
            _ => {}
        }
        crate::views::panel::refresh(self, cx);
        // The tab bar stays outside the focus-tracking area: a click there would focus it and mark the press as handled,
        // and Windows then ignores it on the window buttons (minimize / maximize / close) and the drag area (topbar.rs).
        let main = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .child(topbar::tab_bar(self, window.is_maximized(), cx))
            .child(div().flex().flex_col().flex_1().min_h_0().min_w_0()
                .track_focus(&self.term_focus)
                .on_key_down(cx.listener(Self::on_key))
                .child(if self.view == View::Terminals {
                    div().flex().flex_row().flex_1().min_h_0().min_w_0()
                        .child(layout_view::tab_body(self, cx))
                        .children(crate::views::panel::panel(self, cx))
                } else {
                    views::body(self, cx)
                }));
        if self.connected && !self.keymap_loaded {
            self.keymap_loaded = true;
            self.load_keymap(false);
            self.reload_plugins(false);
        }
        // Window title from the format in keys.toml; set only when it changes.
        let (space, agent) = (self.space.as_deref().and_then(|s| self.store.spaces.iter().find(|x| x.id == s)), self.focused.as_deref().and_then(|p| self.store.pane(p)));
        let tab = agent.and_then(|p| self.store.tab(&p.tab_id)).map(|t| t.name.as_str()).unwrap_or("");
        let title = crate::ui_config::title(&self.ui.window_title, space.map(|s| s.name.as_str()).unwrap_or(""), tab,
            agent.map(|p| p.name.as_str()).unwrap_or(""), agent.map(|p| p.state.as_str()).unwrap_or(""));
        if title != self.window_title {
            window.set_window_title(&title);
            self.window_title = title;
        }
        let view_size = window.viewport_size();
        let overlay = crate::dialogs::confirm_overlay(self, cx).or_else(|| crate::picker::overlay(self, cx))
            .or_else(|| crate::context_menu::overlay(self, view_size, cx))
            .or_else(|| crate::peek::overlay(self, view_size, cx))
            .or_else(|| crate::dialogs::shortcuts_overlay(self, cx))
            .or_else(|| crate::views::machines_add_dialog::overlay(self, cx));
        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(theme::canvas()))
            .text_color(rgb(theme::fg()))
            .font_family(theme::UI_FONT)
            .text_size(crate::ui_scale::px(13.))
            // Resizing (drag.rs): the whole window follows the mouse until the button is released.
            .on_mouse_move(cx.listener(|app, ev, _, cx| app.drag_move(ev, cx)))
            .on_mouse_up(gpui::MouseButton::Left, cx.listener(|app, _, _, cx| app.drag_end(cx)))
            .when(self.drag.is_some(), |d| match &self.drag {
                Some(crate::drag::Drag::Split { dir: drover_proto::wire::Dir::Col, .. }) => d.cursor_row_resize(),
                Some(crate::drag::Drag::Scroll { .. }) => d,
                _ => d.cursor_col_resize(),
            })
            .child(div().flex().flex_row().flex_1().min_h_0()
                .when(self.sidebar_open, |d| d.child(sidebar::sidebar(self, cx))).child(main))
            .child(topbar::status_bar(self, cx))
            .children(crate::toasts::overlay(self, cx))
            .children(overlay)
            .children(if cfg!(target_os = "linux") { crate::window_controls::resize_edges(cx) } else { Vec::new() })
    }
}
