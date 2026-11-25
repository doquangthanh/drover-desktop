//! Files view, the agent beside the editor (`agent-dock.tsx`): a header with the agent's state, a picker when the
//! space has several, ✕ to fold it away; folded it is a thin "Agent ▸" tab that brings it back.

use drover_proto::wire::Pane;
use gpui::{Context, Div, FontWeight, MouseButton, div, prelude::*, rgb};

use crate::context_menu::{Act, Entry, Icon};
use crate::theme::{self, dim, muted, pane};
use crate::ui_scale::px;
use crate::widgets::{kind_icon, state_mark};
use crate::workspace_app::DroverApp;

/// Every terminal that is still alive, of any space: picking one shows its folder's files.
fn live_panes(app: &DroverApp) -> Vec<&Pane> {
    app.store.panes.iter().filter(|p| p.state != "exited").collect()
}

impl DroverApp {
    /// The dock's picker: show this agent, its space and tab, keep the Files view open and follow its folder.
    pub fn dock_pick(&mut self, id: String, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let Some((space, tab)) = self.store.pane(&id).map(|p| (p.space_id.clone(), p.tab_id.clone())) else { return };
        if let (Some(old), Some(view)) = (self.focused.clone(), self.panel()) {
            self.panels.remove(&old);
            self.panels.insert(id.clone(), view);
        }
        self.docs.dock_pane = Some(id.clone());
        self.space = Some(space);
        self.tab = Some(tab);
        self.sizes.lock().unwrap().clear();
        self.focus_pane(id, window, cx);
    }
}

/// The pane shown: the picked one, else the focused one, else an agent, else the first.
fn current<'a>(app: &DroverApp, list: &[&'a Pane]) -> Option<&'a Pane> {
    let picked = app.docs.dock_pane.as_ref().and_then(|id| list.iter().find(|p| &p.id == id));
    let focused = app.focused.as_ref().and_then(|id| list.iter().find(|p| &p.id == id));
    picked.or(focused).or_else(|| list.iter().find(|p| p.kind != "shell")).or(list.first()).copied()
}

fn label(p: &Pane) -> String {
    if !p.name.is_empty() { p.name.clone() } else if !p.kind.is_empty() { p.kind.clone() } else { "terminal".into() }
}

pub fn dock(app: &DroverApp, cx: &mut Context<DroverApp>) -> Option<Div> {
    let list = live_panes(app);
    let cur = current(app, &list)?;
    if !app.docs.dock {
        return Some(div().w(px(26.)).flex_none().my(px(8.)).ml(px(8.)).mr(px(8.)).flex().flex_col().items_center().justify_center().rounded(px(10.))
            .border_1().border_color(theme::border()).bg(rgb(pane())).hover(|d| d.bg(theme::hover()))
            .child(gpui::svg().path("icons/agent-tab.svg").w(px(14.)).h(px(50.)).flex_none().text_color(rgb(muted())))
            .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| { app.docs.dock = true; cx.notify(); })));
    }
    let name = label(cur);
    let picker = if list.len() > 1 {
        let entries: Vec<Entry> = list.iter().map(|p| Entry::Item { label: format!("{} · {}", label(p), p.state), icon: Some(Icon::Kind(p.kind.clone())),
            hint: app.store.spaces.iter().find(|x| x.id == p.space_id).map(|x| x.name.clone()).unwrap_or_default(), key: None, act: Some(Act::DockPane(p.id.clone())) }).collect();
        let mut pick = div().h(px(24.)).max_w(px(220.)).px(px(8.)).flex().items_center().gap(px(6.)).rounded(px(6.)).border_1()
            .border_color(theme::border2()).bg(theme::selected()).text_size(px(12.)).font_weight(FontWeight::MEDIUM)
            .child(div().truncate().child(format!("{name} · {}", cur.state))).child(div().text_color(rgb(dim())).child("▾"));
        pick = pick.on_mouse_down(MouseButton::Left, cx.listener(move |app, ev: &gpui::MouseDownEvent, _, cx| {
            let entries = entries.iter().map(|e| match e {
                Entry::Item { label, icon, hint, key, act } => Entry::Item { label: label.clone(), icon: icon.clone(), hint: hint.clone(), key: key.clone(), act: act.clone() },
                Entry::Head(h) => Entry::Head(h.clone()),
                Entry::Rule => Entry::Rule,
            }).collect();
            app.open_menu(ev.position, entries, cx);
        }));
        pick
    } else {
        div().text_size(px(12.)).font_weight(FontWeight::MEDIUM).child(name)
    };
    let head = div().h(px(34.)).flex_none().flex().items_center().gap(px(8.)).px(px(12.)).border_b_1().border_color(theme::border())
        .child(kind_icon(&cur.kind))
        .child(picker)
        .child(state_mark(&cur.state, cur.exit_code, gpui::SharedString::from(format!("dock-{}", cur.id))))
        .child(div().flex_1())
        .child(div().size(px(22.)).rounded(px(5.)).flex().items_center().justify_center().text_size(px(12.)).text_color(rgb(dim()))
            .hover(|d| d.bg(theme::hover())).child("✕")
            .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| { app.docs.dock = false; cx.notify(); })));
    let body = crate::layout_view::pane_frame(app, &cur.id, true, cx);
    Some(div().w(gpui::relative(0.40)).max_w(px(460.)).flex_none().my(px(8.)).ml(px(8.)).mr(px(8.)).flex().flex_col().min_h_0().rounded(px(10.)).overflow_hidden()
        .border_1().border_color(theme::border()).bg(rgb(pane())).child(head).child(div().flex().flex_1().min_h_0().child(body)))
}
