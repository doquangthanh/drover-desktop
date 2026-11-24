//! Changes panel, the review loop and checks, drawing: the checks strip (one chip per step, its output, Run / Stop),
//! the review bar (progress, Send comments to the agent) and one file's review diff: hunks with Mark reviewed /
//! Reject hunk, `+` on a line to comment, the comments under their line. F7 / Shift+F7 move between hunks, A marks
//! the current one reviewed (click the diff first so it has the keyboard).

use gpui::{Context, Div, Focusable, FontWeight, MouseButton, SharedString, div, prelude::*, rgb};
use serde_json::{Value, json};

use super::review::busy;
use crate::theme::{self, accent, dim, done, error, fg, muted, tint, yellow};
use crate::ui_scale::px;
use crate::views::ui::{self, Kind};
use crate::workspace_app::DroverApp;

fn mark(status: &str) -> &'static str {
    match status { "passed" => "✓", "failed" | "error" => "✗", "timeout" => "⏱", "running" => "…", "cancelled" => "–", _ => "·" }
}

pub fn checks_strip(app: &DroverApp, cx: &mut Context<DroverApp>) -> Option<Div> {
    let run = app.changes.review.checks.as_ref()?;
    let results = run["results"].as_array().cloned().unwrap_or_default();
    let mut row = div().flex().flex_wrap().items_center().gap(px(6.)).child(div().text_color(rgb(dim())).child("Checks"));
    if results.is_empty() { row = row.child(div().text_color(rgb(dim())).child("not run yet")); }
    for r in &results {
        let (name, status) = (r["name"].as_str().unwrap_or("").to_string(), r["status"].as_str().unwrap_or(""));
        let color = match status { "passed" => done(), "failed" | "error" | "timeout" => error(), _ => muted() };
        let open = app.changes.review.check_open.as_deref() == Some(&name);
        let ms = r["ms"].as_u64().filter(|m| *m > 0).map(|m| format!(" {:.1}s", m as f64 / 1000.)).unwrap_or_default();
        let n = name.clone();
        row = row.child(div().px(px(7.)).py(px(2.)).rounded(px(5.)).border_1().border_color(if open { theme::tint(accent(), 0.6) } else { theme::border2() })
            .text_color(rgb(color)).child(format!("{} {name}{ms}", mark(status)))
            .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
                let r = &mut app.changes.review.check_open;
                *r = if r.as_deref() == Some(&n) { None } else { Some(n.clone()) };
                cx.notify();
            })));
    }
    let (label, method) = if busy(run) { ("Stop", "check.cancel") } else { ("Run checks", "check.run") };
    row = row.child(div().flex_1()).child(ui::button(label, Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
        app.review_call(method, json!({}), cx);
        cx.notify();
    })));
    let output = app.changes.review.check_open.as_ref().and_then(|n| results.iter().find(|r| r["name"].as_str() == Some(n)))
        .and_then(|r| r["output"].as_str()).filter(|o| !o.is_empty()).map(|o| {
            let tail: String = o.lines().rev().take(40).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
            div().id("check-out").max_h(px(220.)).overflow_y_scroll().p(px(8.)).rounded(px(6.)).bg(theme::ink(0.04))
                .font_family(app.code_family.clone()).text_size(px(11.5)).text_color(rgb(fg())).whitespace_normal().child(tail)
        });
    Some(div().flex_none().flex().flex_col().gap(px(6.)).px(px(12.)).py(px(7.)).border_b_1().border_color(theme::border()).text_size(px(12.))
        .child(row).children(output))
}

pub fn review_bar(app: &DroverApp, cx: &mut Context<DroverApp>) -> Option<Div> {
    let s = app.changes.review.summary.as_ref()?;
    let (hunks, reviewed, open) = (s["hunks"].as_u64()?, s["reviewed"].as_u64().unwrap_or(0), s["open_comments"].as_u64().unwrap_or(0));
    if hunks == 0 { return None }
    let mut text = format!("{reviewed}/{hunks} hunks reviewed");
    if open > 0 { text += &format!(" · {open} open comment{}", if open == 1 { "" } else { "s" }); }
    let label = if open == 1 { "Send 1 comment to the agent".to_string() } else { format!("Send {open} comments to the agent") };
    Some(div().flex_none().flex().flex_wrap().items_center().gap(px(8.)).px(px(12.)).py(px(7.)).border_b_1().border_color(theme::border()).text_size(px(12.))
        .child(div().text_color(rgb(dim())).child("Review")).child(div().text_color(rgb(fg())).child(text)).child(div().flex_1())
        .when(open > 0, |d| d.child(ui::button(&label, Kind::Primary).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| {
            app.review_call("review.send", json!({"pane": ""}), cx);
            cx.notify();
        })))))
}

fn comment_card(c: &Value, show_line: bool, cx: &mut Context<DroverApp>) -> Div {
    let (id, status) = (c["id"].as_str().unwrap_or("").to_string(), c["status"].as_str().unwrap_or("open").to_string());
    let tone = match status.as_str() { "open" => accent(), "sent" => done(), "outdated" => error(), _ => dim() };
    let act = |label: &'static str, method: &'static str, extra: Value, id: String, cx: &mut Context<DroverApp>| ui::button(label, Kind::Plain)
        .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
            let mut p = extra.clone();
            p["id"] = json!(id);
            app.review_call(method, p, cx);
            cx.notify();
        }));
    let (eid, etext, side, line) = (id.clone(), c["text"].as_str().unwrap_or("").to_string(), c["side"].as_str().unwrap_or("new").to_string(), c["line"].as_u64().unwrap_or(0));
    let resolve = if status == "resolved" { act("Reopen", "review.comment.resolve", json!({"resolved": false}), id.clone(), cx) }
                  else { act("Resolve", "review.comment.resolve", json!({}), id.clone(), cx) };
    div().mx(px(8.)).my(px(4.)).p(px(8.)).flex().flex_col().gap(px(5.)).rounded(px(7.)).border_1().border_color(theme::border2())
        .bg(rgb(theme::card())).text_size(px(12.)).when(status == "resolved", |d| d.opacity(0.6))
        .child(div().flex().flex_wrap().items_center().gap(px(6.))
            .child(div().px(px(6.)).rounded(px(4.)).bg(tint(tone, 0.15)).text_color(rgb(tone)).child(status.clone()))
            .when(show_line, |d| d.child(div().text_color(rgb(dim())).child(format!("{}line {line}", if side == "old" { "old " } else { "" }))))
            .child(div().flex_1())
            .child(resolve)
            .child(ui::button("Edit", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, window, cx| {
                app.changes.review.draft = Some((side.clone(), line, Some(eid.clone())));
                app.changes_in.comment.update(cx, |i, cx| i.set_text(&etext, cx));
                window.focus(&app.changes_in.comment.read(cx).focus_handle(cx));
                cx.notify();
            })))
            .child(act("Delete", "review.comment.delete", json!({}), id, cx)))
        .child(div().text_color(rgb(fg())).child(c["text"].as_str().unwrap_or("").to_string()))
        .when(status == "outdated", |d| d.child(div().text_color(rgb(dim())).truncate().child(format!("was: {}", c["anchor"].as_str().unwrap_or("").trim()))))
}

fn draft_box(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    div().mx(px(8.)).my(px(4.)).p(px(8.)).flex().flex_col().gap(px(6.)).rounded(px(7.)).border_1().border_color(theme::tint(accent(), 0.5))
        .child(div().h(px(76.)).flex().child(app.changes_in.comment.clone()))
        .child(div().flex().items_center().gap(px(6.)).text_size(px(11.5)).child(div().text_color(rgb(dim())).child("Ctrl+S to add")).child(div().flex_1())
            .child(ui::button("Cancel", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| { app.changes.review.draft = None; cx.notify(); })))
            .child(ui::button("Comment", Kind::Primary).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.review_submit_draft(cx)))))
}

/// One file's review diff, or None when the daemon has no review.* (the caller shows the plain diff).
pub fn file_review(app: &DroverApp, cx: &mut Context<DroverApp>) -> Option<Div> {
    let r = &app.changes.review;
    if r.unsupported { return None }
    let diff = match &r.diff { None => return Some(ui::note("Loading review…", false)), Some(Err(e)) => return Some(ui::note(e.clone(), true)), Some(Ok(d)) => d };
    if diff["binary"].as_bool().unwrap_or(false) { return Some(ui::note("Binary file: nothing to review line by line.", false)) }
    let hunks = diff["hunks"].as_array().cloned().unwrap_or_default();
    if hunks.is_empty() { return Some(ui::note("No changes in this file.", false)) }
    let family: SharedString = app.code_family.clone();
    let key = |side: &str, line: u64| format!("{side}:{line}");
    let mut shown_keys = std::collections::HashSet::new();
    let mut body = div().flex().flex_col();
    for (n, h) in hunks.iter().enumerate() {
        let (index, reviewed) = (h["index"].as_u64().unwrap_or(n as u64), h["reviewed"].as_bool().unwrap_or(false));
        let ask = r.reject_ask == Some(index);
        let head = div().h(px(30.)).flex().items_center().gap(px(6.)).px(px(10.)).bg(tint(accent(), 0.08))
            .when(n == r.cur.min(hunks.len() - 1), |d| d.border_l_2().border_color(rgb(accent())))
            .child(div().flex_1().min_w_0().truncate().font_family(family.clone()).text_color(rgb(accent())).child(h["header"].as_str().unwrap_or("").to_string()))
            .child(ui::button(if reviewed { "✓ Reviewed" } else { "Mark reviewed" }, if reviewed { Kind::Primary } else { Kind::Plain })
                .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { app.review_call("review.hunk.accept", json!({"index": index, "accepted": !reviewed}), cx); cx.notify(); })))
            .child(ui::button(if ask { "Click again to reject" } else { "Reject hunk" }, Kind::Plain)
                .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
                    // Rejecting puts the base version back in the working tree: ask with a second click.
                    if app.changes.review.reject_ask == Some(index) { app.changes.review.reject_ask = None; app.review_call("review.hunk.reject", json!({"index": index}), cx); }
                    else { app.changes.review.reject_ask = Some(index); }
                    cx.notify();
                })));
        body = body.child(head);
        for l in h["lines"].as_array().cloned().unwrap_or_default() {
            let k = l["k"].as_str().unwrap_or(" ").to_string();
            let target = if k == "-" { l["o"].as_u64().map(|o| ("old", o)) } else { l["n"].as_u64().map(|n| ("new", n)) };
            let bg = match k.as_str() { "+" => Some(tint(done(), 0.12)), "-" => Some(tint(error(), 0.12)), _ => None };
            let num = |v: &Value| v.as_u64().map(|n| n.to_string()).unwrap_or_default();
            let mut row = div().min_h(px(20.)).flex().items_center().font_family(family.clone()).text_size(px(12.)).when_some(bg, |d, bg| d.bg(bg))
                .child(div().w(px(38.)).flex_none().pr(px(6.)).flex().justify_end().text_color(rgb(dim())).child(num(&l["o"])))
                .child(div().w(px(38.)).flex_none().pr(px(6.)).flex().justify_end().text_color(rgb(dim())).child(num(&l["n"])));
            row = match target {
                Some((side, line)) => row.child(div().w(px(18.)).flex_none().text_color(rgb(accent())).hover(|d| d.bg(theme::hover())).child("+")
                    .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, window, cx| {
                        app.changes.review.draft = Some((side.to_string(), line, None));
                        app.changes_in.comment.update(cx, |i, cx| i.set_text("", cx));
                        window.focus(&app.changes_in.comment.read(cx).focus_handle(cx));
                        cx.notify();
                    }))),
                None => row.child(div().w(px(18.)).flex_none()),
            };
            body = body.child(row.child(div().flex_1().min_w_0().whitespace_nowrap().overflow_hidden().text_color(rgb(fg()))
                .child(format!("{k}{}", l["t"].as_str().unwrap_or("")))));
            if let Some((side, line)) = target {
                shown_keys.insert(key(side, line));
                for c in r.comments.iter().filter(|c| c["status"] != "outdated" && c["side"] == side && c["line"].as_u64() == Some(line)) {
                    body = body.child(comment_card(c, false, cx));
                }
                if r.draft.as_ref().is_some_and(|(s, l, e)| s == side && *l == line && e.is_none()) { body = body.child(draft_box(app, cx)); }
            }
        }
    }
    let loose: Vec<&Value> = r.comments.iter().filter(|c| c["status"] == "outdated"
        || !shown_keys.contains(&key(c["side"].as_str().unwrap_or(""), c["line"].as_u64().unwrap_or(0)))).collect();
    let mut top = div().flex().flex_col();
    if !loose.is_empty() {
        top = top.child(div().px(px(12.)).pt(px(8.)).text_size(px(11.5)).text_color(rgb(yellow())).font_weight(FontWeight::SEMIBOLD).child("Comments whose line is no longer in this diff"));
        for c in loose { top = top.child(comment_card(c, true, cx)); }
    }
    if r.draft.as_ref().is_some_and(|(_, _, e)| e.is_some()) { top = top.child(draft_box(app, cx)); }
    let focus = app.changes_in.review_focus.clone();
    Some(div().id("review-diff").flex_1().min_h_0().overflow_y_scroll().track_focus(&focus)
        .on_key_down(cx.listener(|app, ev: &gpui::KeyDownEvent, window, cx| {
            // Only when the diff itself has the keyboard: an A typed in the comment field is text.
            if !app.changes_in.review_focus.is_focused(window) { return }
            let m = &ev.keystroke.modifiers;
            if !m.control && !m.alt && app.review_key(&ev.keystroke.key, m.shift, cx) { cx.stop_propagation(); }
        }))
        .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, window, cx| { window.focus(&app.changes_in.review_focus); cx.notify(); }))
        .child(top).child(body).into_any_element()).map(|e| div().flex().flex_col().flex_1().min_h_0().child(e))
}
