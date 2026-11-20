//! VS Code style shortcuts, as the web UI's `core/keys.ts`: one chord (Ctrl+B) or a Ctrl+K two-step chord
//! (Ctrl+K W). They reach Drover even inside a terminal (Ctrl+W closes the tab, as in VS Code); every other key goes
//! to the agent. Ctrl+P, Ctrl+Shift+P / F1, F2 and Ctrl+F open the pop-up of picker.rs.

use drover_core::layout_tree::{self, Dir, Node};
use gpui::{Bounds, Context, KeyDownEvent, Pixels, Window};
use serde_json::json;

use crate::views::View;
use crate::workspace_app::DroverApp;

/// Shown by Ctrl+K Ctrl+S (dialogs.rs).
pub const SHORTCUTS: [(&str, &str); 27] = [
    ("Ctrl+Shift+P · F1", "All commands"), ("Ctrl+P", "Jump to a terminal or agent"), ("F2", "Rename the focused agent"),
    ("Ctrl+F", "Find in the focused terminal"),
    ("Ctrl+B", "Toggle sidebar"), ("Ctrl+,", "Settings"), ("Ctrl+K Ctrl+S", "Keyboard shortcuts"),
    ("Ctrl+= · Ctrl+- · Ctrl+0", "Terminal text bigger · smaller · normal"), ("Ctrl+N · Ctrl+Shift+`", "New tab"), ("Ctrl+W", "Close tab"), ("Ctrl+Tab · Ctrl+PageDown", "Next tab"),
    ("Ctrl+Shift+Tab · Ctrl+PageUp", "Previous tab"), ("Alt+1–9", "Go to tab 1–9"),
    ("Ctrl+\\ · Ctrl+Shift+5", "Split right"), ("Ctrl+K Ctrl+\\", "Split down"), ("Ctrl+K W", "Close pane"),
    ("Ctrl+1–9", "Focus pane 1–9"), ("Ctrl+K Ctrl+←↑↓→", "Focus pane by direction"), ("Ctrl+K ← →", "Swap with previous / next pane"),
    ("Ctrl+K Shift+←↑↓→", "Resize pane (Alt: bigger steps)"), ("Ctrl+K Ctrl+M", "Zoom pane"),
    ("Ctrl+Shift+A · E · G", "Agents · Files · Changes panel"), ("Ctrl+Shift+I", "Inbox"), ("Ctrl+Shift+M", "Machines"),
    ("F8", "Next agent that needs you"), ("Shift+F8", "Next finished agent"), ("Escape", "Close a view or this sheet"),
];

impl DroverApp {
    /// Run the shortcut `ev` is, if any. True when it was one (the key then never reaches the agent).
    pub fn shortcut(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let m = &ev.keystroke.modifiers;
        let key = ev.keystroke.key.as_str();
        if matches!(key, "control" | "shift" | "alt" | "platform" | "super") {
            return self.chord;
        }
        if self.chord {
            self.chord = false; // an unknown second key is dropped, as in VS Code
            self.chord_key(key, m.control, m.shift, m.alt, window, cx);
            cx.notify();
            return true;
        }
        if m.control && !m.shift && !m.alt && key == "k" {
            self.chord = true;
            cx.notify();
            return true;
        }
        let digit = key.parse::<usize>().ok().filter(|d| (1..=9).contains(d));
        match (m.control, m.shift, m.alt, key) {
            (_, _, _, "f1") | (true, true, false, "p") => self.open_picker(crate::picker::Kind::Commands, window, cx),
            (true, false, false, "p") => self.open_picker(crate::picker::Kind::Jump, window, cx),
            (false, false, false, "f2") => self.open_picker(crate::picker::Kind::Rename, window, cx),
            (true, false, false, "f") => self.open_picker(crate::picker::Kind::Find, window, cx),
            (_, false, _, "f8") => self.next_with_state("blocked", window, cx),
            (_, true, _, "f8") => self.next_with_state("done", window, cx),
            (false, false, true, _) if digit.is_some() => self.go_to_tab(digit.unwrap_or(1) - 1, cx),
            (true, false, false, _) if digit.is_some() => self.go_to_pane(digit.unwrap_or(1) - 1, window, cx),
            (true, _, false, "=" | "+") => self.zoom_terminal(1.0),
            (true, false, false, "-" | "_") => self.zoom_terminal(-1.0),
            (true, false, false, "0") => self.zoom_terminal(0.0),
            (true, false, false, "b") => self.sidebar_open = !self.sidebar_open,
            (true, false, false, ",") => self.set_view(View::Settings, window, cx),
            (true, false, false, "n") | (true, true, false, "`" | "~") => self.new_tab(cx),
            (true, false, false, "w") => { if let Some(t) = self.tab.clone() { self.ask_close_tab(&t, cx) } }
            (true, false, false, "tab" | "pagedown") => self.step_tab(1, cx),
            (true, true, false, "tab") | (true, false, false, "pageup") => self.step_tab(-1, cx),
            (true, false, false, "\\") | (true, true, false, "5" | "%") => self.split_focused("row"),
            (true, true, false, "a") => self.toggle_panel(View::Agents, cx),
            (true, true, false, "e") => self.toggle_panel(View::Files, cx),
            (true, true, false, "g") => self.toggle_panel(View::Changes, cx),
            (true, true, false, "i") => self.set_view(View::Inbox, window, cx),
            (true, true, false, "m") => self.set_view(View::Machines, window, cx),
            _ => return false,
        }
        cx.notify();
        true
    }

    fn chord_key(&mut self, key: &str, ctrl: bool, shift: bool, alt: bool, window: &mut Window, cx: &mut Context<Self>) {
        let arrow = matches!(key, "left" | "right" | "up" | "down");
        match key {
            _ if arrow && shift => self.resize_focused(key, if alt { 0.15 } else { 0.05 }),
            _ if arrow && ctrl => self.focus_direction(key, window, cx),
            "left" => self.swap_focused(-1),
            "right" => self.swap_focused(1),
            "\\" | "|" if ctrl => self.split_focused("col"),
            "s" if ctrl => self.show_shortcuts = true,
            "m" if ctrl => { if let Some(p) = self.focused.clone() { self.toggle_zoom(&p) } }
            "w" if !ctrl => { if let Some(p) = self.focused.clone() { self.ask_close_pane(&p, cx) } }
            _ => {}
        }
    }

    fn shown_layout(&self) -> Option<(String, Node)> {
        let tab = self.tab.clone()?;
        let layout = self.store.tab(&tab)?.layout.clone()?;
        Some((tab, layout))
    }

    fn split_focused(&mut self, direction: &str) {
        if let Some(p) = self.focused.clone() {
            self.split(&p, direction);
        }
    }

    pub fn step_tab(&mut self, by: isize, cx: &mut Context<Self>) {
        let Some(space) = self.space.clone() else { return };
        let tabs: Vec<String> = self.store.tabs_of(&space).map(|t| t.id.clone())
            .filter(|t| !self.settled.contains(t) || Some(t) == self.tab.as_ref()).collect();
        let Some(i) = tabs.iter().position(|t| Some(t) == self.tab.as_ref()) else { return };
        let next = (i as isize + by).rem_euclid(tabs.len() as isize) as usize;
        self.select_tab(tabs[next].clone(), cx);
    }

    /// Put the shown tab away (Herdr's `tab.settle`): it keeps running, and comes back when shown or when an agent in
    /// it needs you. The shown tab changes to the next one still listed; the last listed tab cannot be settled.
    pub fn settle_current_tab(&mut self, cx: &mut Context<Self>) {
        let (Some(space), Some(cur)) = (self.space.clone(), self.tab.clone()) else { return };
        let others: Vec<String> = self.store.tabs_of(&space).map(|t| t.id.clone())
            .filter(|t| *t != cur && !self.settled.contains(t)).collect();
        let Some(next) = others.first().cloned() else {
            self.toasts.error("This is the only tab in the list; open another one first.".into());
            cx.notify();
            return;
        };
        self.settled.insert(cur);
        self.select_tab(next, cx);
    }

    pub fn unsettle_all(&mut self, cx: &mut Context<Self>) {
        self.settled.clear();
        cx.notify();
    }

    fn go_to_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(space) = self.space.clone() else { return };
        let tab = self.store.tabs_of(&space).filter(|t| !self.settled.contains(&t.id)).nth(index).map(|t| t.id.clone());
        if let Some(t) = tab {
            self.select_tab(t, cx);
        }
    }

    fn go_to_pane(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, layout)) = self.shown_layout() else { return };
        if let Some(p) = layout_tree::pane_ids(Some(&layout)).get(index).cloned() {
            self.focus_pane(p, window, cx);
        }
    }

    /// The nearest pane of the shown tab on that side of the focused one (by where they were painted).
    fn focus_direction(&mut self, dir: &str, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(from), Some((_, layout))) = (self.focused.clone(), self.shown_layout()) else { return };
        let boxes = self.term_bounds.lock().unwrap().clone();
        let center = |b: &Bounds<Pixels>| (f32::from(b.origin.x) + f32::from(b.size.width) / 2., f32::from(b.origin.y) + f32::from(b.size.height) / 2.);
        let Some((ax, ay)) = boxes.get(&from).map(center) else { return };
        let best = layout_tree::pane_ids(Some(&layout)).into_iter().filter(|id| *id != from)
            .filter_map(|id| boxes.get(&id).map(center).map(|c| (id, c)))
            .filter(|(_, (x, y))| match dir { "left" => *x < ax - 1., "right" => *x > ax + 1., "up" => *y < ay - 1., _ => *y > ay + 1. })
            .min_by(|(_, (x1, y1)), (_, (x2, y2))| {
                let d = |x: f32, y: f32| (x - ax).powi(2) + (y - ay).powi(2);
                d(*x1, *y1).total_cmp(&d(*x2, *y2))
            });
        if let Some((id, _)) = best {
            self.focus_pane(id, window, cx);
        }
    }

    /// Swap the focused pane with the previous / next one in the tab.
    fn swap_focused(&mut self, by: isize) {
        let (Some((tab, layout)), Some(p)) = (self.shown_layout(), self.focused.clone()) else { return };
        let ids = layout_tree::pane_ids(Some(&layout));
        let Some(i) = ids.iter().position(|x| *x == p) else { return };
        let j = i as isize + by;
        if j < 0 || j as usize >= ids.len() {
            return;
        }
        if let Some(new) = layout_tree::swap_leaves(Some(&layout), &p, &ids[j as usize]) {
            self.rpc("tab.set_layout", json!({"tab": tab, "layout": new}));
        }
    }

    /// Move the nearest split around the focused pane in that direction's axis: right / down grow the first side.
    fn resize_focused(&mut self, dir: &str, step: f64) {
        let (Some((tab, layout)), Some(p)) = (self.shown_layout(), self.focused.clone()) else { return };
        let Some(path) = layout_tree::path_to(Some(&layout), &p) else { return };
        let want = if matches!(dir, "left" | "right") { Dir::Row } else { Dir::Col };
        let sign = if matches!(dir, "right" | "down") { 1.0 } else { -1.0 };
        for cut in (0..path.len()).rev() {
            let parent = &path[..cut];
            if let Some(Node::Split { split, ratio, .. }) = node_at(&layout, parent)
                && *split == want
            {
                self.save_ratio(&tab, parent, ratio + sign * step);
                return;
            }
        }
    }

    /// F8 / Shift+F8: the next agent in that state after the focused one, shown.
    pub fn next_with_state(&mut self, state: &str, window: &mut Window, cx: &mut Context<Self>) {
        let ids: Vec<String> = self.store.panes.iter().filter(|p| p.state == state && p.kind != "shell").map(|p| p.id.clone()).collect();
        let at = self.focused.as_ref().and_then(|f| ids.iter().position(|x| x == f));
        if let Some(next) = ids.get(at.map(|i| (i + 1) % ids.len()).unwrap_or(0)).cloned() {
            self.show_pane(&next);
            self.view = View::Terminals;
            window.focus(&self.term_focus);
            cx.notify();
        }
    }
}

fn node_at<'a>(node: &'a Node, path: &str) -> Option<&'a Node> {
    let mut n = node;
    for step in path.chars() {
        let Node::Split { a, b, .. } = n else { return None };
        n = if step == 'a' { a } else { b };
    }
    Some(n)
}
