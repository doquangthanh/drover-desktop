//! Files view, left panel (`file-tree.tsx`): All (lazy folders) or Changed (git status), status badges.

use gpui::{Context, Div, FontWeight, MouseButton, div, prelude::*, rgb};
use crate::ui_scale::px;

use super::Entry;
use crate::theme::{self, accent, dim, done, error, fg, yellow};
use crate::views::ui;
use crate::widgets::icon;
use crate::workspace_app::DroverApp;

fn badge(s: &str) -> Div {
    let (text, color) = match s {
        "A" => ("A", done()),
        "D" => ("D", error()),
        "R" => ("R", accent()),
        "?" => ("U", done()),
        _ => ("M", yellow()),
    };
    div().flex_none().text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(color)).child(text)
}

fn row(app: &DroverApp, e: &Entry, depth: usize, label: String, status: Option<String>, dot: bool, cx: &mut Context<DroverApp>) -> Div {
    let open = e.dir && app.files.open.contains(&e.path);
    let on = !e.dir && app.files.file.as_deref() == Some(e.path.as_str());
    let (path, dir) = (e.path.clone(), e.dir);
    ui::row(on).h(px(26.)).pl(px(8. + depth as f32 * 14.)).text_size(px(12.5))
        .child(div().w(px(10.)).flex_none().text_color(rgb(dim())).child(if e.dir { if open { "▾" } else { "▸" } } else { "" }))
        .when(e.dir, |d| d.child(icon("folder", 13., dim())))
        .child(div().flex_1().min_w_0().truncate().text_color(rgb(fg())).child(label))
        .children(status.map(|s| badge(&s)))
        .when(dot, |d| d.child(div().size(px(5.)).rounded_full().bg(rgb(yellow()))))
        .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| app.files_pick(path.clone(), dir, cx)))
}

fn tool(label: &'static str, on: impl Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static) -> Div {
    div().size(px(24.)).flex_none().flex().items_center().justify_center().rounded(px(6.)).text_size(px(14.)).text_color(rgb(dim()))
        .hover(|d| d.bg(theme::hover())).child(label).on_mouse_down(MouseButton::Left, on)
}

pub fn panel(app: &DroverApp, name: &str, cx: &mut Context<DroverApp>) -> Div {
    let only = app.files.only_changed;
    let mut changed: Vec<(&String, &String)> = app.files.status.iter().collect();
    changed.sort();
    let seg = |label: String, value: bool, cx: &mut Context<DroverApp>| ui::tab(&label, only == value)
        .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { app.files.only_changed = value; cx.notify(); }));
    let head = div().h(px(42.)).flex_none().flex().items_center().gap(px(4.)).px(px(10.)).border_b_1().border_color(theme::border())
        .child(div().flex_1().min_w_0().truncate().font_weight(FontWeight::SEMIBOLD).child(name.to_string()))
        .child(seg("All".into(), false, cx))
        .child(seg(if changed.is_empty() { "Changed".into() } else { format!("Changed {}", changed.len()) }, true, cx))
        .child(tool("+", cx.listener(|app, _, window, cx| {
            let dir = app.files.file.as_deref().and_then(|f| f.rfind('/').map(|i| f[..=i].to_string())).unwrap_or_default();
            app.open_picker_with(crate::picker::Kind::NewFile, ("Path inside this space, e.g. src/utils.py", dir), None, window, cx);
        })))
        .child(tool("↻", cx.listener(|app, _, _, cx| { app.files_reload(); cx.notify(); })));
    let mut list = div().id("files-tree").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().p(px(6.));
    if only {
        for (p, s) in &changed {
            let dir = p.ends_with('/');
            let clean = p.trim_end_matches('/').to_string();
            let e = Entry { name: clean.clone(), path: clean.clone(), dir: false };
            if dir {
                let target = clean.clone();
                list = list.child(ui::row(false).h(px(26.)).text_size(px(12.5)).child(icon("folder", 13., dim()))
                    .child(div().flex_1().min_w_0().truncate().child(clean)).child(badge(s))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
                        app.files_reveal(&format!("{target}/x")); // opens the folders down to the untracked one
                        app.files.open.insert(target.clone());
                        if !app.files.dirs.contains_key(&target) { app.files_load_dir(&target); }
                        cx.notify();
                    })));
            } else {
                list = list.child(row(app, &e, 0, clean, Some((*s).clone()), false, cx));
            }
        }
        if changed.is_empty() {
            list = list.child(ui::note(if app.files.is_repo { "No changes — working tree is clean." } else { "Not a git repository." }, false));
        }
    } else {
        let mut stack: Vec<(Entry, usize)> = app.files.dirs.get("").cloned().unwrap_or_default().into_iter().rev().map(|e| (e, 0)).collect();
        while let Some((e, depth)) = stack.pop() {
            let status = if e.dir { app.files.status.contains_key(&format!("{}/", e.path)).then(|| "?".to_string()) }
                else { app.files.status.get(&e.path).cloned() };
            let prefix = format!("{}/", e.path);
            let dot = e.dir && status.is_none() && app.files.status.keys().any(|k| k.starts_with(&prefix));
            list = list.child(row(app, &e, depth, e.name.clone(), status, dot, cx));
            if e.dir && app.files.open.contains(&e.path) {
                for child in app.files.dirs.get(&e.path).cloned().unwrap_or_default().into_iter().rev() {
                    stack.push((child, depth + 1));
                }
            }
        }
    }
    div().flex().flex_col().flex_1().min_h_0().child(head).child(list)
}
