//! The Chat view: the list of chats on the left; on the right the open chat's transcript and its message box.

use gpui::{Context, Div, FontWeight, MouseButton, MouseDownEvent, div, prelude::*, rgb};

use super::options::MODES;
use super::{transcript, types::Chat};
use crate::theme::{self, blocked, dim, fg, muted, working};
use crate::ui_scale::px;
use crate::views::ui::{self, Kind};
use crate::widgets::icon;
use crate::workspace_app::DroverApp;

fn mode_label(mode: &str) -> &str { MODES.iter().find(|(m, _)| *m == mode).map_or(mode, |(_, l)| l) }

fn suggestions(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let rows = app.chat.suggest.iter().enumerate().map(|(i, (label, note))| {
        ui::row(false).h(px(26.)).text_size(px(12.5)).child(div().flex_none().text_color(rgb(fg())).child(label.clone()))
            .child(div().flex_1().min_w_0().truncate().text_color(rgb(dim())).child(note.clone()))
            .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| app.apply_suggestion(i, cx)))
    }).collect::<Vec<_>>();
    div().flex().flex_col().p(px(4.)).rounded(px(8.)).border_1().border_color(theme::border2()).bg(rgb(theme::card())).children(rows)
}

fn chat_row(c: &Chat, on: bool, cx: &mut Context<DroverApp>) -> Div {
    let id = c.id.clone();
    let color = match c.state.as_str() { "blocked" => blocked(), "working" => working(), _ => dim() };
    ui::row(on).h(px(38.)).gap(px(9.))
        .child(div().size(px(8.)).flex_none().rounded_full().bg(rgb(color)))
        .child(div().flex_1().min_w_0().flex().flex_col()
            .child(div().truncate().text_size(px(13.)).child(c.name.clone()))
            .child(div().truncate().text_size(px(11.)).text_color(rgb(dim())).child(crate::folder_picker_logic::base_name(&c.cwd).to_string())))
        .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { app.open_chat(&id); app.chat.stick.set(true); cx.notify(); }))
}

fn list(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let mut col = div().flex().flex_col().gap(px(2.)).p(px(8.)).flex_1().min_h_0().overflow_hidden()
        .child(ui::button("New chat", Kind::Primary).on_mouse_down(MouseButton::Left, cx.listener(|app, _, window, cx| app.new_chat(window, cx))))
        .child(div().h(px(6.)));
    for c in &app.chat.chats { col = col.child(chat_row(c, app.chat.current.as_deref() == Some(c.id.as_str()), cx)); }
    if app.chat.chats.is_empty() { col = col.child(ui::note("No chats yet", false)); }
    col
}

fn composer(app: &DroverApp, chat: &Chat, cx: &mut Context<DroverApp>) -> Div {
    let Some(input) = app.chat.composer.clone() else { return div() };
    let idle = chat.state == "idle";
    let action = if idle {
        ui::button("Send", Kind::Primary).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.send_chat(cx)))
    } else {
        ui::button("■ Stop", Kind::Danger).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, _| app.stop_chat()))
    };
    div().flex_none().flex().flex_col().gap(px(6.)).px(px(24.)).pb(px(14.)).pt(px(8.)).border_t_1().border_color(theme::border())
        .when(!app.chat.suggest.is_empty(), |d| d.child(suggestions(app, cx)))
        .child(div().min_h(px(44.)).max_h(px(160.)).overflow_hidden().child(input))
        .child(div().flex().items_center().gap(px(10.)).text_size(px(12.)).text_color(rgb(muted()))
            .child(div().px(px(6.)).py(px(2.)).rounded(px(5.)).hover(|d| d.bg(theme::hover())).child(format!("{} ▾", mode_label(&chat.permission_mode)))
                .on_mouse_down(MouseButton::Left, cx.listener(|app, ev: &MouseDownEvent, _, cx| app.open_chat_mode_menu(ev.position, cx))))
            .child(div().px(px(6.)).py(px(2.)).rounded(px(5.)).hover(|d| d.bg(theme::hover())).child(format!("{} ▾", chat.model.clone().unwrap_or_else(|| "default model".into())))
                .on_mouse_down(MouseButton::Left, cx.listener(|app, ev: &MouseDownEvent, _, cx| app.open_chat_model_menu(ev.position, cx))))
            .child(div().flex_1())
            .child(action))
}

pub fn body(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let main = match app.chat.current_chat() {
        Some(chat) => div().flex().flex_col().flex_1().min_h_0().min_w_0()
            .child(div().h(px(40.)).flex_none().flex().items_center().gap(px(10.)).px(px(24.)).border_b_1().border_color(theme::border())
                .child(div().font_weight(FontWeight::SEMIBOLD).text_color(rgb(fg())).child(chat.name.clone()))
                .child(div().flex_1().min_w_0().truncate().text_size(px(12.)).text_color(rgb(dim())).child(chat.cwd.clone()))
                .when(chat.state != "idle", |d| d.child(icon("spin", 13., working()))))
            .child(transcript::transcript(app, &chat.id.clone(), cx))
            .child(composer(app, chat, cx)),
        None => div().flex().flex_1().child(ui::note("Pick a chat, or start a new one.", false)),
    };
    ui::split(240., list(app, cx), main)
}
