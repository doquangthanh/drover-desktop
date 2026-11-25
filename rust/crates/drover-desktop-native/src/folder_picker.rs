//! "Choose a folder" (the web UI's in-app folder picker): Places and recent folders on the left, the sub-folders of
//! the open folder on the right (click picks, double click opens, ↑ goes up), a filter, and "Use this folder", which
//! opens it as a new space. It browses the machine of the focused pane. Escape or a click outside closes it.

use gpui::{Context, Div, Entity, Focusable, FontWeight, MouseButton, MouseDownEvent, Window, div, prelude::*, rgb, rgba};
use serde_json::json;

use crate::folder_picker_logic::{self as logic, DirListing};
use crate::theme::{self, dim, fg, muted, UI_FONT};
use crate::ui_scale::px;
use crate::views::text_input::{InputEvent, TextInput};
use crate::views::ui::{self, Kind};
use crate::widgets::icon;
use crate::workspace_app::DroverApp;

pub struct FolderPicker {
    /// The machine browsed (`None` = this computer).
    pub machine: Option<String>,
    pub listing: Option<DirListing>,
    pub chosen: Option<String>,
    pub filter: Entity<TextInput>,
    pub error: Option<String>,
}

impl DroverApp {
    /// Open the picker on the machine of the focused pane, starting in the focused space's folder (else the home folder).
    pub fn open_folder_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pane = self.focused.as_deref().and_then(|p| self.store.pane(p));
        let machine = pane.map(|p| p.machine.clone()).filter(|m| !m.is_empty() && m != "local");
        let start = pane.map(|p| p.cwd.clone()).unwrap_or_default();
        let filter = cx.new(|cx| TextInput::new("Filter folders", cx));
        cx.subscribe(&filter, |_, _, ev, cx| if matches!(ev, InputEvent::Changed) { cx.notify() }).detach();
        window.focus(&filter.read(cx).focus_handle(cx));
        self.folder_picker = Some(FolderPicker { machine, listing: None, chosen: None, filter, error: None });
        self.folder_list(&start, cx);
    }

    pub fn close_folder_picker(&mut self, cx: &mut Context<Self>) {
        self.folder_picker = None;
        self.want_term_focus = true;
        cx.notify();
    }

    /// List the sub-folders of `path` ("" = the first allowed root) and show them.
    pub fn folder_list(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(f) = &self.folder_picker else { return };
        let mut params = json!({"path": path});
        if let Some(m) = &f.machine { params["machine"] = json!(m); }
        f.filter.update(cx, |i, cx| i.set_text("", cx)); // a filter for the old folder would hide the new one's rows
        self.rpc_reply("fs.list_dirs", params, |app, r| {
            if let Some(f) = &mut app.folder_picker {
                match r.map(|v| DirListing::from_json(&v)) {
                    Ok(Some(l)) => (f.chosen, f.listing, f.error) = (Some(l.path.clone()), Some(l), None),
                    Ok(None) => f.error = Some("unexpected reply from fs.list_dirs".into()),
                    Err(e) => f.error = Some(e),
                }
            }
        });
        cx.notify();
    }

    /// Open the chosen folder as a new space (on that machine) and close the picker.
    pub fn folder_use(&mut self, cx: &mut Context<Self>) {
        if let Some(f) = &self.folder_picker && let Some(path) = f.chosen.clone().or_else(|| f.listing.as_ref().map(|l| l.path.clone())) {
            let mut params = json!({"cwd": path, "new": true});
            if let Some(m) = &f.machine { params["machine"] = json!(m); }
            self.rpc_then("space.create", params, Self::focus_created);
        }
        self.close_folder_picker(cx);
    }
}

fn place(label: &str, path: &str, on: bool, cx: &mut Context<DroverApp>) -> Div {
    let path = path.to_string();
    div().h(px(28.)).px(px(8.)).flex().items_center().gap(px(8.)).rounded(px(6.)).text_size(px(12.5)).text_color(rgb(fg()))
        .when(on, |d| d.bg(theme::selected())).when(!on, |d| d.hover(|d| d.bg(theme::hover())))
        .child(icon("folder", 13., dim())).child(div().flex_1().min_w_0().truncate().child(label.to_string()))
        .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| app.folder_list(&path, cx)))
}

fn folder_row(e: &logic::DirEntry, on: bool, now: f64, cx: &mut Context<DroverApp>) -> Div {
    let path = e.path.clone();
    let hidden = e.name.starts_with('.');
    div().h(px(30.)).px(px(8.)).flex().items_center().gap(px(8.)).rounded(px(6.)).text_size(px(13.))
        .text_color(rgb(if hidden { dim() } else { fg() }))
        .when(on, |d| d.bg(theme::selected())).when(!on, |d| d.hover(|d| d.bg(theme::hover())))
        .child(icon("folder", 14., dim())).child(div().flex_1().min_w_0().truncate().child(e.name.clone()))
        .when(e.is_git, |d| d.child(div().text_size(px(11.)).text_color(rgb(theme::accent())).child("git")))
        .child(div().w(px(90.)).text_size(px(12.)).text_color(rgb(dim())).flex().justify_end()
            .child(if hidden { "hidden".to_string() } else if e.mtime > 0.0 { logic::folder_age(e.mtime, now) } else { String::new() }))
        .on_mouse_down(MouseButton::Left, cx.listener(move |app, ev: &MouseDownEvent, _, cx| {
            if ev.click_count >= 2 { app.folder_list(&path, cx); } else if let Some(f) = &mut app.folder_picker { f.chosen = Some(path.clone()); cx.notify(); }
        }))
}

pub fn overlay(app: &DroverApp, cx: &mut Context<DroverApp>) -> Option<Div> {
    let f = app.folder_picker.as_ref()?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    let (here, parent) = f.listing.as_ref().map(|l| (l.path.clone(), l.parent.clone())).unwrap_or_default();
    let query = f.filter.read(cx).value();

    let mut places = div().w(px(190.)).flex_none().flex().flex_col().gap(px(2.)).child(ui::label("Places"));
    if let Some(l) = &f.listing {
        for (i, r) in l.roots.iter().enumerate() {
            places = places.child(place(if i == 0 { "Home" } else { logic::base_name(r) }, r, *r == l.path, cx));
        }
        let spaces: Vec<_> = app.store.spaces.iter().map(|s| (s.machine.clone(), s.cwd.clone())).collect();
        let recent = logic::recent_folders(&spaces, f.machine.as_deref(), 6);
        if !recent.is_empty() { places = places.child(div().pt(px(10.)).child(ui::label("Recent"))); }
        for r in recent { places = places.child(place(logic::base_name(&r), &r, false, cx)); }
    }

    let mut list = div().id("folder-picker-list").flex().flex_col().h(px(300.)).overflow_y_scroll();
    match (&f.error, &f.listing) {
        (Some(e), _) => list = list.child(ui::note(e.clone(), true)),
        (None, None) => list = list.child(ui::note("Loading…", false)),
        (None, Some(l)) => {
            let shown = logic::filter_entries(&l.entries, &query);
            if shown.is_empty() { list = list.child(ui::note("No sub-folders", false)); }
            for e in shown { list = list.child(folder_row(e, f.chosen.as_deref() == Some(e.path.as_str()), now, cx)); }
        }
    }

    let up = parent.clone();
    let card = div().w(px(760.)).p(px(22.)).flex().flex_col().gap(px(12.)).rounded(px(12.)).bg(rgb(theme::card()))
        .border_1().border_color(theme::border2()).shadow_lg().font_family(UI_FONT)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(div().flex().items_center().gap(px(8.))
            .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(fg())).child("Choose a folder"))
            .child(div().flex_1().text_size(px(12.)).text_color(rgb(muted())).child(format!("on {}", f.machine.as_deref().unwrap_or("this computer"))))
            .child(div().w(px(220.)).child(f.filter.clone())))
        .child(div().flex().gap(px(14.)).child(places).child(div().flex_1().min_w_0().flex().flex_col().gap(px(8.))
            .child(div().flex().items_center().gap(px(8.))
                .child(ui::button("↑", Kind::Plain).when(parent.is_none(), |d| d.opacity(0.45))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| if let Some(p) = &up { app.folder_list(p, cx) })))
                .child(div().flex_1().min_w_0().truncate().text_size(px(12.5)).text_color(rgb(fg())).child(here.clone())))
            .child(list)))
        .child(div().flex().items_center().gap(px(8.))
            .child(div().flex_1().min_w_0().truncate().text_size(px(12.)).text_color(rgb(dim())).child(f.chosen.clone().unwrap_or(here)))
            .child(ui::button("Cancel", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.close_folder_picker(cx))))
            .child(ui::button("Use this folder", Kind::Primary).when(f.listing.is_none(), |d| d.opacity(0.45))
                .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.folder_use(cx)))));
    Some(div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().bg(rgba(0x00000073))
        .occlude().child(card)
        .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.close_folder_picker(cx))))
}
