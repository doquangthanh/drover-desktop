//! Machines (port of `machines-view.tsx`, `machine-card.tsx`, `machine-add-form.tsx`): this computer and each remote
//! machine with its connection state, Remove (asked first), and the "Add a machine" form (test, add, install dr).

use gpui::{Context, Div, Entity, FontWeight, MouseButton, div, prelude::*, px, rgb};
use serde_json::{Value, json};

use crate::actions::{Confirm, Then};
use crate::theme::{self, DIM, DONE, ERROR, FG, MUTED, WORKING, tint};
use crate::views::text_input::TextInput;
use crate::views::ui::{self, Kind};
use crate::widgets::icon;
use crate::workspace_app::DroverApp;

pub struct MachinesForm {
    pub alias: Entity<TextInput>,
    pub target: Entity<TextInput>,
    pub port: Entity<TextInput>,
    pub key: Entity<TextInput>,
    pub install: bool,
    /// What the last test / add said: busy text, steps, or an error.
    pub log: Option<(Vec<String>, Option<String>)>,
}

impl MachinesForm {
    pub fn new(cx: &mut Context<DroverApp>) -> Self {
        let mut make = |p: &str| cx.new(|cx| TextInput::new(p, cx));
        MachinesForm { alias: make("vps-2"), target: make("deploy@vps-2.tail-net"), port: make("22"),
                       key: make("Use ssh-agent / default keys"), install: false, log: None }
    }
}

impl DroverApp {
    fn machine_params(&self, cx: &gpui::App) -> Value {
        let f = &self.machines_form;
        let t = |e: &Entity<TextInput>| e.read(cx).text.trim().to_string();
        json!({"alias": t(&f.alias), "target": t(&f.target), "port": t(&f.port).parse::<u16>().unwrap_or(22), "key": t(&f.key)})
    }

    pub fn machine_test(&mut self, cx: &mut Context<Self>) {
        self.machines_form.log = Some((vec!["Testing SSH…".into()], None));
        self.rpc_reply("machine.test", self.machine_params(cx), |app, r| {
            app.machines_form.log = Some(match r {
                Ok(v) => (v["steps"].as_array().into_iter().flatten().filter_map(|s| s.as_str()).map(|s| format!("✓ {s}")).collect(),
                          (v["ok"] != true).then(|| v["error"].as_str().unwrap_or("failed").to_string())),
                Err(e) => (vec![], Some(e)),
            });
        });
        cx.notify();
    }

    pub fn machine_add(&mut self, cx: &mut Context<Self>) {
        let mut params = self.machine_params(cx);
        params["install"] = json!(self.machines_form.install);
        let busy = if self.machines_form.install { "Installing dr and connecting…" } else { "Adding…" };
        self.machines_form.log = Some((vec![busy.into()], None));
        let alias = params["alias"].as_str().unwrap_or("").to_string();
        self.rpc_reply("machine.add", params, move |app, r| {
            app.machines_form.log = Some(match r {
                Ok(_) => (vec![format!("✓ Added {alias}")], None),
                Err(e) => (vec![], Some(e)),
            });
        });
        cx.notify();
    }

    fn ask_remove_machine(&mut self, alias: String, cx: &mut Context<Self>) {
        self.confirm = Some(Confirm { title: format!("Remove {alias}?"), detail: "Agents there keep running; Drover stops showing them.".into(),
                                      ok: "Remove", then: Then::RemoveMachine(alias) });
        cx.notify();
    }
}

fn card(title: &str, icon_name: &str, pill: (String, u32)) -> Div {
    div().w(px(300.)).flex_none().flex().flex_col().gap(px(8.)).p(px(14.)).rounded(px(10.)).border_1().border_color(theme::border())
        .child(div().flex().items_center().gap(px(8.)).child(icon(icon_name, 15., MUTED))
            .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child(title.to_string()))
            .child(div().px(px(7.)).h(px(20.)).flex().items_center().rounded(px(5.)).text_size(px(11.)).bg(tint(pill.1, 0.16)).text_color(rgb(pill.1)).child(pill.0)))
}

fn field(label: &str, input: &Entity<TextInput>) -> Div {
    div().flex().flex_col().gap(px(4.)).child(div().text_size(px(12.)).text_color(rgb(MUTED)).child(label.to_string())).child(input.clone())
}

pub fn body(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let mut grid = div().flex().flex_wrap().gap(px(12.)).content_start()
        .child(card("This computer", "monitor", ("Connected".into(), DONE))
            .child(div().text_size(px(12.)).text_color(rgb(DIM)).child(format!("{} spaces · {} agents", app.store.spaces.len(), app.store.panes.len()))));
    for m in app.store.machines.as_array().into_iter().flatten() {
        let alias = m["alias"].as_str().unwrap_or("?").to_string();
        let on = m["status"] == "connected";
        let pill = match m["status"].as_str() {
            Some("connected") => (m["latency_ms"].as_i64().map(|l| format!("{l} ms")).unwrap_or("Connected".into()), DONE),
            Some("connecting") => ("Connecting".into(), WORKING),
            Some("retrying") => ("Retrying".into(), ERROR),
            _ => ("Unreachable".into(), ERROR),
        };
        let snap = &m["snapshot"];
        let count = |k: &str| snap[k].as_array().map(|a| a.len()).unwrap_or(0);
        let a = alias.clone();
        grid = grid.child(card(&alias, "server", pill)
            .child(div().text_size(px(12.)).text_color(rgb(DIM)).child(if on {
                format!("dr {} · {} spaces · {} agents", m["info"]["version"].as_str().unwrap_or("?"), count("spaces"), count("panes"))
            } else { m["error"].as_str().unwrap_or("Agents there keep running; you'll see them when it's back.").to_string() }))
            .child(div().text_size(px(12.)).text_color(rgb(DIM)).font_family(app.code_family.clone()).child(m["target"].as_str().unwrap_or("").to_string()))
            .child(div().flex().child(ui::button("Remove", Kind::Danger).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| app.ask_remove_machine(a.clone(), cx))))));
    }
    let f = &app.machines_form;
    let mut log = div().flex().flex_col().gap(px(3.)).text_size(px(12.));
    if let Some((steps, err)) = &f.log {
        for s in steps { log = log.child(div().text_color(rgb(FG)).child(s.clone())); }
        if let Some(e) = err { log = log.child(div().text_color(rgb(ERROR)).child(e.clone())); }
    }
    let form = div().w(px(340.)).flex_none().flex().flex_col().gap(px(12.)).p(px(16.)).border_l_1().border_color(theme::border())
        .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child("Add a machine"))
        .child(field("Name", &f.alias))
        .child(field("SSH target (user@host or a Host from ~/.ssh/config)", &f.target))
        .child(div().flex().gap(px(8.)).child(div().w(px(80.)).child(field("Port", &f.port))).child(div().flex_1().child(field("Key file", &f.key))))
        .child(div().flex().items_center().gap(px(8.)).child(ui::toggle(f.install)).child(div().text_size(px(12.5)).child("Install dr there if missing (Linux/macOS)"))
            .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| { app.machines_form.install = !app.machines_form.install; cx.notify(); })))
        .child(div().text_size(px(12.)).text_color(rgb(DIM)).child("Host keys are checked against known_hosts. The daemon there only listens on 127.0.0.1; Drover reaches it through the SSH tunnel."))
        .child(log)
        .child(div().flex().justify_end().gap(px(8.))
            .child(ui::button("Test only", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.machine_test(cx))))
            .child(ui::button("Add", Kind::Primary).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.machine_add(cx)))));
    let note = div().text_size(px(12.)).text_color(rgb(DIM)).child(
        "Phone: with Tailscale on both devices, run `dr server stop` then `dr server run --listen <tailscale-ip>`, and open the link from `dr server url` on the phone. Keep that link private.");
    div().flex().flex_row().flex_1().min_h_0()
        .child(div().id("machines").flex_1().min_w_0().overflow_y_scroll().flex().flex_col().gap(px(14.)).p(px(16.)).child(grid).child(note))
        .child(form)
}
