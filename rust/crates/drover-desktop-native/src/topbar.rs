//! Tab bar (`.tabs`, `.tab`, `.pb` in workspace.css) and status bar (`.status`).

use gpui::{Context, Div, FontWeight, MouseButton, WindowControlArea, div, prelude::*, rgb};
use crate::ui_scale::px;

use crate::theme::{self, accent, chrome, dim, done, error, fg, muted, raised, UI_FONT, tint};
use crate::widgets::{icon, key_cap, kind_icon, short_path, state_mark};
use crate::views::View;
use crate::workspace_app::DroverApp;

fn pb(view: View, on: bool) -> Div {
    div().h(px(26.)).flex().items_center().gap(px(5.)).px(px(8.)).rounded(px(6.)).text_size(px(12.))
        .text_color(rgb(if on { fg() } else { muted() })).when(on, |d| d.bg(theme::selected()))
        .hover(|d| d.bg(theme::hover()).text_color(rgb(fg()))).child(icon(view.icon(), 14., if on { fg() } else { muted() }))
        .child(view.title())
}

pub fn tab_bar(app: &DroverApp, maximized: bool, cx: &mut Context<DroverApp>) -> Div {
    let mut bar = div().h(px(crate::theme_data::layout::TOPBAR_HEIGHT)).flex_none().flex().items_center().gap(px(3.)).pl(px(10.)).pr(px(8.))
        .bg(rgb(chrome())).border_b_1().border_color(theme::border()).font_family(UI_FONT);
    if let Some(space) = app.space.as_deref() {
        for t in app.store.tabs_of(space).filter(|t| !app.settled.contains(&t.id)) {
            let on = app.view == View::Terminals && app.tab.as_deref() == Some(t.id.as_str());
            let (id, close_id, menu_id) = (t.id.clone(), t.id.clone(), t.id.clone());
            // The tab shows its most urgent agent, else its first pane.
            let lead = app.store.panes.iter().filter(|p| p.tab_id == t.id)
                .max_by_key(|p| (p.kind != "shell", drover_proto::store::urgency(&p.state)));
            let (kind, state, exit) = lead.map(|p| (p.kind.clone(), p.state.clone(), p.exit_code)).unwrap_or_default();
            bar = bar.child(
                div().h(px(29.)).flex_none().flex().items_center().gap(px(7.)).px(px(10.)).rounded(px(7.)).border_1()
                    .text_size(px(12.8)).text_color(rgb(if on { fg() } else { muted() }))
                    .when(on, |d| d.bg(rgb(raised())).border_color(theme::border()))
                    .when(!on, |d| d.border_color(gpui::transparent_black()).hover(|d| d.bg(theme::hover())))
                    .child(kind_icon(&kind))
                    .child(t.name.clone())
                    .child(state_mark(&state, exit, gpui::SharedString::from(format!("tab-{}", t.id))))
                    .when(on, |d| d.child(div().rounded(px(4.)).hover(|d| d.bg(theme::hover())).child(icon("close", 12., dim()))
                        .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { cx.stop_propagation(); app.ask_close_tab(&close_id, cx); }))))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |app, ev: &gpui::MouseDownEvent, window, cx| {
                        if ev.click_count >= 2 { app.open_rename_tab(Some(id.clone()), window, cx); } else { app.select_tab(id.clone(), cx); }
                    }))
                    .on_mouse_down(MouseButton::Right, cx.listener(move |app, ev: &gpui::MouseDownEvent, _, cx| {
                        app.open_menu(ev.position, crate::context_menu::tab_items(&menu_id), cx);
                    })),
            );
        }
        let away = app.store.tabs_of(space).filter(|t| app.settled.contains(&t.id)).count();
        bar = bar.child(div().size(px(26.)).flex().items_center().justify_center().rounded(px(6.))
            .hover(|d| d.bg(theme::hover())).child(icon("plus", 14., dim()))
            .on_mouse_down(MouseButton::Left, cx.listener(|app, ev: &gpui::MouseDownEvent, _, _| app.open_new_menu(ev.position))))
            .when(away > 0, |d| d.child(div().h(px(22.)).px(px(8.)).rounded(px(6.)).flex().items_center().text_size(px(11.5)).text_color(rgb(dim()))
                .hover(|d| d.bg(theme::hover()).text_color(rgb(fg()))).child(format!("{away} put away"))
                .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.unsettle_all(cx)))));
    }
    // Built before `toggle`, which keeps `cx` borrowed.
    let spacer = div().flex_1().h_full().when(CUSTOM_TITLEBAR, |d| d.window_control_area(WindowControlArea::Drag))
        .when(cfg!(target_os = "linux"), |d| d.on_mouse_down(MouseButton::Left, cx.listener(|_, ev: &gpui::MouseDownEvent, w, _| {
            if ev.click_count >= 2 { w.zoom_window() } else { w.start_window_move() }
        })));
    let controls = CUSTOM_TITLEBAR.then(|| window_controls(maximized, cx));
    let toggle = |view: View| cx.listener(move |app: &mut DroverApp, _: &gpui::MouseDownEvent, _: &mut gpui::Window, cx: &mut Context<DroverApp>| app.toggle_panel(view, cx));
    let open = |view: View| app.view == View::Terminals && app.panel() == Some(view);
    // The empty middle moves the window (and a double click maximizes) once the system title bar is hidden.
    bar.child(spacer)
        .child(div().flex().items_center().gap(px(2.))
            .child(div().size(px(26.)).rounded(px(6.)).flex().items_center().justify_center().hover(|d| d.bg(theme::hover()))
                .child(icon("sidebar", 15., dim()))
                .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| { app.sidebar_open = !app.sidebar_open; cx.notify(); })))
            .child(pb(View::Agents, open(View::Agents)).on_mouse_down(MouseButton::Left, toggle(View::Agents)))
            .child(pb(View::Files, open(View::Files)).on_mouse_down(MouseButton::Left, toggle(View::Files)))
            .child(pb(View::Changes, open(View::Changes)).on_mouse_down(MouseButton::Left, toggle(View::Changes))))
        .when(CUSTOM_TITLEBAR, |d| d.pr_0())
        .children(controls)
}

/// On Windows the white system title bar is hidden (main.rs) and the window is moved, maximized and closed from the
/// app's own top bar, as Herdr does. Linux does the same with client-side decorations (main.rs, window_controls.rs).
/// macOS keeps the system title bar.
pub const CUSTOM_TITLEBAR: bool = cfg!(any(windows, target_os = "linux"));

/// Minimize, maximize / restore and close at the right end of the tab bar. Windows handles their clicks itself
/// through `window_control_area`, as on its own title bar (snap layouts on hovering maximize included). gpui ignores
/// `window_control_area` on Linux, so there the cells act on the click themselves.
fn window_controls(maximized: bool, cx: &mut Context<DroverApp>) -> Div {
    let linux = cfg!(target_os = "linux");
    let cell = |area| div().w(px(46.)).h_full().flex().items_center().justify_center().window_control_area(area);
    let min = cx.listener(|_, _, w: &mut gpui::Window, _| w.minimize_window());
    let max = cx.listener(|_, _, w: &mut gpui::Window, _| w.zoom_window());
    let close = cx.listener(|_, _, w: &mut gpui::Window, cx| { w.remove_window(); cx.quit(); });
    let outline = || div().border_1().border_color(rgb(muted()));
    let max_glyph = if maximized {
        // Restore: two overlapping squares.
        div().relative().size(px(10.))
            .child(div().absolute().top_0().right_0().size(px(8.)).border_t_1().border_r_1().border_color(rgb(muted())))
            .child(outline().absolute().bottom_0().left_0().size(px(8.)))
    } else {
        outline().size(px(10.))
    };
    div().h_full().flex().flex_none().ml(px(6.))
        .child(cell(WindowControlArea::Min).hover(|d| d.bg(theme::hover())).child(div().w(px(10.)).h(px(1.)).bg(rgb(muted())))
            .when(linux, |d| d.on_mouse_down(MouseButton::Left, min)))
        .child(cell(WindowControlArea::Max).hover(|d| d.bg(theme::hover())).child(max_glyph)
            .when(linux, |d| d.on_mouse_down(MouseButton::Left, max)))
        .child(cell(WindowControlArea::Close).hover(|d| d.bg(rgb(0xc42b1c))).child(icon("close", 14., muted()))
            .when(linux, |d| d.on_mouse_down(MouseButton::Left, close)))
}

/// Keys worth knowing on each screen, shown in the status bar: [keys, what it does]. Only keys that work there.
pub fn screen_hints(screen: View) -> &'static [(&'static str, &'static str)] {
    match screen {
        View::Terminals => &[("Ctrl+P", "jump"), ("Ctrl+\\", "split"), ("F8", "next agent")],
        View::Files => &[("Ctrl+P", "jump"), ("Ctrl+Shift+G", "changes")],
        View::Changes => &[("Ctrl+Shift+E", "files"), ("F8", "next agent")],
        View::Inbox => &[("F8", "next agent")],
        _ => &[],
    }
}

pub fn status_bar(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let blocked = app.store.panes.iter().filter(|p| p.state == "blocked").count();
    let space = app.space.as_deref().and_then(|s| app.store.spaces.iter().find(|x| x.id == s));
    let pane = app.focused.as_deref().and_then(|p| app.store.pane(p));
    let mut bar = div().h(px(crate::theme_data::layout::STATUS_HEIGHT)).flex_none().flex().items_center().gap(px(16.)).px(px(14.)).bg(rgb(chrome()))
        .border_t_1().border_color(theme::border()).text_size(px(11.8)).text_color(rgb(muted())).font_family(UI_FONT)
        .child(div().flex().items_center().gap(px(6.))
            .child(div().size(px(6.)).rounded_full().bg(rgb(if app.connected { done() } else { error() })))
            .child(app.status.clone()))
        .child(format!("This computer · {}", app.local_summary()))
        .when(blocked > 0, |d| d.child(div().text_color(rgb(crate::theme::blocked())).child(format!("{blocked} need you"))));
    if let Some(branch) = space.and_then(|s| s.branch.clone()) {
        let dirty = space.and_then(|s| s.dirty).unwrap_or(0);
        bar = bar.child(div().h(px(20.)).px(px(7.)).rounded(px(5.)).flex().items_center().gap(px(5.)).max_w(px(240.))
            .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| { app.set_panel(Some(View::Changes)); app.view = View::Terminals; cx.notify(); }))
            .bg(tint(accent(), 0.14)).text_color(rgb(accent())).text_size(px(11.)).font_weight(FontWeight::SEMIBOLD)
            .child(icon("branch", 12., accent())).child(div().truncate().child(branch))
            .when(dirty > 0, |d| d.child(div().text_color(rgb(crate::theme::blocked())).child(format!("●{dirty}")))));
    }
    if let Some(p) = pane {
        bar = bar.child(div().min_w_0().truncate().child(short_path(&p.cwd)));
    }
    bar.child(div().flex_1())
        .when(app.copy_mode.is_some(), |d| d.child(div().text_color(rgb(accent())).child("COPY MODE · hjkl move · w b e · v V select · y copy · Esc exit")))
        .when(app.chord, |d| d.child(div().text_color(rgb(accent())).child("Ctrl+K… waiting for the second key")))
        .when(!app.chord && app.copy_mode.is_none(), |d| d.children(screen_hints(app.screen()).iter().map(|(keys, what)| {
            div().flex().items_center().gap(px(5.)).child(key_cap(keys)).child(div().text_color(rgb(dim())).child(*what))
        })))
        .child(div().flex().items_center().gap(px(5.)).child(key_cap("Ctrl+K")).child(key_cap("Ctrl+S"))
            .child(div().text_color(rgb(dim())).child("shortcuts"))
            .on_mouse_down(MouseButton::Left, cx.listener(|app, _, window, cx| app.open_shortcuts(window, cx))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hints_exist_for_the_main_screens_and_name_real_keys() {
        for v in [View::Terminals, View::Files, View::Changes] {
            assert!(!screen_hints(v).is_empty(), "{v:?}");
        }
        assert!(screen_hints(View::Settings).is_empty());
        // every hinted key is a shortcut the palette lists, so the bar never advertises a key that does nothing
        for (keys, _) in screen_hints(View::Terminals).iter().chain(screen_hints(View::Files)).chain(screen_hints(View::Changes)) {
            assert!(crate::picker::command_keys().contains(keys), "{keys} is not a palette shortcut");
        }
    }
}
