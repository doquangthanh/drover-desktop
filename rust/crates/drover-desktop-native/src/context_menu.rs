//! Right-click menus (Herdr's context menus) on a space, a tab, an agent in the sidebar and a terminal pane: a small
//! card at the mouse with a few actions. Click an item to run it, click anywhere else or press Escape to close.

use alacritty_terminal::term::TermMode;
use gpui::{Context, Div, MouseButton, Pixels, Point, Size, Window, div, prelude::*, rgb};

use crate::picker::Kind;
use crate::theme::{self, dim, fg, muted, UI_FONT};
use crate::widgets::{icon, kind_icon};
use crate::ui_scale::px;
use crate::workspace_app::DroverApp;

/// What a menu item does; the String is the id of the space, tab or pane it was opened on.
#[derive(Clone)]
pub enum Act {
    RenameSpace(String), CloseSpace(String),
    RenameTab(String), SettleTab(String), CloseTab(String),
    RenameAgent(String), Peek(String), SplitRight(String), SplitDown(String), Zoom(String), ClosePane(String),
    Copy, Paste, SelectAll, ClearPane,
    // The tab bar's + menu (new_pane_menu.rs): the String of StartAgent is the agent's command.
    NewShell, StartAgent(String), NewSpace, NewSpaceInFolder, NewWorktree, OpenWorktrees,
}

/// What a menu line shows on its left: a line icon by name, or an agent kind's badge.
#[derive(Clone)]
pub enum Icon { Name(&'static str), Kind(String) }

/// One line of a menu: an action, a small heading, or a rule between groups.
pub enum Entry {
    /// `hint` is shown on the right (a shortcut, "not installed"); `key` picks the item while the menu is open;
    /// an item without `act` is shown greyed and does nothing.
    Item { label: String, icon: Option<Icon>, hint: String, key: Option<String>, act: Option<Act> },
    Head(String),
    Rule,
}

pub fn item(label: &str, act: Act) -> Entry {
    Entry::Item { label: label.into(), icon: None, hint: String::new(), key: None, act: Some(act) }
}

pub struct Menu {
    pub at: Point<Pixels>,
    pub entries: Vec<Entry>,
}

pub fn space_items(id: &str) -> Vec<Entry> {
    vec![item("Rename space", Act::RenameSpace(id.into())), item("Close space", Act::CloseSpace(id.into()))]
}

pub fn tab_items(id: &str) -> Vec<Entry> {
    vec![item("Rename tab", Act::RenameTab(id.into())), item("Put this tab away", Act::SettleTab(id.into())), item("Close tab", Act::CloseTab(id.into()))]
}

/// The agent / terminal menu; `with_text` adds Copy, Paste, Select all, Clear (a menu on the terminal itself).
pub fn pane_items(id: &str, with_text: bool) -> Vec<Entry> {
    let mut items = vec![];
    if with_text {
        items.extend([item("Copy", Act::Copy), item("Paste", Act::Paste), item("Select all", Act::SelectAll), item("Clear screen and scrollback", Act::ClearPane)]);
    }
    if !with_text {
        // The sidebar's menu: a pane that may not be in front of you.
        items.push(item("Peek (stay where you are)", Act::Peek(id.into())));
    }
    items.extend([item("Rename agent", Act::RenameAgent(id.into())), item("Split right", Act::SplitRight(id.into())),
                  item("Split down", Act::SplitDown(id.into())), item("Zoom pane", Act::Zoom(id.into())), item("Close pane", Act::ClosePane(id.into()))]);
    items
}

/// Height of one menu line.
fn entry_h(e: &Entry) -> f32 {
    match e { Entry::Item { .. } => 30., Entry::Head(_) => 26., Entry::Rule => 9. }
}

impl DroverApp {
    pub fn open_menu(&mut self, at: Point<Pixels>, entries: Vec<Entry>, cx: &mut Context<Self>) {
        self.menu = Some(Menu { at, entries });
        cx.notify();
    }

    /// A key typed while a menu is open: runs the item it picks (the + menu's 1–9). False when none does.
    pub fn menu_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let picked = self.menu.as_ref().and_then(|m| m.entries.iter().position(|e| {
            matches!(e, Entry::Item { key: Some(k), act: Some(_), .. } if k == key)
        }));
        picked.inspect(|&i| self.run_menu_item(i, window, cx)).is_some()
    }

    pub fn close_menu(&mut self, cx: &mut Context<Self>) {
        self.menu = None;
        cx.notify();
    }

    fn run_menu_item(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let act = self.menu.take().and_then(|m| m.entries.into_iter().nth(index)).and_then(|e| match e {
            Entry::Item { act, .. } => act,
            _ => None,
        });
        let Some(act) = act else { return cx.notify() };
        match act {
            Act::RenameSpace(id) => self.open_rename_space(Some(id), window, cx),
            Act::CloseSpace(id) => self.ask_close_space(&id, cx),
            Act::RenameTab(id) => self.open_rename_tab(Some(id), window, cx),
            Act::SettleTab(id) => { self.select_tab(id, cx); self.settle_current_tab(cx); }
            Act::CloseTab(id) => self.ask_close_tab(&id, cx),
            Act::RenameAgent(id) => { self.show_pane(&id); self.open_picker(Kind::Rename, window, cx); }
            Act::Peek(id) => self.open_peek(&id, window, cx),
            Act::SplitRight(id) => self.split(&id, "row"),
            Act::SplitDown(id) => self.split(&id, "col"),
            Act::Zoom(id) => self.toggle_zoom(&id),
            Act::ClosePane(id) => self.ask_close_pane(&id, cx),
            Act::Copy => { self.copy_selection(cx); }
            Act::Paste => self.paste(cx),
            Act::SelectAll => self.select_all(cx),
            Act::ClearPane => self.clear_focused_pane(),
            Act::NewShell => self.start_in_space("", None, cx),
            Act::StartAgent(command) => self.start_in_space(&command, None, cx),
            Act::NewSpace => self.new_space(cx),
            Act::NewSpaceInFolder => self.open_folder_picker(window, cx),
            Act::NewWorktree => self.open_picker(Kind::NewWorktree, window, cx),
            Act::OpenWorktrees => self.open_worktrees(),
        }
        cx.notify();
    }

    /// Clear the focused pane's scrollback and, at a shell prompt, its screen (Herdr's `keys.clear_pane`). A full-screen
    /// program (vim, an agent's TUI) is left alone.
    pub fn clear_focused_pane(&mut self) {
        let Some(pane) = self.focused.clone() else { return };
        let full_screen = {
            let mut screens = self.screens.lock().unwrap();
            let Some(screen) = screens.get_mut(&pane) else { return };
            let alt = screen.term().mode().contains(TermMode::ALT_SCREEN);
            if !alt { screen.term_mut().grid_mut().clear_history(); }
            alt
        };
        if !full_screen {
            self.send_to_focused("\x0c".into()); // Ctrl+L: the shell redraws its prompt at the top
        }
    }
}

pub fn overlay(app: &DroverApp, view: Size<Pixels>, cx: &mut Context<DroverApp>) -> Option<Div> {
    let m = app.menu.as_ref()?;
    // Wider when lines carry a hint on the right; sizes are logical (ui_scale) so the clamp uses scaled values.
    let wide = m.entries.iter().any(|e| matches!(e, Entry::Item { hint, .. } if !hint.is_empty()));
    let (w, h) = (if wide { 290. } else { 200. }, 8. + m.entries.iter().map(entry_h).sum::<f32>());
    let (sw, sh) = (f32::from(px(w)), f32::from(px(h)));
    let x = f32::from(m.at.x).min(f32::from(view.width) - sw - 4.).max(0.);
    let y = f32::from(m.at.y).min(f32::from(view.height) - sh - 4.).max(0.);
    let mut card = div().absolute().left(gpui::px(x)).top(gpui::px(y)).w(px(w)).p(px(4.)).flex().flex_col().rounded(px(8.))
        .bg(rgb(theme::card())).border_1().border_color(theme::border2()).shadow_lg().font_family(UI_FONT).text_size(px(12.5));
    for (i, e) in m.entries.iter().enumerate() {
        card = card.child(match e {
            Entry::Head(label) => div().h(px(entry_h(e))).px(px(10.)).flex().items_end().pb(px(4.)).text_size(px(11.5)).text_color(rgb(dim())).child(label.clone()),
            Entry::Rule => div().h(px(entry_h(e))).flex().items_center().child(div().h(px(1.)).w_full().bg(theme::border())),
            Entry::Item { label, icon: ic, hint, act, .. } => {
                let on = act.is_some();
                div().h(px(entry_h(e))).px(px(10.)).flex().items_center().gap(px(10.)).rounded(px(5.))
                    .text_color(rgb(if on { fg() } else { dim() }))
                    .children(ic.as_ref().map(|ic| match ic {
                        Icon::Name(n) => div().flex_none().child(icon(n, 14., if on { muted() } else { dim() })),
                        Icon::Kind(k) => kind_icon(k).when(!on, |d| d.opacity(0.5)),
                    }))
                    .child(div().flex_1().min_w_0().truncate().child(label.clone()))
                    .child(div().flex_none().text_size(px(11.5)).text_color(rgb(dim())).child(hint.clone()))
                    .when(on, |d| d.hover(|d| d.bg(theme::hover()))
                        .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, window, cx| { cx.stop_propagation(); app.run_menu_item(i, window, cx); })))
                    // A greyed item swallows the click so the menu stays open.
                    .when(!on, |d| d.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()))
            }
        });
    }
    Some(div().absolute().top_0().left_0().size_full().occlude()
        .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.close_menu(cx)))
        .on_mouse_down(MouseButton::Right, cx.listener(|app, _, _, cx| app.close_menu(cx)))
        .child(card))
}
