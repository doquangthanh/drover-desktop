//! Linux: the window has no system title bar (client-side decorations, main.rs), so it also has no system resize
//! border. These invisible strips on the edges and corners start a resize by the window manager. The window buttons
//! and the drag area are in topbar.rs, shared with Windows.

use gpui::{Context, Div, MouseButton, ResizeEdge, div, prelude::*};
use crate::ui_scale::px;

use crate::workspace_app::DroverApp;

pub fn resize_edges(cx: &mut Context<DroverApp>) -> Vec<Div> {
    let edge = |e: ResizeEdge, d: Div| d.absolute()
        .on_mouse_down(MouseButton::Left, cx.listener(move |_, _, w, _| w.start_window_resize(e)));
    let (t, c) = (px(4.), px(10.));
    vec![
        edge(ResizeEdge::Top, div().top_0().left(c).right(c).h(t).cursor_ns_resize()),
        edge(ResizeEdge::Bottom, div().bottom_0().left(c).right(c).h(t).cursor_ns_resize()),
        edge(ResizeEdge::Left, div().left_0().top(c).bottom(c).w(t).cursor_ew_resize()),
        edge(ResizeEdge::Right, div().right_0().top(c).bottom(c).w(t).cursor_ew_resize()),
        edge(ResizeEdge::TopLeft, div().top_0().left_0().size(c).cursor_nwse_resize()),
        edge(ResizeEdge::BottomRight, div().bottom_0().right_0().size(c).cursor_nwse_resize()),
        edge(ResizeEdge::TopRight, div().top_0().right_0().size(c).cursor_nesw_resize()),
        edge(ResizeEdge::BottomLeft, div().bottom_0().left_0().size(c).cursor_nesw_resize()),
    ]
}
