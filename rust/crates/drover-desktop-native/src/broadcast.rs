//! "Send to several agents" (the web UI's Broadcast dialog): one prompt, a tick per agent, Send. Agents (not plain
//! shells) are ticked when it opens; exited panes are left out. Escape or a click outside closes it, Ctrl+S sends.

use std::collections::HashSet;

use gpui::{Context, Div, Entity, Focusable, FontWeight, MouseButton, Window, div, prelude::*, rgb, rgba};
use serde_json::json;

use crate::theme::{self, dim, fg, muted, UI_FONT};
use crate::ui_scale::px;
use crate::views::text_input::{InputEvent, TextInput};
use crate::views::ui::{self, Kind};
use crate::workspace_app::DroverApp;

pub struct Broadcast {
    pub text: Entity<TextInput>,
    /// (pane id, name, space name): a snapshot taken when the dialog opens, like the web dialog.
    pub panes: Vec<(String, String, String)>,
    pub on: HashSet<String>,
}

impl DroverApp {
    pub fn open_broadcast(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let live: Vec<_> = self.store.panes.iter().filter(|p| p.state != "exited").collect();
        let space_name = |id: &str| self.store.spaces.iter().find(|s| s.id == id).map(|s| s.name.clone()).unwrap_or_default();
        let panes = live.iter().map(|p| (p.id.clone(), p.name.clone(), space_name(&p.space_id))).collect();
        let on = live.iter().filter(|p| p.kind != "shell").map(|p| p.id.clone()).collect();
        let text = cx.new(TextInput::new_multiline);
        cx.subscribe(&text, |app, _, ev, cx| if matches!(ev, InputEvent::Submit) { app.send_broadcast(cx) }).detach();
        window.focus(&text.read(cx).focus_handle(cx));
        self.broadcast = Some(Broadcast { text, panes, on });
        cx.notify();
    }

    pub fn close_broadcast(&mut self, cx: &mut Context<Self>) {
        self.broadcast = None;
        self.want_term_focus = true;
        cx.notify();
    }

    /// `pane.send` of the prompt to every ticked agent; an empty prompt sends nothing.
    pub fn send_broadcast(&mut self, cx: &mut Context<Self>) {
        if let Some(b) = &self.broadcast {
            let text = b.text.read(cx).value().trim().to_string();
            if !text.is_empty() {
                for (id, _, _) in b.panes.iter().filter(|(id, _, _)| b.on.contains(id)) {
                    self.rpc("pane.send", json!({"pane": id, "text": text}));
                }
            }
        }
        self.close_broadcast(cx);
    }
}

fn agent_row(id: &str, name: &str, space: &str, on: bool, cx: &mut Context<DroverApp>) -> Div {
    let id = id.to_string();
    div().h(px(32.)).flex().items_center().gap(px(10.)).px(px(8.)).rounded(px(7.)).hover(|d| d.bg(theme::hover()))
        .child(ui::toggle(on))
        .child(div().flex_1().min_w_0().truncate().text_size(px(13.)).text_color(rgb(fg())).child(name.to_string()))
        .child(div().text_size(px(12.)).text_color(rgb(dim())).child(space.to_string()))
        .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
            if let Some(b) = &mut app.broadcast
                && !b.on.remove(&id)
            {
                b.on.insert(id.clone());
            }
            cx.notify();
        }))
}

pub fn overlay(app: &DroverApp, cx: &mut Context<DroverApp>) -> Option<Div> {
    let b = app.broadcast.as_ref()?;
    let rows = b.panes.iter().map(|(id, name, space)| agent_row(id, name, space, b.on.contains(id), cx)).collect::<Vec<_>>();
    let count = b.on.len();
    let card = div().w(px(560.)).p(px(22.)).flex().flex_col().gap(px(12.)).rounded(px(12.)).bg(rgb(theme::card()))
        .border_1().border_color(theme::border2()).shadow_lg().font_family(UI_FONT)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(div().flex().items_center()
            .child(div().flex_1().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(fg())).child("Send to several agents"))
            .child(div().text_size(px(12.)).text_color(rgb(dim())).child("Esc")))
        .child(div().text_size(px(12.)).text_color(rgb(muted())).child("Prompt to send (Ctrl+S sends)"))
        .child(div().h(px(90.)).child(b.text.clone()))
        .child(div().flex().flex_col().gap(px(2.)).max_h(px(260.)).overflow_hidden().children(rows))
        .child(div().flex().items_center().gap(px(8.))
            .child(div().flex_1().text_size(px(12.)).text_color(rgb(dim())).child(format!("{count} agent(s) ticked")))
            .child(ui::button("Cancel", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.close_broadcast(cx))))
            .child(ui::button("Send", Kind::Primary).when(count == 0, |d| d.opacity(0.45))
                .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.send_broadcast(cx)))));
    Some(div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().bg(rgba(0x00000073))
        .occlude().child(card)
        .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.close_broadcast(cx))))
}
