//! The right panel beside the terminals, as in Herdr: Agents, Files or Changes of the focused pane (its agent, its
//! folder, its worktree), following the focus. Opened and closed from the tab bar's buttons.

use gpui::{Context, Div, FontWeight, MouseButton, div, prelude::*, rgb};
use crate::ui_scale::px;

use super::{View, agents, changes, files};
use crate::theme::{self, chrome, dim, muted};
use crate::widgets::{icon, short_path};
use crate::workspace_app::DroverApp;

/// What the panel looks at: the focused pane of the shown tab (its folder follows its program, e.g. an agent's
/// worktree), else the space. `key` changes whenever the panel should load again.
#[derive(Clone, Debug, PartialEq)]
pub struct Scope {
    pub pane: Option<String>,
    pub space: String,
    pub cwd: String,
    pub key: String,
}

pub fn scope(app: &DroverApp) -> Option<Scope> {
    let space = changes::space(app)?;
    let pane = app.focused.as_deref().and_then(|id| app.store.pane(id))
        .filter(|p| Some(&p.tab_id) == app.tab.as_ref() && !p.cwd.is_empty());
    Some(match pane {
        Some(p) => Scope { pane: Some(p.id.clone()), space: space.id.clone(), cwd: p.cwd.clone(), key: format!("{}|{}", p.id, p.cwd) },
        None => Scope { pane: None, space: space.id.clone(), cwd: space.cwd.clone(), key: format!("{}|{}", space.id, space.cwd) },
    })
}

impl DroverApp {
    /// The panel open beside the focused pane, if any.
    pub fn panel(&self) -> Option<View> {
        self.panels.get(self.focused.as_deref()?).copied()
    }

    /// Open (or close, with None) a panel for the focused pane only.
    pub fn set_panel(&mut self, view: Option<View>) {
        let Some(pane) = self.focused.clone() else { return };
        match view {
            Some(v) => { self.panels.insert(pane, v); }
            None => { self.panels.remove(&pane); }
        }
    }

    /// Open `view` in the panel, or close it when it is the one shown.
    pub fn toggle_panel(&mut self, view: View, cx: &mut Context<Self>) {
        self.set_panel(if self.panel() == Some(view) { None } else { Some(view) });
        self.view = View::Terminals;
        cx.notify();
    }
}

/// Load what the open panel shows (called on every render; each panel fetches only when its scope changed).
pub fn refresh(app: &mut DroverApp, cx: &gpui::App) {
    if app.view != View::Terminals {
        return;
    }
    match app.panel() {
        Some(View::Agents) => agents::refresh(app),
        Some(View::Changes) => changes::refresh(app, cx),
        _ => {}
    }
}

pub fn panel(app: &DroverApp, cx: &mut Context<DroverApp>) -> Option<Div> {
    let view = app.panel().filter(|_| app.view == View::Terminals)?;
    let sc = scope(app);
    let what = match (&sc, sc.as_ref().and_then(|s| s.pane.as_deref()).and_then(|id| app.store.pane(id))) {
        (_, Some(p)) => format!("{} · {}", p.name, short_path(&p.cwd)),
        (Some(s), None) => short_path(&s.cwd),
        _ => String::new(),
    };
    let head = div().h(px(38.)).flex_none().flex().items_center().gap(px(8.)).px(px(12.)).bg(rgb(chrome()))
        .border_b_1().border_color(theme::border())
        .child(icon(view.icon(), 14., muted()))
        .child(div().flex_none().font_weight(FontWeight::SEMIBOLD).child(view.title()))
        .child(div().flex_1().min_w_0().truncate().text_size(px(11.5)).text_color(rgb(dim())).child(what))
        .child(div().size(px(22.)).rounded(px(5.)).flex().items_center().justify_center().hover(|d| d.bg(theme::hover()))
            .child(icon("close", 12., dim()))
            .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| { app.set_panel(None); cx.notify(); })));
    let body = match view {
        View::Agents => agents::panel_body(app, cx),
        View::Files => files::panel_body(app, cx),
        _ => changes::panel_body(app, cx),
    };
    // The left edge resizes the panel (drag.rs), 280-900 px as Herdr's.
    let w0 = app.panel_width;
    let edge = div().absolute().top_0().left(px(-3.)).h_full().w(px(6.)).cursor_col_resize()
        .on_mouse_down(MouseButton::Left, cx.listener(move |app, ev: &gpui::MouseDownEvent, _, cx| {
            app.start_drag(crate::drag::Drag::Panel { x0: f32::from(ev.position.x), w0 }, cx);
        }));
    Some(div().relative().w(px(app.panel_width)).flex_none().flex().flex_col().min_h_0().border_l_1().border_color(theme::border())
        .bg(rgb(theme::canvas())).child(head).child(body).child(edge))
}
