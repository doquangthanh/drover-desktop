//! Files view, right panel: the picked file read-only with line numbers (visible rows only), its diff vs HEAD when
//! git says it changed, Copy file, and Edit in browser (the web UI has the editor).

use std::sync::Arc;

use gpui::{ClipboardItem, Context, Div, MouseButton, SharedString, div, prelude::*, px, rgb, uniform_list};

use crate::theme::{self, accent, dim, fg};
use crate::views::changes::diff_view;
use crate::views::ui::{self, Kind};
use crate::workspace_app::DroverApp;

/// Text with line numbers, only the visible rows drawn (also Settings' file sections).
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

pub fn panel(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let Some(file) = app.files.file.clone() else {
        return ui::note("Pick a file on the left to read it.", false);
    };
    let changed = app.files.diff.is_some();
    let show_diff = app.files.show_diff && changed;
    let family = app.code_family.clone();
    let mut head = div().h(px(42.)).flex_none().flex().items_center().gap(px(6.)).px(px(12.)).border_b_1().border_color(theme::border())
        .child(div().flex_1().min_w_0().truncate().font_family(family.clone()).text_color(rgb(accent())).child(file.clone()));
    if changed {
        for (label, diff) in [("File", false), ("Diff", true)] {
            head = head.child(ui::tab(label, show_diff == diff).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
                app.files.show_diff = diff;
                cx.notify();
            })));
        }
    }
    let text = match &app.files.content { Some(Ok((c, _))) => c.text.clone(), _ => None };
    head = head
        .when_some(text, |d, text| d.child(ui::button("Copy file", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(move |_, _, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
        }))))
        .child(ui::button("Edit in browser", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, _| app.open_web("files"))));
    let body = if show_diff {
        match &app.files.diff {
            Some(Ok(rows)) => diff_view(rows.clone(), family),
            Some(Err(e)) => ui::note(e.clone(), true),
            None => ui::note("Loading diff…", false),
        }
    } else {
        match &app.files.content {
            None => ui::note("Loading…", false),
            Some(Err(e)) => ui::note(e.clone(), true),
            Some(Ok((c, _))) if c.error.is_some() => ui::note(c.error.clone().unwrap_or_default(), true),
            Some(Ok((c, _))) if c.binary => ui::note(format!("Binary file ({} bytes): not shown.", c.size), false),
            Some(Ok((_, lines))) => text_view(lines.clone(), family),
        }
    };
    div().flex().flex_col().flex_1().min_h_0().min_w_0().child(head).child(body)
}
