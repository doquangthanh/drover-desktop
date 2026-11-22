//! Peek: look at another pane's terminal in a pop-up without leaving the one you work in. Nothing about where you
//! are changes: the focused pane, space and tab stay, the pane is not resized (it is drawn at its own cols x rows)
//! and the daemon is not told it was seen. Enter (or a click on the card) goes there; Escape, Space or a click
//! outside closes it. Opened with Ctrl+K P (a picker like Ctrl+P), Alt+click on an agent in the sidebar, or the
//! agent's right-click menu.

use alacritty_terminal::grid::Dimensions;
use drover_proto::client::Command;
use gpui::{Bounds, Context, Div, FontWeight, MouseButton, Pixels, Size, Window, canvas, div, prelude::*, rgb, rgba};

use crate::theme::{self, dim, fg, muted, UI_FONT};
use crate::ui_scale::px;
use crate::widgets::{kind_icon, short_path, state_mark};
use crate::workspace_app::DroverApp;

impl DroverApp {
    /// Show `pane` in the peek pop-up. Its output is attached first if this window never showed it.
    pub fn open_peek(&mut self, pane: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.store.pane(pane).is_none() {
            return;
        }
        if !self.screens.lock().unwrap().contains_key(pane) {
            let _ = self.client.commands.send(Command::Attach(pane.to_string()));
        }
        self.menu = None;
        self.peek = Some(pane.to_string());
        // Keys come to the app (on_key) so Enter / Escape reach the pop-up, even over a view with a field.
        window.focus(&self.term_focus);
        cx.notify();
    }

    pub fn close_peek(&mut self, cx: &mut Context<Self>) {
        self.peek = None;
        self.want_term_focus = true;
        cx.notify();
    }

    /// Leave for the peeked pane: its space and tab are shown and keys go to it.
    pub fn peek_go(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.peek.take() {
            self.show_pane(&id);
            window.focus(&self.term_focus);
        }
        cx.notify();
    }

    /// A key while peeking: Enter goes there, Escape / Space close; nothing reaches the focused pane meanwhile.
    pub fn peek_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        match key {
            "enter" => self.peek_go(window, cx),
            "escape" | "space" => self.close_peek(cx),
            _ => {}
        }
    }
}

/// Size of the terminal area: the pane's grid in pixels, kept inside `max` (a bigger grid is clipped).
fn term_size(grid: Option<(usize, usize)>, cell: (f32, f32), max: (f32, f32)) -> (f32, f32) {
    let (cols, rows) = grid.unwrap_or((80, 24));
    ((cols as f32 * cell.0).min(max.0).max(200.), (rows as f32 * cell.1).min(max.1).max(80.))
}

pub fn overlay(app: &DroverApp, view: Size<Pixels>, cx: &mut Context<DroverApp>) -> Option<Div> {
    let id = app.peek.clone()?;
    let p = app.store.pane(&id)?;
    let space = app.store.spaces.iter().find(|s| s.id == p.space_id).map(|s| s.name.as_str()).unwrap_or("");
    let tab = app.store.tab(&p.tab_id).map(|t| t.name.as_str()).unwrap_or("");
    let grid = app.screens.lock().unwrap().get(&id).map(|s| (s.term().grid().columns(), s.term().grid().screen_lines()));
    let max = (f32::from(view.width) * 0.92 - 28., f32::from(view.height) * 0.85 - f32::from(px(60.)));
    let (w, h) = term_size(grid, app.cell_size, max);

    let head = div().h(px(34.)).flex_none().flex().items_center().gap(px(8.)).px(px(12.))
        .border_b_1().border_color(theme::border()).text_size(px(12.))
        .child(kind_icon(&p.kind))
        .child(div().flex_none().font_weight(FontWeight::MEDIUM).text_color(rgb(fg())).child(p.name.clone()))
        .child(div().flex_1().min_w_0().truncate().text_color(rgb(dim())).child(format!("{space} · {tab} · {}", short_path(&p.cwd))))
        .child(state_mark(&p.state, p.exit_code, gpui::SharedString::from(format!("pe-{id}"))))
        .child(div().flex_none().text_size(px(11.5)).text_color(rgb(muted())).child("Enter: go there · Esc: close"));

    let (screens, graphics, tf, paint_id) = (app.screens.clone(), app.graphics.clone(), app.term_font.clone(), id.clone());
    let body = match grid {
        // Drawn read-only at the pane's own size: no pane.resize, so the program in it is not disturbed.
        Some(_) => div().child(canvas(|_, _, _| {}, move |bounds: Bounds<Pixels>, _, window, cx| {
            if let Some(screen) = screens.lock().unwrap().get(&paint_id) {
                let images = graphics.lock().unwrap();
                crate::terminal_paint::paint(screen, &tf, bounds, false, None, None, images.get(&paint_id), window, cx);
            }
        }).size_full()),
        None => div().flex().items_center().justify_center().text_color(rgb(dim())).child("Loading…"),
    };

    let card = div().flex().flex_col().rounded(px(12.)).overflow_hidden().bg(rgb(theme::pane()))
        .border_1().border_color(rgb(theme::accent())).shadow_lg().font_family(UI_FONT)
        .child(head)
        .child(div().p(gpui::px(14.)).child(body.w(gpui::px(w)).h(gpui::px(h)).overflow_hidden()))
        .on_mouse_down(MouseButton::Left, cx.listener(|app, _, window, cx| { cx.stop_propagation(); app.peek_go(window, cx); }));
    Some(div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().bg(rgba(0x00000073))
        .occlude().child(card)
        .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.close_peek(cx)))
        .on_mouse_down(MouseButton::Right, cx.listener(|app, _, _, cx| app.close_peek(cx))))
}

#[cfg(test)]
mod tests {
    use super::term_size;

    #[test]
    fn grid_size_fits_inside_the_window() {
        assert_eq!(term_size(Some((100, 30)), (8., 17.), (2000., 2000.)), (800., 510.));
        assert_eq!(term_size(Some((300, 80)), (8., 17.), (1000., 600.)), (1000., 600.));
        assert_eq!(term_size(None, (8., 17.), (2000., 2000.)), (640., 408.));
    }
}
