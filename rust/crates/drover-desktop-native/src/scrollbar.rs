//! A thin scrollbar for a list that scrolls (gpui draws none): the thumb is sized and placed from the list's
//! ScrollHandle and dragged with the mouse (drag.rs `Drag::Scroll`). Shown only when the list is longer than its box.
//! Sizes here are window pixels (what ScrollHandle reports), so they use gpui::px, not the app-zoomed ui_scale::px.

use gpui::{Context, Div, MouseButton, ScrollHandle, div, point, prelude::*, px};

use crate::drag::Drag;
use crate::theme;
use crate::workspace_app::DroverApp;

/// Shortest thumb, so a very long list still has something to grab.
const MIN_THUMB: f32 = 24.;

/// (box height, how far it can scroll, thumb height); None when everything fits.
pub fn geometry(handle: &ScrollHandle) -> Option<(f32, f32, f32)> {
    let view = f32::from(handle.bounds().size.height);
    let max = f32::from(handle.max_offset().height);
    (view > 0. && max > 0.5).then(|| (view, max, (view * view / (view + max)).max(MIN_THUMB).min(view)))
}

/// The scroll offset for a thumb dragged `dy` pixels from where it was grabbed at offset `from`.
pub fn dragged_offset(handle: &ScrollHandle, from: f32, dy: f32) -> Option<f32> {
    let (view, max, thumb) = geometry(handle)?;
    Some((from + dy * max / (view - thumb).max(1.)).clamp(0., max))
}

pub fn scroll_to(handle: &ScrollHandle, offset: f32) {
    handle.set_offset(point(px(0.), px(-offset)));
}

/// `list` (which must `track_scroll(handle)`) with its scrollbar on the right edge. `which` names the handle for
/// drag.rs; a wheel turn redraws so the thumb follows.
pub fn with_scrollbar(list: impl IntoElement, handle: &ScrollHandle, which: usize, cx: &mut Context<DroverApp>) -> Div {
    let bar = geometry(handle).map(|(view, max, thumb)| {
        let off = (-f32::from(handle.offset().y)).clamp(0., max);
        let top = off / max * (view - thumb);
        div().absolute().top_0().right(px(1.)).w(px(7.)).h_full()
            .child(div().absolute().top(px(top)).h(px(thumb)).w_full().rounded_full().bg(theme::ink(0.22))
                .hover(|d| d.bg(theme::ink(0.38)))
                .on_mouse_down(MouseButton::Left, cx.listener(move |app, ev: &gpui::MouseDownEvent, _, cx| {
                    app.start_drag(Drag::Scroll { which, y0: f32::from(ev.position.y), off0: off }, cx);
                })))
    });
    div().relative().flex_1().min_h_0().flex().flex_col().child(list).children(bar)
        .on_scroll_wheel(cx.listener(|_, _, _, cx| cx.notify()))
}
