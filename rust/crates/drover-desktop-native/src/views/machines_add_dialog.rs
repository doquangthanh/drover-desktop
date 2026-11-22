//! The "Add a machine" dialog over the Machines view: hosts from ~/.ssh as one-click rows (✓ Added once saved), a
//! field for how ssh reaches another one, and More options (name, port, key, install dr, test only).
//! Escape, Esc or a click outside closes it.

use gpui::{Context, Div, Entity, FontWeight, MouseButton, div, prelude::*, rgb, rgba};
use crate::ui_scale::px;

use crate::theme::{self, dim, done, error, fg, muted, UI_FONT};
use crate::views::text_input::TextInput;
use crate::views::ui::{self, Kind};
use crate::views::View;
use crate::widgets::icon;
use crate::workspace_app::DroverApp;

fn small(text: &str) -> Div {
    div().text_size(px(12.)).text_color(rgb(muted())).child(text.to_string())
}

fn field(label: &str, input: &Entity<TextInput>) -> Div {
    div().flex().flex_col().gap(px(4.)).child(small(label)).child(input.clone())
}

/// A host found on this computer: Added when a saved machine has it as name or target, else an Add button.
fn host_row(host: &str, added: bool, cx: &mut Context<DroverApp>) -> Div {
    let pick = host.to_string();
    div().h(px(36.)).flex().items_center().gap(px(10.)).px(px(10.)).rounded(px(7.)).hover(|d| d.bg(theme::hover()))
        .child(icon("server", 14., muted()))
        .child(div().flex_1().min_w_0().truncate().text_color(rgb(fg())).child(host.to_string()))
        .child(if added {
            div().flex().items_center().gap(px(5.)).text_size(px(12.5)).text_color(rgb(done())).child(icon("check", 13., done())).child("Added")
        } else {
            ui::button("Add", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| app.machine_add_host(&pick, cx)))
        })
}

pub fn overlay(app: &DroverApp, cx: &mut Context<DroverApp>) -> Option<Div> {
    let f = &app.machines_form;
    if !f.open || app.view != View::Machines {
        return None;
    }
    let saved: Vec<&str> = app.store.machines.as_array().into_iter().flatten()
        .flat_map(|m| [m["alias"].as_str(), m["target"].as_str()]).flatten().collect();
    let typed = !f.target.read(cx).text.trim().is_empty();
    let mut log = div().flex().flex_col().gap(px(3.)).text_size(px(12.));
    if let Some((steps, err)) = &f.log {
        for s in steps { log = log.child(div().text_color(rgb(fg())).child(s.clone())); }
        if let Some(e) = err { log = log.child(div().text_color(rgb(error())).child(e.clone())); }
    }
    let add = ui::button("Add", Kind::Primary).when(!typed, |d| d.opacity(0.45))
        .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.machine_add(cx)));
    let card = div().w(px(700.)).p(px(22.)).flex().flex_col().gap(px(14.)).rounded(px(12.)).bg(rgb(theme::card()))
        .border_1().border_color(theme::border2()).shadow_lg().font_family(UI_FONT)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(div().flex().items_center()
            .child(div().flex_1().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(fg())).child("Add a machine"))
            .child(div().text_size(px(12.)).text_color(rgb(dim())).child("Esc")
                .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.close_add_machine(cx)))))
        .child(div().text_size(px(13.5)).text_color(rgb(muted()))
            .child("A VPS, a Mac mini, a box at home: Drover reaches it over ssh, and agents there keep running while your laptop sleeps."))
        .when(!f.hosts.is_empty(), |d| d.child(div().flex().flex_col().gap(px(4.))
            .child(small("From your ssh config"))
            .children(f.hosts.iter().map(|h| host_row(h, saved.contains(&h.as_str()), cx)))))
        .child(div().flex().flex_col().gap(px(6.))
            .child(small("Or type how ssh reaches it"))
            .child(div().flex().items_center().gap(px(10.)).child(div().flex_1().child(f.target.clone())).child(add)))
        .child(div().flex().items_center().gap(px(6.)).text_size(px(12.)).text_color(rgb(muted()))
            .child(if f.more { "▾" } else { "▸" }).child("More options (name, port, key, install dr)")
            .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| { app.machines_form.more = !app.machines_form.more; cx.notify(); })))
        .when(f.more, |d| d
            .child(div().flex().gap(px(8.))
                .child(div().w(px(180.)).child(field("Name", &f.alias)))
                .child(div().w(px(110.)).child(field("Port", &f.port)))
                .child(div().flex_1().child(field("Key file", &f.key))))
            .child(div().flex().items_center().gap(px(8.)).child(ui::toggle(f.install))
                .child(div().text_size(px(12.5)).child("Install dr there if missing (Linux/macOS)"))
                .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| { app.machines_form.install = !app.machines_form.install; cx.notify(); })))
            .child(div().flex().items_center().gap(px(10.))
                .child(div().flex_1().text_size(px(12.)).text_color(rgb(dim()))
                    .child("Host keys are checked against known_hosts. The daemon there only listens on 127.0.0.1; Drover reaches it through the SSH tunnel."))
                .child(ui::button("Test only", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.machine_test(cx))))))
        .child(log);
    Some(div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().bg(rgba(0x00000073))
        .occlude().child(card)
        .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.close_add_machine(cx))))
}
