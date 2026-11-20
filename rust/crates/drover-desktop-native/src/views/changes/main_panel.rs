//! Changes view, right side (`changes-desktop.tsx`, `diff-view.tsx`): branch vs base, Commit / Rebase / Push & PR,
//! the last action's output, the changed files and the selected file's diff (only visible rows are drawn).

use std::sync::Arc;

use drover_proto::changes::{self, DiffRow, RowKind, Worktree};
use gpui::{Context, Div, MouseButton, SharedString, div, prelude::*, px, rgb, uniform_list};
use serde_json::json;

use crate::theme::{self, accent, dim, done, error, fg, muted, yellow, tint};
use crate::views::ui::{self, Kind};
use crate::workspace_app::DroverApp;

fn status_color(s: &str) -> u32 {
    match s {
        "A" | "?" => done(),
        "D" => error(),
        "R" => accent(),
        _ => yellow(),
    }
}

/// A parsed diff, numbered, coloured, only the visible rows drawn (also the Files view's Diff tab).
pub fn diff_view(rows: Arc<Vec<DiffRow>>, family: SharedString) -> Div {
    if rows.is_empty() {
        return ui::note("No changes in this file.", false);
    }
    let list = uniform_list("changes-diff", rows.len(), move |range, _, _| {
        range.map(|i| {
            let r = &rows[i];
            let (bg, fg) = match r.kind {
                RowKind::Hunk => (Some(tint(accent(), 0.08)), accent()),
                RowKind::Added => (Some(tint(done(), 0.12)), fg()),
                RowKind::Removed => (Some(tint(error(), 0.12)), fg()),
                RowKind::Context => (None, muted()),
            };
            div().h(px(20.)).flex().items_center().whitespace_nowrap().overflow_hidden().font_family(family.clone())
                .text_size(px(12.5)).text_color(rgb(fg)).when_some(bg, |d, bg| d.bg(bg))
                .child(div().w(px(52.)).flex_none().pr(px(10.)).flex().justify_end().text_color(rgb(dim()))
                    .child(r.n.map(|n| n.to_string()).unwrap_or_default()))
                .child(r.text.clone())
        }).collect()
    }).size_full();
    div().flex_1().min_h_0().min_w_0().py(px(6.)).child(list)
}

pub fn panel(app: &DroverApp, current: Option<&Worktree>, base: &str, file: Option<&str>, cx: &mut Context<DroverApp>) -> Div {
    let branch = current.and_then(|t| t.branch.clone()).unwrap_or_default();
    let path = current.map(|t| t.path.clone()).unwrap_or_default();
    let mut head = div().min_h(px(46.)).flex_none().flex().flex_wrap().items_center().gap(px(8.)).px(px(12.)).py(px(8.))
        .border_b_1().border_color(theme::border())
        .child(div().text_color(rgb(accent())).font_family(app.code_family.clone()).child(branch.clone()))
        .child(div().text_color(rgb(dim())).child("vs"))
        .child(div().w(px(150.)).child(app.changes_in.base.clone()))
        .child(div().flex_1())
        .child(div().w(px(220.)).child(app.changes_in.commit.clone()))
        .child(ui::button("Commit", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.changes_commit(cx))));
    if !branch.is_empty() && branch != base {
        let (p, b) = (path.clone(), base.to_string());
        head = head.child(ui::button(&format!("Rebase on {base}"), Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
            app.changes_run("Rebase", "git.rebase", json!({"path": p, "base": b}));
            cx.notify();
        })));
    }
    head = head.child(ui::button("Push & open PR", Kind::Primary).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| {
        app.changes_push_pr(cx);
        cx.notify();
    })));
    let output = app.changes.output.clone().map(|(text, error)| {
        div().flex_none().flex().items_start().gap(px(8.)).px(px(12.)).py(px(8.)).border_b_1().border_color(theme::border())
            .bg(tint(if error { crate::theme::error() } else { done() }, 0.08)).text_size(px(12.))
            .child(div().flex_1().min_w_0().text_color(rgb(if error { crate::theme::error() } else { fg() })).child(text))
            .child(div().text_color(rgb(dim())).child("✕").on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| {
                app.changes.output = None;
                cx.notify();
            })))
    });
    let mut files = div().id("changes-files").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().p(px(6.));
    match &app.changes.files {
        None => files = files.child(ui::note("Loading…", false)),
        Some(Err(e)) => files = files.child(ui::note(e.clone(), true)),
        Some(Ok(list)) if list.is_empty() => files = files.child(ui::note(format!("No changes vs {base}"), false)),
        Some(Ok(list)) => {
            for f in list {
                let (name, dir) = changes::split_path(&f.path);
                let status = f.status.clone().unwrap_or("M".into());
                let id = f.path.clone();
                files = files.child(ui::row(file == Some(f.path.as_str())).text_size(px(12.5))
                    .child(div().w(px(14.)).flex_none().text_color(rgb(status_color(&status))).child(status))
                    .child(div().flex_1().min_w_0().flex().gap(px(5.)).overflow_hidden()
                        .child(div().flex_none().child(name.to_string()))
                        .child(div().min_w_0().truncate().text_color(rgb(dim())).child(dir.to_string())))
                    .child(div().flex_none().text_color(rgb(done())).child(format!("+{}", f.added)))
                    .when(f.removed > 0, |d| d.child(div().flex_none().text_color(rgb(error())).child(format!("-{}", f.removed))))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
                        app.changes.picked_file = Some(id.clone());
                        cx.notify();
                    })));
            }
        }
    }
    let diff = match (&app.changes.diff, file) {
        (_, None) => div().flex_1(),
        (None, _) => ui::note("Loading diff…", false),
        (Some(Err(e)), _) => ui::note(e.clone(), true),
        (Some(Ok(rows)), _) => diff_view(rows.clone(), app.code_family.clone()),
    };
    // Narrow panel: the files on top, the diff under them.
    let body = div().flex().flex_col().flex_1().min_h_0()
        .child(div().max_h(px(220.)).flex_none().flex().flex_col().border_b_1().border_color(theme::border()).child(files))
        .child(diff);
    div().flex().flex_col().flex_1().min_h_0().min_w_0().child(head).children(output).child(body)
}
