//! Settings → Rules tester (port of `rule-tester.tsx`): runs the detection rules against the live screen of a pane
//! (`rules.test`, read-only) and shows which rule matched and the resulting state.

use gpui::{Context, Div, FontWeight, MouseButton, div, prelude::*, px, rgb};
use serde_json::{Value, json};

use crate::theme::{self, blocked, dim, done, fg, muted, working};
use crate::views::ui::{self, Kind};
use crate::workspace_app::DroverApp;

#[derive(Default)]
pub struct RulesState {
    /// The pane being tested (None: the first local one).
    pub pane: Option<String>,
    /// `rules.test` reply for `loaded`, or its error.
    pub result: Option<Result<Value, String>>,
    loaded: Option<String>,
}

/// Panes of this computer (`machine` empty or "local"): remote panes have their own daemon.
fn local_panes(app: &DroverApp) -> Vec<(String, String)> {
    app.store.panes.iter().filter(|p| p.machine.is_empty() || p.machine == "local")
        .map(|p| (p.id.clone(), p.name.clone())).collect()
}

fn chosen(app: &DroverApp) -> Option<String> {
    let panes = local_panes(app);
    app.rules.pane.clone().filter(|id| panes.iter().any(|(p, _)| p == id)).or_else(|| panes.first().map(|(id, _)| id.clone()))
}

/// Ask for the result of the chosen pane when it is not loaded yet.
pub fn refresh(app: &mut DroverApp) {
    let Some(pane) = chosen(app) else { return };
    if app.rules.loaded.as_deref() == Some(pane.as_str()) { return }
    app.rules.loaded = Some(pane.clone());
    app.rules.result = Some(Err("Running the rules…".into()));
    app.rpc_reply("rules.test", json!({"pane": pane}), |app, r| app.rules.result = Some(r));
}

/// Pill label and colour for a detected state.
pub fn state_pill(state: &str) -> (String, u32) {
    match state {
        "blocked" => ("Needs you".into(), blocked()),
        "working" => ("Working".into(), working()),
        "done" => ("Done".into(), done()),
        "idle" => ("Idle".into(), dim()),
        "" => ("no rule matched".into(), dim()),
        other => (other.to_string(), dim()),
    }
}

pub fn panel(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let panes = local_panes(app);
    let Some(current) = chosen(app) else { return ui::note("Start an agent to test the rules against its screen.", false) };
    let mut bar = div().flex().flex_wrap().items_center().gap(px(6.)).p(px(12.)).border_b_1().border_color(theme::border())
        .child(div().text_size(px(12.)).text_color(rgb(dim())).child("Test against"));
    for (id, name) in panes {
        let on = id == current;
        bar = bar.child(ui::tab(&name, on).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
            app.rules.pane = Some(id.clone());
            cx.notify();
        })));
    }
    bar = bar.child(div().flex_1())
        .child(ui::button("Run again", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| {
            app.rules.loaded = None;
            cx.notify();
        })));
    let body = match &app.rules.result {
        None => ui::note("", false),
        Some(Err(e)) => ui::note(e.clone(), e != "Running the rules…"),
        Some(Ok(r)) => result(app, r),
    };
    div().flex().flex_col().flex_1().min_h_0().child(bar).child(body)
}

fn result(app: &DroverApp, r: &Value) -> Div {
    let code = app.code_family.clone();
    let mut screen = div().flex().flex_col().p(px(12.)).font_family(code.clone()).text_size(px(12.))
        .child(div().text_color(rgb(dim())).child(format!("title: {}", r["title"].as_str().unwrap_or(""))));
    for l in r["screen"].as_array().into_iter().flatten().filter_map(|l| l.as_str()) {
        screen = screen.child(div().whitespace_nowrap().text_color(rgb(fg())).child(l.to_string()));
    }
    let head = |t: &str| div().text_size(px(11.)).text_color(rgb(dim())).font_weight(FontWeight::SEMIBOLD).child(t.to_string());
    let mut table = div().flex().flex_col().gap(px(4.)).p(px(12.)).text_size(px(12.5)).child(head("RULES"));
    for x in r["results"].as_array().into_iter().flatten() {
        let hit = x["matched"] == true;
        let color = if hit { blocked() } else { muted() };
        table = table.child(div().flex().gap(px(12.))
            .child(div().flex_1().min_w_0().truncate().text_color(rgb(color))
                .child(format!("{} → {}", x["rule_id"].as_str().unwrap_or("?"), x["state"].as_str().unwrap_or("?"))))
            .child(div().w(px(40.)).text_color(rgb(dim())).child(x["priority"].to_string()))
            .child(div().w(px(90.)).font_family(code.clone()).text_color(rgb(dim())).child(x["region"].as_str().unwrap_or("").to_string()))
            .child(div().w(px(70.)).text_color(rgb(color)).child(if hit { "match" } else { "no match" })));
    }
    let (label, color) = state_pill(r["detected"].as_str().unwrap_or(""));
    let osc = r["osc7501"].as_str().filter(|s| !s.is_empty()).map(|s| format!(" · OSC 7501 says {s}")).unwrap_or_default();
    table = table.child(div().flex().items_center().gap(px(10.)).pt(px(8.))
        .child("Result")
        .child(div().px(px(7.)).h(px(20.)).flex().items_center().rounded(px(5.)).text_size(px(11.)).bg(theme::tint(color, 0.16)).text_color(rgb(color)).child(label))
        .child(div().text_color(rgb(dim())).child(format!("current state {}{osc}", r["state"].as_str().unwrap_or("?")))));
    div().flex().flex_col().flex_1().min_h_0()
        .child(div().id("rules-result").flex().flex_col().size_full().overflow_y_scroll().child(screen).child(table))
}
