//! A tab's split layout as nested flex boxes; each leaf is a pane frame with its terminal canvas.
//! The canvas sizes its pane to the frame (pane.resize when cols x rows change) and, for the focused pane,
//! registers the input method handler so composed text reaches the agent.

use alacritty_terminal::grid::Dimensions;
use drover_proto::client::Command;
use drover_proto::wire::{Dir, Node};
use gpui::{
    AnyElement, Bounds, Context, Div, ElementInputHandler, MouseButton, Pixels, canvas, div, prelude::*, px, relative, rgb,
};
use serde_json::json;

use crate::terminal_paint;
use crate::theme::{self, ACCENT, DIM, PANE};
use crate::widgets::{icon, kind_icon, short_path, state_mark};
use crate::workspace_app::DroverApp;

pub fn tab_body(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let body = div().flex().flex_1().min_h_0().min_w_0().p(px(8.));
    match app.tab.clone().and_then(|t| app.store.tab_view(&t).map(|n| (t, n))) {
        Some((tab, node)) => body.child(node_view(app, &node, &tab, String::new(), cx)),
        None => body.items_center().justify_center().text_color(rgb(DIM))
            .child(if app.connected { "No terminal in this space yet" } else { "Waiting for drover-server…" }),
    }
}

/// A split draws its two sides around a gutter: drag it to resize, double-click to even it out (drag.rs).
fn node_view(app: &DroverApp, node: &Node, tab: &str, path: String, cx: &mut Context<DroverApp>) -> AnyElement {
    match node {
        Node::Pane { pane } => pane_frame(app, pane, cx).into_any_element(),
        Node::Split { split, ratio, a, b } => {
            let row = matches!(split, Dir::Row);
            let ratio = app.shown_ratio(tab, &path, *ratio).clamp(0.05, 0.95) as f32;
            let first = div().flex().min_w_0().min_h_0().flex_basis(relative(ratio)).flex_shrink_0()
                .child(node_view(app, a, tab, format!("{path}a"), cx));
            let second = div().flex().flex_1().min_w_0().min_h_0().child(node_view(app, b, tab, format!("{path}b"), cx));
            let (key, bounds) = (format!("{tab}|{path}"), app.split_bounds.clone());
            let (t, p, dir) = (tab.to_string(), path.clone(), *split);
            let gutter = div().flex_shrink_0().when(row, |d| d.w(px(8.)).cursor_col_resize()).when(!row, |d| d.h(px(8.)).cursor_row_resize())
                .hover(|d| d.bg(theme::hover()))
                .on_mouse_down(MouseButton::Left, cx.listener(move |app, ev: &gpui::MouseDownEvent, _, cx| {
                    if ev.click_count >= 2 {
                        app.save_ratio(&t, &p, 0.5);
                        cx.stop_propagation();
                    } else {
                        app.start_drag(crate::drag::Drag::Split { tab: t.clone(), path: p.clone(), dir, ratio: ratio as f64 }, cx);
                    }
                }));
            div().relative().flex().flex_1().min_w_0().min_h_0().when(row, |d| d.flex_row()).when(!row, |d| d.flex_col())
                .child(canvas(|_, _, _| {}, move |b: Bounds<Pixels>, _, _, _| { bounds.lock().unwrap().insert(key.clone(), b); })
                    .absolute().top_0().left_0().size_full())
                .child(first).child(gutter).child(second).into_any_element()
        }
    }
}

fn pane_frame(app: &DroverApp, pane_id: &str, cx: &mut Context<DroverApp>) -> Div {
    let (name, state, kind, cwd, exit) = app.store.pane(pane_id)
        .map(|p| (p.name.clone(), p.state.clone(), p.kind.clone(), p.cwd.clone(), p.exit_code)).unwrap_or_default();
    let focused = app.focused.as_deref() == Some(pane_id);
    let id = pane_id.to_string();
    let (screens, tf, switched, sizes) = (app.screens.clone(), app.term_font.clone(), app.switched_at.clone(), app.sizes.clone());
    let (commands, cursor_at, focus) = (app.client.commands.clone(), app.cursor_bounds.clone(), app.term_focus.clone());
    let marked = if focused { app.marked.clone() } else { None };
    let link = app.hover_link.as_ref().filter(|l| l.pane == pane_id).map(|l| (l.line, l.cols.clone()));
    let on_link = link.is_some();
    let term_bounds = app.term_bounds.clone();
    let (sel_down, sel_move, sel_up) = (id.clone(), id.clone(), id.clone());
    let entity = cx.entity();
    let (click_id, wheel_id) = (id.clone(), id.clone());
    let (split_id, down_id, zoom_id, close_id) = (id.clone(), id.clone(), id.clone(), id.clone());
    let hbtn = |n: &str| div().size(px(22.)).rounded(px(5.)).flex().items_center().justify_center()
        .hover(|d| d.bg(theme::hover())).child(icon(n, 13., DIM));
    div().flex().flex_col().flex_1().min_w_0().min_h_0().rounded(px(10.)).overflow_hidden()
        .border_1().when(focused, |d| d.border_color(rgb(ACCENT))).when(!focused, |d| d.border_color(theme::border())).bg(rgb(PANE))
        .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, window, cx| app.focus_pane(click_id.clone(), window, cx)))
        .on_scroll_wheel(cx.listener(move |app, ev, _, cx| app.on_wheel(&wheel_id, ev, cx)))
        .child(div().h(px(30.)).flex_none().flex().items_center().gap(px(8.)).pl(px(10.)).pr(px(6.))
            .border_b_1().border_color(theme::border()).text_size(px(12.)).font_family(theme::UI_FONT)
            .child(kind_icon(&kind))
            .child(div().flex_none().font_weight(gpui::FontWeight::MEDIUM).child(name))
            .child(div().flex_1().min_w_0().truncate().text_color(rgb(DIM)).child(short_path(&cwd)))
            .child(state_mark(&state, exit, gpui::SharedString::from(format!("ph-{pane_id}"))))
            .child(hbtn("split").on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { cx.stop_propagation(); app.split(&split_id, "row"); })))
            .child(hbtn("splitdown").on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { cx.stop_propagation(); app.split(&down_id, "col"); })))
            .child(hbtn("zoom").on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { cx.stop_propagation(); app.toggle_zoom(&zoom_id); cx.notify(); })))
            .child(hbtn("close").on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { cx.stop_propagation(); app.ask_close_pane(&close_id, cx); }))))
        .child(div().flex().flex_1().min_h_0().min_w_0().pt(px(10.)).pr(px(6.)).pb(px(6.)).pl(px(14.))
            .when(on_link, |d| d.cursor_pointer()).when(!on_link, |d| d.cursor_text())
            // Select, copy on release, open links (terminal_selection.rs).
            .on_mouse_down(MouseButton::Left, cx.listener(move |app, ev, window, cx| app.select_start(&sel_down, ev, window, cx)))
            .on_mouse_move(cx.listener(move |app, ev, window, cx| app.select_move(&sel_move, ev, window, cx)))
            .on_mouse_up(MouseButton::Left, cx.listener(move |app, ev, window, cx| app.select_end(&sel_up, ev, window, cx)))
            .child(canvas(|_, _, _| {}, move |bounds: Bounds<Pixels>, _, window, cx| {
            let started = std::time::Instant::now();
            term_bounds.lock().unwrap().insert(id.clone(), bounds);
            // Size the pane to this frame; ask the daemon only when cols x rows really change.
            let cell_w = terminal_paint::cell_width(&tf, window);
            let cols = (f32::from(bounds.size.width) / f32::from(cell_w)).floor().max(2.) as u16;
            let rows = (f32::from(bounds.size.height) / tf.line_height).floor().max(2.) as u16;
            if sizes.lock().unwrap().insert(id.clone(), (cols, rows)) != Some((cols, rows)) {
                let _ = commands.send(Command::Rpc { method: "pane.resize".into(),
                    params: json!({"pane": id, "cols": cols, "rows": rows}), reply: None });
                if let Some(s) = screens.lock().unwrap().get_mut(&id)
                    && (s.term().grid().columns(), s.term().grid().screen_lines()) != (cols as usize, rows as usize)
                {
                    s.resize(cols as usize, rows as usize);
                }
            }
            if focused {
                window.handle_input(&focus, ElementInputHandler::new(bounds, entity.clone()), cx);
            }
            if let Some(screen) = screens.lock().unwrap().get(&id) {
                let cursor = terminal_paint::paint(screen, &tf, bounds, focused, marked.as_deref(), link.clone(), window, cx);
                if focused {
                    *cursor_at.lock().unwrap() = Some(cursor);
                }
            }
            if std::env::var_os("DROVER_NATIVE_TRACE").is_some() {
                if started.elapsed().as_millis() > 20 {
                    eprintln!("trace: painting {id} took {:.1} ms", started.elapsed().as_secs_f64() * 1000.0);
                }
                if let Some(t) = switched.lock().unwrap().take() {
                    eprintln!("trace: switch painted {:.1} ms after click", t.elapsed().as_secs_f64() * 1000.0);
                }
            }
        }).size_full()))
}
