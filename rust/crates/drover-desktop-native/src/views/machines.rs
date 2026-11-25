//! Machines: this computer and each saved remote machine as a list (status dot, what runs there, Test / Rename /
//! Remove), and "Add a machine", which opens the add dialog (machines_add_dialog.rs).

use gpui::{Context, Div, Entity, Focusable, FontWeight, MouseButton, div, prelude::*, rgb};
use crate::ui_scale::px;
use serde_json::{Value, json};

use crate::actions::{Confirm, Then};
use crate::theme::{self, dim, done, error, fg, muted, working};
use crate::views::text_input::{InputEvent, TextInput};
use crate::views::ui::{self, Kind};
use crate::workspace_app::DroverApp;

pub struct MachinesForm {
    /// The add dialog is shown.
    pub open: bool,
    pub alias: Entity<TextInput>,
    pub target: Entity<TextInput>,
    pub port: Entity<TextInput>,
    pub key: Entity<TextInput>,
    pub install: bool,
    /// Hosts found in ~/.ssh (ssh_hosts.rs), offered as one-click rows; and whether Name / Port / Key are shown.
    pub hosts: Vec<String>,
    pub more: bool,
    /// What the last test / add said: busy text, steps, or an error.
    pub log: Option<(Vec<String>, Option<String>)>,
}

impl MachinesForm {
    pub fn new(cx: &mut Context<DroverApp>) -> Self {
        let mut make = |p: &str| cx.new(|cx| TextInput::new(p, cx));
        let (alias, target, port, key) = (make("Same as the host"), make("user@host  or a Host from ~/.ssh/config"),
                                          make("22 (or the Port in ~/.ssh/config)"), make("Use ssh-agent / default keys"));
        // Enter in the host field adds it; edits redraw the Add button. Escape reaches on_key (terminal_input.rs),
        // which closes the dialog.
        cx.subscribe(&target, |app, _, ev, cx| match ev {
            InputEvent::Submit => app.machine_add(cx),
            InputEvent::Changed => cx.notify(),
            InputEvent::Cancel | InputEvent::Pick(_) | InputEvent::PasteImage => {}
        }).detach();
        MachinesForm { open: false, alias, target, port, key, install: false, hosts: crate::ssh_hosts::detect(), more: false, log: None }
    }
}

impl DroverApp {
    /// The host is enough: the name is taken from it and the port and key from ~/.ssh/config unless filled in.
    fn machine_params(&self, cx: &gpui::App) -> Value {
        let f = &self.machines_form;
        let t = |e: &Entity<TextInput>| e.read(cx).text.trim().to_string();
        let target = t(&f.target);
        let taken: Vec<&str> = self.store.machines.as_array().into_iter().flatten().filter_map(|m| m["alias"].as_str()).collect();
        let alias = Some(t(&f.alias)).filter(|a| !a.is_empty()).unwrap_or_else(|| crate::ssh_hosts::alias_for(&target, &taken));
        json!({"alias": alias, "target": target, "port": t(&f.port).parse::<u16>().unwrap_or(22), "key": t(&f.key)})
    }

    pub fn open_add_machine(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let f = &mut self.machines_form;
        (f.open, f.log) = (true, None);
        f.hosts = crate::ssh_hosts::detect(); // ~/.ssh may have changed since the app started
        window.focus(&f.target.read(cx).focus_handle(cx));
        cx.notify();
    }

    pub fn close_add_machine(&mut self, cx: &mut Context<Self>) {
        self.machines_form.open = false;
        cx.notify();
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

    /// Add what the host field says. The field is emptied and the dialog stays for the next one; the log says how it went.
    pub fn machine_add(&mut self, cx: &mut Context<Self>) {
        let mut params = self.machine_params(cx);
        if params["target"].as_str().unwrap_or("").is_empty() {
            return;
        }
        params["install"] = json!(self.machines_form.install);
        let (alias, target) = (params["alias"].as_str().unwrap_or("").to_string(), params["target"].as_str().unwrap_or("").to_string());
        let busy = if self.machines_form.install { format!("Installing dr on {target} and connecting…") } else { format!("Adding {target}…") };
        self.machines_form.log = Some((vec![busy], None));
        self.machines_form.target.update(cx, |t, cx| t.set_text("", cx));
        self.rpc_reply("machine.add", params, move |app, r| {
            app.machines_form.log = Some(match r {
                Ok(_) => (vec![format!("✓ Added {alias}")], None),
                Err(e) => (vec![], Some(format!("{target}: {e}"))),
            });
        });
        cx.notify();
    }

    /// One click on a host found in ~/.ssh: add it as it is.
    pub fn machine_add_host(&mut self, host: &str, cx: &mut Context<Self>) {
        self.machines_form.target.update(cx, |t, cx| t.set_text(host, cx));
        self.machine_add(cx);
    }

    fn ask_remove_machine(&mut self, alias: String, cx: &mut Context<Self>) {
        self.confirm = Some(Confirm { title: format!("Remove {alias}?"), detail: "Agents there keep running; Drover stops showing them.".into(),
                                      ok: "Remove", then: Then::RemoveMachine(alias) });
        cx.notify();
    }

    /// "N spaces · M agents" on this computer only (the store also holds every machine's).
    pub fn local_summary(&self) -> String {
        let local = |m: &str| m.is_empty() || m == "local";
        let spaces = self.store.spaces.iter().filter(|s| local(s.machine.as_deref().unwrap_or(""))).count();
        let agents = self.store.panes.iter().filter(|p| local(&p.machine)).count();
        format!("{spaces} spaces · {agents} agents")
    }
}

/// One list line: name over a grey detail, then the buttons and the status dot on the right.
fn line(name: &str, detail: String, dot: u32, buttons: Div) -> Div {
    div().flex().items_center().gap(px(10.)).py(px(12.)).border_b_1().border_color(theme::border())
        .child(div().flex_1().min_w_0().flex().flex_col().gap(px(3.))
            .child(div().text_size(px(14.)).text_color(rgb(fg())).child(name.to_string()))
            .child(div().truncate().text_size(px(12.5)).text_color(rgb(dim())).child(detail)))
        .child(buttons)
        .child(div().size(px(8.)).flex_none().rounded_full().bg(rgb(dot)))
}

pub fn body(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let mut list = div().flex().flex_col()
        .child(line("This computer", format!("Connected · {}", app.local_summary()), done(), div()));
    for m in app.store.machines.as_array().into_iter().flatten() {
        let alias = m["alias"].as_str().unwrap_or("?").to_string();
        let target = m["target"].as_str().unwrap_or("").to_string();
        let snap = &m["snapshot"];
        let count = |k: &str| snap[k].as_array().map(|a| a.len()).unwrap_or(0);
        let (detail, dot) = match m["status"].as_str() {
            Some("connected") => (format!("dr {} · {} spaces · {} agents · {target}", m["info"]["version"].as_str().unwrap_or("?"), count("spaces"), count("panes")), done()),
            Some("connecting") => (format!("Connecting… · {target}"), working()),
            _ => (m["error"].as_str().unwrap_or("Unreachable. Agents there keep running; you'll see them when it's back.").to_string(), error()),
        };
        let (a, rename_alias, test_alias) = (alias.clone(), alias.clone(), alias.clone());
        let buttons = div().flex().gap(px(8.))
            .child(ui::button("Test", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
                app.test_saved_machine(test_alias.clone(), target.clone());
                cx.notify();
            })))
            .child(ui::button("Rename", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, window, cx| app.open_rename_machine(rename_alias.clone(), window, cx))))
            .child(ui::button("Remove", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| app.ask_remove_machine(a.clone(), cx))));
        list = list.child(line(&alias, detail, dot, buttons));
    }
    let page = div().max_w(px(820.)).flex().flex_col().gap(px(10.))
        .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(muted())).child("MACHINES"))
        .child(div().text_size(px(13.)).text_color(rgb(muted())).child("Your computers, reached over ssh. Their terminals keep running when you disconnect."))
        .child(list)
        .child(div().flex().pt(px(6.)).child(ui::button("Add a machine", Kind::Primary)
            .on_mouse_down(MouseButton::Left, cx.listener(|app, _, window, cx| app.open_add_machine(window, cx)))))
        .child(div().pt(px(14.)).text_size(px(12.)).text_color(rgb(dim())).child(
            "Phone: with Tailscale on both devices, run `dr server stop` then `dr server run --listen <tailscale-ip>`, and open the link from `dr server url` on the phone. Keep that link private."));
    div().flex().flex_col().flex_1().min_h_0().child(div().id("machines").flex_1().min_h_0().overflow_y_scroll().p(px(24.)).child(page))
}
