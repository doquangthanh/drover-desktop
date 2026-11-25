//! Files view, the open file: tabs of the open files (● unsaved), the path as a breadcrumb, File / Diff, Save,
//! Send to agent (Alt+S), Wide (Ctrl+J), the code editor (code_editor/), and a banner when the file changed on disk
//! under local edits, a save was refused for that, or a tab with edits is being closed.

use std::sync::Arc;

use gpui::{ClipboardItem, Context, Div, MouseButton, SharedString, div, prelude::*, rgb, uniform_list};
use crate::ui_scale::px;

use super::docs::key;
use crate::theme::{self, dim, fg, muted, yellow};
use crate::views::changes::diff_view;
use crate::views::ui::{self, Kind};
use crate::workspace_app::DroverApp;

/// Text with line numbers, only the visible rows drawn (Settings' file sections, a file before its editor exists).
pub fn text_view(lines: Arc<Vec<String>>, family: SharedString) -> Div {
    let width = lines.len().to_string().len() as f32 * 8. + 18.;
    let list = uniform_list("files-text", lines.len(), move |range, _, _| {
        range.map(|i| {
            div().h(px(20.)).flex().items_center().whitespace_nowrap().overflow_hidden().font_family(family.clone())
                .text_size(px(12.5)).text_color(rgb(fg()))
                .child(div().w(px(width)).flex_none().pr(px(10.)).flex().justify_end().text_color(rgb(dim())).child((i + 1).to_string()))
                .child(lines[i].clone())
        }).collect()
    }).size_full();
    div().flex_1().min_h_0().min_w_0().py(px(6.)).child(list)
}

fn tabs(app: &DroverApp, file: &str, cx: &mut Context<DroverApp>) -> gpui::Stateful<Div> {
    let mut row = div().id("file-tabs").h(px(32.)).flex_none().flex().items_end().gap(px(2.)).px(px(6.)).overflow_x_scroll()
        .border_b_1().border_color(theme::border()).bg(rgb(theme::chrome()));
    for path in app.docs.tabs.get(&app.files.space).cloned().unwrap_or_default() {
        let on = path == file;
        let dirty = app.docs.open.get(&key(&app.files.space, &path)).is_some_and(|d| d.editor.read(cx).dirty());
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();
        let (p1, p2, p3) = (path.clone(), path.clone(), path.clone());
        row = row.child(div().h(px(28.)).flex_none().flex().items_center().gap(px(6.)).pl(px(10.)).pr(px(4.)).rounded_t(px(6.))
            .text_size(px(12.)).text_color(rgb(if on { fg() } else { muted() })).when(on, |d| d.bg(rgb(theme::pane())))
            .hover(|d| d.bg(theme::hover()))
            .child(name)
            .child(div().size(px(16.)).rounded(px(4.)).flex().items_center().justify_center().text_color(rgb(if dirty { fg() } else { dim() }))
                .hover(|d| d.bg(theme::selected())).child(if dirty { "●" } else { "×" })
                .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
                    cx.stop_propagation();
                    let dirty = app.docs.open.get(&key(&app.files.space, &p1)).is_some_and(|d| d.editor.read(cx).dirty());
                    if dirty { app.docs.closing = Some(p1.clone()); cx.notify(); } else { app.files_close_tab(&p1, cx); }
                })))
            .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { app.files_show(p2.clone()); cx.notify(); }))
            .on_mouse_down(MouseButton::Middle, cx.listener(move |app, _, _, cx| {
                let dirty = app.docs.open.get(&key(&app.files.space, &p3)).is_some_and(|d| d.editor.read(cx).dirty());
                if dirty { app.docs.closing = Some(p3.clone()); cx.notify(); } else { app.files_close_tab(&p3, cx); }
            })));
    }
    row
}

/// A banner with a message and buttons (label, primary, action).
fn banner(text: String, tone: u32, buttons: Vec<(&'static str, bool, Box<dyn Fn(&mut DroverApp, &mut Context<DroverApp>)>)>, cx: &mut Context<DroverApp>) -> Div {
    let mut d = div().flex_none().flex().flex_wrap().items_center().gap(px(8.)).px(px(12.)).py(px(7.)).border_b_1().border_color(theme::border())
        .bg(theme::tint(tone, 0.10)).text_size(px(12.)).child(div().flex_1().min_w(px(160.)).text_color(rgb(fg())).child(text));
    for (label, primary, act) in buttons {
        d = d.child(ui::button(label, if primary { Kind::Primary } else { Kind::Plain })
            .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { act(app, cx); cx.notify(); })));
    }
    d
}

pub fn panel(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let Some(file) = app.files.file.clone() else {
        return ui::note("Pick a file on the left to read it.", false);
    };
    let doc = app.shown_doc();
    let dirty = doc.is_some_and(|d| d.editor.read(cx).dirty());
    let changed = app.files.diff.is_some();
    let show_diff = app.files.show_diff && changed;
    let family = app.code_family.clone();
    let crumbs = file.split('/').collect::<Vec<_>>().join("  ›  ");
    let back = ui::back_header("Files", crumbs, cx.listener(|app, _, _, cx| { app.files_close_file(); cx.notify(); }));
    let mut head = div().min_h(px(38.)).flex_none().flex().flex_wrap().items_center().gap(px(6.)).px(px(12.)).py(px(4.))
        .border_b_1().border_color(theme::border());
    if changed {
        for (label, diff) in [("File", false), ("Diff", true)] {
            head = head.child(ui::tab(label, show_diff == diff).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
                app.files.show_diff = diff;
                cx.notify();
            })));
        }
    }
    let (f1, f2) = (file.clone(), file.clone());
    let text = match &app.files.content { Some(Ok((c, _))) => c.text.clone(), _ => None };
    head = head.child(div().flex_1())
        .when(dirty, |d| d.child(ui::button("Save", Kind::Primary).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| app.files_save(&f1, false, cx)))))
        .when(doc.is_some(), |d| d.child(ui::button("Send to agent", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.files_send_selection(cx)))))
        .child(ui::button(if app.docs.wide { "Narrow" } else { "Wide" }, Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| {
            app.docs.wide = !app.docs.wide;
            cx.notify();
        })))
        .when_some(text, |d, text| d.child(ui::button("Copy file", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(move |_, _, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
        }))));
    let mut banners = Vec::new();
    if let Some(closing) = app.docs.closing.clone() {
        let c = closing.clone();
        banners.push(banner(format!("{closing} has changes that are not saved."), yellow(), vec![
            ("Close and discard", true, Box::new(move |app: &mut DroverApp, cx: &mut Context<DroverApp>| { app.docs.closing = None; app.files_close_tab(&c, cx); })),
            ("Cancel", false, Box::new(|app: &mut DroverApp, _: &mut Context<DroverApp>| app.docs.closing = None)),
        ], cx));
    }
    if let Some(d) = doc.filter(|d| d.conflict || d.stale) {
        let (a, b) = (f2.clone(), f2.clone());
        let msg = if d.conflict { "Not saved: something else (an agent?) changed this file after you opened it." } else { "This file changed on disk, and you have edits that are not saved." };
        banners.push(banner(msg.into(), crate::theme::error(), vec![
            ("Overwrite with mine", d.conflict, Box::new(move |app: &mut DroverApp, cx: &mut Context<DroverApp>| app.files_save(&a, true, cx))),
            ("Reload from disk", false, Box::new(move |app: &mut DroverApp, cx: &mut Context<DroverApp>| app.files_reload_doc(&b, cx))),
        ], cx));
    }
    let body = if show_diff {
        match &app.files.diff {
            Some(Ok(rows)) => diff_view(rows.clone(), family),
            Some(Err(e)) => ui::note(e.clone(), true),
            None => ui::note("Loading diff…", false),
        }
    } else {
        match (&app.files.content, doc) {
            (None, _) => ui::note("Loading…", false),
            (Some(Err(e)), _) => ui::note(e.clone(), true),
            (Some(Ok((c, _))), _) if c.error.is_some() => ui::note(c.error.clone().unwrap_or_default(), true),
            (Some(Ok((c, _))), _) if c.binary => ui::note(format!("Binary file ({} bytes): not shown.", c.size), false),
            (Some(Ok(_)), Some(d)) => div().flex().flex_col().flex_1().min_h_0().min_w_0().child(d.editor.clone()),
            (Some(Ok((_, lines))), None) => text_view(lines.clone(), family),
        }
    };
    let readonly = (!app.files.editable && doc.is_some()).then(|| div().flex_none().px(px(12.)).py(px(4.)).text_size(px(11.5)).text_color(rgb(dim()))
        .child("Read-only: the file is over 1 MB or not UTF-8."));
    div().flex().flex_col().flex_1().min_h_0().min_w_0().child(back).child(tabs(app, &file, cx)).child(head).children(banners).children(readonly).child(body)
}
