//! Machines (port of `machines-view.tsx`, `machine-card.tsx`, `machine-add-form.tsx`): this computer and each remote
//! machine with its connection state, Remove (asked first), and the "Add a machine" form (test, add, install dr).

use gpui::{Context, Div, Entity, FontWeight, MouseButton, div, prelude::*, rgb};
use crate::ui_scale::px;
use serde_json::{Value, json};

use crate::actions::{Confirm, Then};
use crate::theme::{self, dim, done, error, fg, muted, working, tint};
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
    /// Hosts found in ~/.ssh (ssh_hosts.rs), offered as buttons; and whether Name / Port / Key are shown.
    pub hosts: Vec<String>,
    pub more: bool,
    /// What the last test / add said: busy text, steps, or an error.
    pub log: Option<(Vec<String>, Option<String>)>,
}

impl MachinesForm {
    pub fn new(cx: &mut Context<DroverApp>) -> Self {
        let mut make = |p: &str| cx.new(|cx| TextInput::new(p, cx));
        MachinesForm { alias: make("Same as the host"), target: make("workbox  or  user@host"), port: make("22 (or the Port in ~/.ssh/config)"),
                       key: make("Use ssh-agent / default keys"), install: false, hosts: crate::ssh_hosts::detect(), more: false, log: None }
    }
}

impl DroverApp {
    /// The host is enough: the name is taken from it and the port and key from ~/.ssh/config unless filled in.
    fn machine_params(&self, cx: &gpui::App) -> Value {
        let f = &self.machines_form;
        let t = |e: &Entity<TextInput>| e.read(cx).text.trim().to_string();
        let target = t(&f.target);
        let alias = Some(t(&f.alias)).filter(|a| !a.is_empty()).unwrap_or_else(|| crate::ssh_hosts::alias_for(&target));
        json!({"alias": alias, "target": target, "port": t(&f.port).parse::<u16>().unwrap_or(22), "key": t(&f.key)})
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
        .child(div().flex().items_center().gap(px(8.)).child(icon(icon_name, 15., muted()))
            .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child(title.to_string()))
            .child(div().px(px(7.)).h(px(20.)).flex().items_center().rounded(px(5.)).text_size(px(11.)).bg(tint(pill.1, 0.16)).text_color(rgb(pill.1)).child(pill.0)))
}

fn field(label: &str, input: &Entity<TextInput>) -> Div {
    div().flex().flex_col().gap(px(4.)).child(div().text_size(px(12.)).text_color(rgb(muted())).child(label.to_string())).child(input.clone())
}

pub fn body(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let mut grid = div().flex().flex_wrap().gap(px(12.)).content_start()
        .child(card("This computer", "monitor", ("Connected".into(), done()))
            .child(div().text_size(px(12.)).text_color(rgb(dim())).child(format!("{} spaces · {} agents", app.store.spaces.len(), app.store.panes.len()))));
    for m in app.store.machines.as_array().into_iter().flatten() {
        let alias = m["alias"].as_str().unwrap_or("?").to_string();
        let on = m["status"] == "connected";
        let pill = match m["status"].as_str() {
            Some("connected") => (m["latency_ms"].as_i64().map(|l| format!("{l} ms")).unwrap_or("Connected".into()), done()),
            Some("connecting") => ("Connecting".into(), working()),
            Some("retrying") => ("Retrying".into(), error()),
            _ => ("Unreachable".into(), error()),
        };
        let snap = &m["snapshot"];
        let count = |k: &str| snap[k].as_array().map(|a| a.len()).unwrap_or(0);
        let (a, rename_alias, test_alias) = (alias.clone(), alias.clone(), alias.clone());
        let target = m["target"].as_str().unwrap_or("").to_string();
        grid = grid.child(card(&alias, "server", pill)
            .child(div().text_size(px(12.)).text_color(rgb(dim())).child(if on {
                format!("dr {} · {} spaces · {} agents", m["info"]["version"].as_str().unwrap_or("?"), count("spaces"), count("panes"))
            } else { m["error"].as_str().unwrap_or("Agents there keep running; you'll see them when it's back.").to_string() }))
            .child(div().text_size(px(12.)).text_color(rgb(dim())).font_family(app.code_family.clone()).child(m["target"].as_str().unwrap_or("").to_string()))
            .child(div().flex().gap(px(8.))
                .child(ui::button("Test", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
                    app.test_saved_machine(test_alias.clone(), target.clone());
                    cx.notify();
                })))
                .child(ui::button("Rename", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, window, cx| app.open_rename_machine(rename_alias.clone(), window, cx))))
                .child(ui::button("Remove", Kind::Danger).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| app.ask_remove_machine(a.clone(), cx))))));
    }
    let f = &app.machines_form;
    // Hosts from ~/.ssh that are not saved yet.
    let saved: Vec<&str> = app.store.machines.as_array().into_iter().flatten()
        .flat_map(|m| [m["alias"].as_str(), m["target"].as_str()]).flatten().collect();
    let offered: Vec<String> = f.hosts.iter().filter(|h| !saved.contains(&h.as_str())).cloned().collect();
    let mut log =div().flex().flex_col().gap(px(3.)).text_size(px(12.));
    if let Some((steps, err)) = &f.log {
        for s in steps { log = log.child(div().text_color(rgb(fg())).child(s.clone())); }
        if let Some(e) = err { log = log.child(div().text_color(rgb(error())).child(e.clone())); }
    }
    let form = div().w(px(340.)).flex_none().flex().flex_col().gap(px(12.)).p(px(16.)).border_l_1().border_color(theme::border())
        .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child("Add a machine"))
        .child(field("Host (a name from ~/.ssh/config, or user@host)", &f.target))
        .when(!offered.is_empty(), |d| d.child(div().flex().flex_col().gap(px(5.))
            .child(div().text_size(px(12.)).text_color(rgb(muted())).child("Found on this computer"))
            .child(div().flex().flex_wrap().gap(px(6.)).children(offered.into_iter().map(|h| {
                let pick = h.clone();
                div().px(px(9.)).py(px(3.)).rounded(px(6.)).border_1().border_color(theme::border()).text_size(px(12.))
                    .hover(|d| d.bg(theme::hover())).child(h)
                    .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
                        let target = app.machines_form.target.clone();
                        target.update(cx, |t, cx| t.set_text(&pick, cx));
                        cx.notify();
                    }))
            })))))
        .child(div().flex().items_center().gap(px(6.)).text_size(px(12.)).text_color(rgb(muted()))
            .child(if f.more { "▾" } else { "▸" }).child("More options (name, port, key)")
            .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| { app.machines_form.more = !app.machines_form.more; cx.notify(); })))
        .when(f.more, |d| d.child(field("Name", &f.alias))
            .child(div().flex().gap(px(8.)).child(div().w(px(80.)).child(field("Port", &f.port))).child(div().flex_1().child(field("Key file", &f.key)))))
        .child(div().flex().items_center().gap(px(8.)).child(ui::toggle(f.install)).child(div().text_size(px(12.5)).child("Install dr there if missing (Linux/macOS)"))
            .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| { app.machines_form.install = !app.machines_form.install; cx.notify(); })))
        .child(div().text_size(px(12.)).text_color(rgb(dim())).child("Host keys are checked against known_hosts. The daemon there only listens on 127.0.0.1; Drover reaches it through the SSH tunnel."))
        .child(log)
        .child(div().flex().justify_end().gap(px(8.))
            .child(ui::button("Test only", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.machine_test(cx))))
            .child(ui::button("Add", Kind::Primary).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.machine_add(cx)))));
    let note = div().text_size(px(12.)).text_color(rgb(dim())).child(
        "Phone: with Tailscale on both devices, run `dr server stop` then `dr server run --listen <tailscale-ip>`, and open the link from `dr server url` on the phone. Keep that link private.");
    div().flex().flex_row().flex_1().min_h_0()
        .child(div().id("machines").flex_1().min_w_0().overflow_y_scroll().flex().flex_col().gap(px(14.)).p(px(16.)).child(grid).child(note))
        .child(form)
}
