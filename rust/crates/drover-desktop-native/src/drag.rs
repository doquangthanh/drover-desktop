//! Resizing with the mouse, as in Herdr and the web UI: the gutter between two panes (double-click evens it out),
//! the panel's left edge and the sidebar's right edge. A split is drawn with the dragged ratio while moving and
//! saved once, on release (`tab.set_layout`, as the web UI's `split-node.tsx`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use drover_core::layout_tree::set_ratio;
use drover_proto::wire::Dir;
use gpui::{Bounds, Context, MouseMoveEvent, Pixels};
use serde_json::json;

use crate::workspace_app::DroverApp;

/// Where each split box was painted, by "tab|path" (path: "" = root, then "a"/"b" steps).
pub type SplitBounds = Arc<Mutex<HashMap<String, Bounds<Pixels>>>>;

pub const PANEL: (f32, f32) = (280., 900.);
pub const SIDEBAR: (f32, f32) = (180., 480.);

pub enum Drag {
    Panel { x0: f32, w0: f32 },
    Sidebar { x0: f32, w0: f32 },
    Split { tab: String, path: String, dir: Dir, ratio: f64 },
    /// A scrollbar thumb (scrollbar.rs): `which` indexes `sidebar_scroll`, grabbed at `y0` with the list at `off0`.
    Scroll { which: usize, y0: f32, off0: f32 },
}

impl DroverApp {
    pub fn start_drag(&mut self, drag: Drag, cx: &mut Context<Self>) {
        crate::actions::trace(&format!("drag start: {}", match &drag { Drag::Panel { .. } => "panel", Drag::Sidebar { .. } => "sidebar", Drag::Split { .. } => "split", Drag::Scroll { .. } => "scroll" }));
        self.drag = Some(drag);
        cx.stop_propagation();
        cx.notify();
    }

    pub fn drag_move(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        let (x, y) = (f32::from(ev.position.x), f32::from(ev.position.y));
        match &mut self.drag {
            None => return,
            Some(Drag::Panel { x0, w0 }) => self.panel_width = (*w0 + (*x0 - x) / crate::ui_scale::zoom()).clamp(PANEL.0, PANEL.1),
            Some(Drag::Sidebar { x0, w0 }) => self.sidebar_width = (*w0 + (x - *x0) / crate::ui_scale::zoom()).clamp(SIDEBAR.0, SIDEBAR.1),
            Some(Drag::Scroll { which, y0, off0 }) => {
                let handle = &self.sidebar_scroll[*which];
                if let Some(off) = crate::scrollbar::dragged_offset(handle, *off0, y - *y0) { crate::scrollbar::scroll_to(handle, off) }
            }
            Some(Drag::Split { tab, path, dir, ratio }) => {
                let Some(b) = self.split_bounds.lock().unwrap().get(&format!("{tab}|{path}")).copied() else { return };
                let (pos, start, len) = match dir {
                    Dir::Row => (x, f32::from(b.origin.x), f32::from(b.size.width)),
                    Dir::Col => (y, f32::from(b.origin.y), f32::from(b.size.height)),
                };
                if len > 0. {
                    *ratio = (((pos - start) / len) as f64).clamp(0.1, 0.9);
                }
            }
        }
        cx.notify();
    }

    pub fn drag_end(&mut self, cx: &mut Context<Self>) {
        match self.drag.take() {
            None => return,
            Some(Drag::Split { tab, path, ratio, .. }) => self.save_ratio(&tab, &path, ratio),
            Some(_) => {}
        }
        cx.notify();
    }

    /// Store a split's ratio in the daemon: every viewer gets the new layout.
    pub fn save_ratio(&mut self, tab: &str, path: &str, ratio: f64) {
        let Some(layout) = self.store.tab(tab).and_then(|t| t.layout.clone()) else { return };
        if let Some(new) = set_ratio(Some(&layout), path, ratio) {
            self.rpc("tab.set_layout", json!({"tab": tab, "layout": new}));
        }
    }

    /// The ratio to draw a split with: the one being dragged, else the layout's.
    pub fn shown_ratio(&self, tab: &str, path: &str, ratio: f64) -> f64 {
        match &self.drag {
            Some(Drag::Split { tab: t, path: p, ratio: r, .. }) if t == tab && p == path => *r,
            _ => ratio,
        }
    }
}
