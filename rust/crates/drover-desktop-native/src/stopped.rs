//! Panes that stopped when the computer restarted (`stopped` in the daemon's state): the layout still has their place,
//! so it shows a card with Resume / Start fresh / Close instead of a terminal, as the web UI's stopped-pane.tsx.

use gpui::{Context, Div, FontWeight, MouseButton, div, prelude::*, rgb};
use serde_json::{Value, json};

use crate::theme::{self, dim, fg, muted, pane, UI_FONT};
use crate::ui_scale::px;
use crate::views::ui::{self, Kind};
use crate::widgets::{kind_icon, short_path};
use crate::workspace_app::DroverApp;

#[derive(Clone, Debug, PartialEq)]
pub struct StoppedPane {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub cwd: String,
    /// The machine it ran on (not this computer), for the RPC.
    pub machine: Option<String>,
}

/// The `stopped` list of a `state` or `resync` event; `None` when the event carries none (keep what we have).
pub fn parse(data: &Value) -> Option<Vec<StoppedPane>> {
    let list = data["stopped"].as_array()?;
    let text = |v: &Value| v.as_str().unwrap_or("").to_string();
    Some(list.iter().filter_map(|p| Some(StoppedPane {
        id: p["id"].as_str()?.to_string(), name: text(&p["name"]), kind: text(&p["kind"]), cwd: text(&p["cwd"]),
        machine: p["machine"].as_str().filter(|m| !m.is_empty() && *m != "local").map(String::from),
    })).collect())
}

impl DroverApp {
    fn session_rpc(&mut self, method: &str, p: &StoppedPane, fresh: bool) {
        let mut params = json!({"panes": [p.id]});
        if method == "session.resume" { params["fresh"] = json!(fresh); }
        if let Some(m) = &p.machine { params["machine"] = json!(m); }
        self.rpc(method, params);
    }
}

fn button(label: &'static str, kind: Kind, p: StoppedPane, method: &'static str, fresh: bool, cx: &mut Context<DroverApp>) -> Div {
    ui::button(label, kind).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { app.session_rpc(method, &p, fresh); cx.notify(); }))
}

/// The card in a pane's place.
pub fn card(p: &StoppedPane, cx: &mut Context<DroverApp>) -> Div {
    div().flex().flex_col().flex_1().min_w_0().min_h_0().rounded(px(10.)).overflow_hidden().border_1().border_color(theme::border()).bg(rgb(pane()))
        .child(div().h(px(30.)).flex_none().flex().items_center().gap(px(8.)).pl(px(10.)).pr(px(6.)).border_b_1().border_color(theme::border())
            .text_size(px(12.)).font_family(UI_FONT)
            .child(kind_icon(&p.kind))
            .child(div().flex_none().font_weight(FontWeight::MEDIUM).child(if p.name.is_empty() { p.id.clone() } else { p.name.clone() }))
            .child(div().flex_1().min_w_0().truncate().text_color(rgb(dim())).child(short_path(&p.cwd))))
        .child(div().flex().flex_col().gap(px(12.)).p(px(18.)).font_family(UI_FONT)
            .child(div().text_size(px(13.)).text_color(rgb(muted())).child("This agent stopped when the computer restarted."))
            .child(div().flex().gap(px(8.))
                .child(button("Resume", Kind::Primary, p.clone(), "session.resume", false, cx))
                .child(button("Start fresh", Kind::Plain, p.clone(), "session.resume", true, cx))
                .child(button("Close", Kind::Plain, p.clone(), "session.discard", false, cx))))
        .text_color(rgb(fg()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_stopped_list_and_keeps_the_machine() {
        let v = json!({"stopped": [{"id": "%3", "name": "api", "kind": "claude", "cwd": "/x", "machine": "box"}, {"id": "%4", "machine": "local"}, {"name": "no id"}]});
        let l = parse(&v).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!((l[0].machine.as_deref(), l[1].machine.clone()), (Some("box"), None));
        assert!(parse(&json!({})).is_none());
    }
}
