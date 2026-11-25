//! One chat's transcript: your messages, the agent's text, tool calls, permission cards, the result line, notices.

use gpui::{Context, Div, FontWeight, MouseButton, SharedString, div, prelude::*, rgb};

use super::logic::{self, DiffLine};
use super::markdown;
use super::types::{Item, Kind};
use crate::theme::{self, blocked, dim, done, error, fg, muted, working};
use crate::ui_scale::px;
use crate::views::ui::{self, Kind as Btn};
use crate::workspace_app::DroverApp;

/// Longest diff or tool result drawn in full; the rest is summed up in one line.
const MAX_LINES: usize = 30;

fn diff_box(lines: &[DiffLine], mono: &SharedString) -> Div {
    let (added, removed) = logic::diff_counts(lines);
    let mut b = div().flex().flex_col().rounded(px(6.)).bg(theme::ink(0.04)).font_family(mono.clone()).text_size(px(12.)).py(px(4.));
    for l in lines.iter().take(MAX_LINES) {
        let (color, bg) = match l.sign { '+' => (done(), theme::tint(done(), 0.12)), _ => (error(), theme::tint(error(), 0.12)) };
        b = b.child(div().flex().gap(px(8.)).px(px(8.)).bg(bg).text_color(rgb(color)).child(l.sign.to_string()).child(div().flex_1().min_w_0().child(l.text.clone())));
    }
    let hidden = lines.len().saturating_sub(MAX_LINES);
    b.when(hidden > 0, |d| d.child(div().px(px(8.)).text_color(rgb(dim())).child(format!("… {hidden} more lines"))))
        .child(div().px(px(8.)).pt(px(2.)).text_color(rgb(dim())).child(format!("+{added} −{removed}")))
}

fn text_block(text: &str, mono: &SharedString) -> Div {
    div().font_family(mono.clone()).text_size(px(12.)).text_color(rgb(muted())).p(px(8.)).rounded(px(6.)).bg(theme::ink(0.04))
        .child(text.lines().take(MAX_LINES).collect::<Vec<_>>().join("\n"))
}

fn tool_card(item: &Item, mono: &SharedString) -> Div {
    let mark = match item.status.as_str() {
        "running" => ("…", working()), "error" => ("✗", error()), "stopped" => ("■", dim()), _ => ("✓", done()),
    };
    let diff = logic::edit_diff(&item.tool, &item.input);
    let command = (item.tool == "Bash").then(|| item.input["command"].as_str().unwrap_or("").to_string());
    div().flex().flex_col().gap(px(5.))
        .child(div().flex().items_center().gap(px(8.)).text_size(px(12.5))
            .child(div().text_color(rgb(mark.1)).child(mark.0))
            .child(div().font_weight(FontWeight::MEDIUM).text_color(rgb(fg())).child(item.tool.clone()))
            .child(div().flex_1().min_w_0().truncate().font_family(mono.clone()).text_color(rgb(dim())).child(logic::tool_summary(&item.tool, &item.input))))
        .when_some(diff.filter(|d| !d.is_empty()), |d, lines| d.child(diff_box(&lines, mono)))
        .when_some(command.filter(|c| c.contains('\n')), |d, c| d.child(text_block(&c, mono)))
        .when_some(item.result.as_ref().filter(|_| item.is_error), |d, r| d.child(text_block(r, mono)))
}

fn permission_card(item: &Item, mono: &SharedString, cx: &mut Context<DroverApp>) -> Div {
    let pending = item.status == "pending";
    let diff = logic::edit_diff(&item.tool, &item.input).filter(|d| !d.is_empty());
    let command = (item.tool == "Bash").then(|| item.input["command"].as_str().unwrap_or("").to_string()).filter(|c| !c.is_empty());
    let subject = logic::tool_summary(&item.tool, &item.input);
    let ask = |label: &str, kind: Btn, allow: bool, always: bool, id: &str, cx: &mut Context<DroverApp>| {
        let id = id.to_string();
        ui::button(label, kind).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { app.answer_permission(&id, allow, always); cx.notify(); }))
    };
    div().flex().flex_col().gap(px(8.)).p(px(12.)).rounded(px(9.)).border_1()
        .border_color(if pending { rgb(blocked()) } else { theme::border2() }).bg(theme::ink(0.03))
        .child(div().text_size(px(13.5)).font_weight(FontWeight::MEDIUM).text_color(rgb(fg())).child(item.title.clone().unwrap_or_else(|| logic::permission_question(item))))
        .when_some(item.detail.clone(), |d, t| d.child(div().text_size(px(12.)).text_color(rgb(muted())).child(t)))
        .when_some(command, |d, c| d.child(text_block(&c, mono)))
        .when_some(diff, |d, lines| d.child(diff_box(&lines, mono)))
        .when(!subject.is_empty() && item.tool != "Bash", |d| d.child(div().font_family(mono.clone()).text_size(px(12.)).text_color(rgb(dim())).child(subject)))
        .child(if pending {
            div().flex().gap(px(8.)).child(ask("Allow", Btn::Primary, true, false, &item.id, cx))
                .child(ask("Always allow", Btn::Plain, true, true, &item.id, cx)).child(ask("Deny", Btn::Danger, false, false, &item.id, cx))
        } else {
            div().text_size(px(12.)).text_color(rgb(dim())).child(if item.status == "allowed" { "Allowed" } else { "Denied" })
        })
}

fn item_view(item: &Item, mono: &SharedString, cx: &mut Context<DroverApp>) -> Div {
    match item.kind {
        Kind::User => div().flex().justify_end().child(div().max_w(px(560.)).px(px(12.)).py(px(8.)).rounded(px(10.)).bg(theme::selected())
            .text_size(px(13.5)).text_color(rgb(fg())).child(item.text.clone())
            .when(item.images > 0, |d| d.child(div().pt(px(4.)).text_size(px(12.)).text_color(rgb(dim())).child(format!("{} picture(s) attached", item.images))))),
        Kind::Text => markdown::render(&item.text, mono),
        Kind::Thinking => div().text_size(px(12.5)).text_color(rgb(dim())).child(format!("Thinking… {}", item.text.lines().last().unwrap_or(""))),
        Kind::Tool => tool_card(item, mono),
        Kind::Permission => permission_card(item, mono, cx),
        Kind::Result => div().text_size(px(12.)).text_color(rgb(if item.ok { dim() } else { error() })).child(match (&item.error, item.ok) {
            (Some(e), false) => e.clone(),
            _ => format!("Done in {}{} · {} turn(s)", logic::duration(item.ms), item.cost.map(|c| format!(" · ${c:.2}")).unwrap_or_default(), item.turns),
        }),
        Kind::Notice => div().text_size(px(12.5)).text_color(rgb(if item.level == "error" { error() } else { muted() })).child(item.text.clone()),
    }
}

/// The scrolling list of items of the open chat.
pub fn transcript(app: &DroverApp, chat: &str, cx: &mut Context<DroverApp>) -> impl IntoElement {
    let items = app.chat.items_of(chat);
    if app.chat.stick.replace(false) { app.chat.scroll.scroll_to_bottom(); }
    let body = div().id("chat-transcript").flex_1().min_h_0().overflow_y_scroll().track_scroll(&app.chat.scroll)
        .flex().flex_col().gap(px(12.)).px(px(24.)).py(px(16.));
    if items.is_empty() {
        let note = if app.chat.loaded.contains(chat) { "Say something to start." } else { "Loading…" };
        return body.child(ui::note(note, false));
    }
    body.children(items.iter().map(|i| item_view(i, &app.code_family, cx)).collect::<Vec<_>>())
}
