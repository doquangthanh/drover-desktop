//! Settings → Server (port of `server-view.tsx`): the daemon's process, connected clients and config problems,
//! "Test notification", and Stop (asked first: stopping the server stops the agents too).

use gpui::{Context, Div, FontWeight, MouseButton, div, prelude::*, px, rgb};
use serde_json::json;

use crate::actions::{Confirm, Then};
use crate::theme::{dim, done, error, fg};
use crate::views::ui::{self, Kind};
use crate::workspace_app::DroverApp;

/// "2d 3h", "5h 10m", "<1m": the two biggest units (same as `uptimeLabel` in the web UI).
pub fn uptime_label(seconds: f64) -> String {
    let up = seconds.max(0.) as u64;
    let parts = [(up / 86400, "d"), (up % 86400 / 3600, "h"), (up % 3600 / 60, "m")];
    let text: Vec<String> = parts.iter().filter(|(n, _)| *n > 0).take(2).map(|(n, u)| format!("{n}{u}")).collect();
    if text.is_empty() { "<1m".into() } else { text.join(" ") }
}

pub fn refresh(app: &mut DroverApp) {
    if app.settings.server.is_none() {
        app.settings.server = Some(Err("Loading…".into()));
        app.rpc_reply("server.info", json!({}), |app, r| app.settings.server = Some(r));
    }
}

impl DroverApp {
    pub fn ask_stop_server(&mut self, cx: &mut Context<Self>) {
        self.confirm = Some(Confirm { title: "Stop the Drover server?".into(),
            detail: "Agents stop too (the session can be restored when it starts again).".into(), ok: "Stop", then: Then::StopServer });
        cx.notify();
    }
}

fn kv(code: &gpui::SharedString, k: &str, v: String) -> Div {
    div().flex().gap(px(12.))
        .child(div().w(px(110.)).flex_none().text_color(rgb(dim())).child(k.to_string()))
        .child(div().flex_1().min_w_0().truncate().font_family(code.clone()).child(v))
}

pub fn panel(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let info = match &app.settings.server {
        None => return ui::note("", false),
        Some(Err(e)) => return ui::note(e.clone(), e != "Loading…"),
        Some(Ok(v)) => v,
    };
    let code = &app.code_family;
    let counts = info["counts"].as_object().map(|o| o.iter().map(|(k, v)| format!("{} {k}", v)).collect::<Vec<_>>().join(" · ")).filter(|s| !s.is_empty()).unwrap_or("none".into());
    let str_of = |k: &str| info[k].as_str().unwrap_or("").to_string();
    let clients: Vec<String> = info["clients"].as_array().into_iter().flatten().filter_map(|c| c["label"].as_str()).map(String::from).collect();
    let problems: Vec<String> = ["rule_errors", "settings_errors"].iter()
        .flat_map(|k| info[*k].as_array().into_iter().flatten().filter_map(|e| e.as_str()).map(String::from)).collect();
    let head = |t: &str| div().text_size(px(11.)).text_color(rgb(dim())).font_weight(FontWeight::SEMIBOLD).child(t.to_string());
    let mut d = div().flex().flex_col().gap(px(10.)).p(px(18.)).max_w(px(760.))
        .child(div().flex().items_center().gap(px(8.)).text_color(rgb(done()))
            .child(format!("● running · up {}", uptime_label(info["uptime"].as_f64().unwrap_or(0.)))))
        .child(head("STATUS"))
        .child(kv(code, "Process", format!("pid {} · dr {}", info["pid"], str_of("version"))))
        .child(kv(code, "Listening", format!("127.0.0.1:{} (this machine only)", info["port"])))
        .child(kv(code, "Session file", format!("session.json · {}", if info["session_saved"] == true { "saved" } else { "not saved yet" })))
        .child(kv(code, "Config", str_of("config_dir")))
        .child(kv(code, "Agents", counts))
        .child(head(&format!("CONNECTED CLIENTS · {}", clients.len())));
    for c in clients { d = d.child(div().text_color(rgb(fg())).child(format!("● {c}"))); }
    for p in problems { d = d.child(div().text_size(px(12.)).text_color(rgb(error())).child(p)); }
    d.child(div().flex().items_center().gap(px(8.))
            .child(ui::button("Test notification", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, _| app.rpc("notify.test", json!({})))))
            .child(ui::button("Stop server", Kind::Danger).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.ask_stop_server(cx)))))
        .child(div().text_size(px(12.)).text_color(rgb(dim())).child("Closing the window never stops agents. Stopping the server does."))
}

#[cfg(test)]
mod tests {
    use super::uptime_label;

    #[test]
    fn shows_the_two_biggest_units() {
        assert_eq!(uptime_label(30.), "<1m");
        assert_eq!(uptime_label(3600. * 5. + 600.), "5h 10m");
        assert_eq!(uptime_label(86400. * 2. + 3600. * 3. + 60.), "2d 3h");
    }
}
