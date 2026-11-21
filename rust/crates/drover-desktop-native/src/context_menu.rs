//! Right-click menus (Herdr's context menus) on a space, a tab, an agent in the sidebar and a terminal pane: a small
//! card at the mouse with a few actions. Click an item to run it, click anywhere else or press Escape to close.

use alacritty_terminal::term::TermMode;
use gpui::{Context, Div, MouseButton, Pixels, Point, Size, Window, div, prelude::*, rgb};

use crate::picker::Kind;
use crate::theme::{self, fg, UI_FONT};
use crate::ui_scale::px;
use crate::workspace_app::DroverApp;

/// What a menu item does; the String is the id of the space, tab or pane it was opened on.
#[derive(Clone)]
pub enum Act {
    RenameSpace(String), CloseSpace(String),
    RenameTab(String), SettleTab(String), CloseTab(String),
    RenameAgent(String), SplitRight(String), SplitDown(String), Zoom(String), ClosePane(String),
    Copy, Paste, SelectAll, ClearPane,
}

pub struct Menu {
    pub at: Point<Pixels>,
    pub items: Vec<(&'static str, Act)>,
}

pub fn space_items(id: &str) -> Vec<(&'static str, Act)> {
    vec![("Rename space", Act::RenameSpace(id.into())), ("Close space", Act::CloseSpace(id.into()))]
}

pub fn tab_items(id: &str) -> Vec<(&'static str, Act)> {
    vec![("Rename tab", Act::RenameTab(id.into())), ("Put this tab away", Act::SettleTab(id.into())), ("Close tab", Act::CloseTab(id.into()))]
}

/// The agent / terminal menu; `with_text` adds Copy, Paste, Select all, Clear (a menu on the terminal itself).
pub fn pane_items(id: &str, with_text: bool) -> Vec<(&'static str, Act)> {
    let mut items = vec![];
    if with_text {
        items.extend([("Copy", Act::Copy), ("Paste", Act::Paste), ("Select all", Act::SelectAll), ("Clear screen and scrollback", Act::ClearPane)]);
    }
    items.extend([("Rename agent", Act::RenameAgent(id.into())), ("Split right", Act::SplitRight(id.into())),
                  ("Split down", Act::SplitDown(id.into())), ("Zoom pane", Act::Zoom(id.into())), ("Close pane", Act::ClosePane(id.into()))]);
    items
}

impl DroverApp {
    pub fn open_menu(&mut self, at: Point<Pixels>, items: Vec<(&'static str, Act)>, cx: &mut Context<Self>) {
        self.menu = Some(Menu { at, items });
        cx.notify();
    }

    pub fn close_menu(&mut self, cx: &mut Context<Self>) {
        self.menu = None;
        cx.notify();
    }

    fn run_menu_item(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let act = self.menu.take().and_then(|m| m.items.into_iter().nth(index)).map(|(_, a)| a);
        let Some(act) = act else { return };
        match act {
            Act::RenameSpace(id) => self.open_rename_space(Some(id), window, cx),
            Act::CloseSpace(id) => self.ask_close_space(&id, cx),
            Act::RenameTab(id) => self.open_rename_tab(Some(id), window, cx),
            Act::SettleTab(id) => { self.select_tab(id, cx); self.settle_current_tab(cx); }
            Act::CloseTab(id) => self.ask_close_tab(&id, cx),
            Act::RenameAgent(id) => { self.show_pane(&id); self.open_picker(Kind::Rename, window, cx); }
            Act::SplitRight(id) => self.split(&id, "row"),
            Act::SplitDown(id) => self.split(&id, "col"),
            Act::Zoom(id) => self.toggle_zoom(&id),
            Act::ClosePane(id) => self.ask_close_pane(&id, cx),
            Act::Copy => { self.copy_selection(cx); }
            Act::Paste => self.paste(cx),
            Act::SelectAll => self.select_all(cx),
            Act::ClearPane => self.clear_focused_pane(),
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
    let (w, h) = (200., 8. + 28. * m.items.len() as f32);
    let x = f32::from(m.at.x).min(f32::from(view.width) - w - 4.).max(0.);
    let y = f32::from(m.at.y).min(f32::from(view.height) - h - 4.).max(0.);
    let mut card = div().absolute().left(gpui::px(x)).top(gpui::px(y)).w(px(w)).p(px(4.)).flex().flex_col().rounded(px(8.))
        .bg(rgb(theme::card())).border_1().border_color(theme::border2()).shadow_lg().font_family(UI_FONT).text_size(px(12.5));
    for (i, (label, _)) in m.items.iter().enumerate() {
        card = card.child(div().h(px(28.)).px(px(10.)).flex().items_center().rounded(px(5.)).text_color(rgb(fg()))
            .hover(|d| d.bg(theme::hover())).child(*label)
            .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, window, cx| { cx.stop_propagation(); app.run_menu_item(i, window, cx); })));
    }
    Some(div().absolute().top_0().left_0().size_full().occlude()
        .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.close_menu(cx)))
        .on_mouse_down(MouseButton::Right, cx.listener(|app, _, _, cx| app.close_menu(cx)))
        .child(card))
}
