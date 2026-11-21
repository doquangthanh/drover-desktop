//! In-app toasts (port of `shell/toasts.tsx`, `core/toast-store.ts`, `core/update-notifier.ts`): a pane you are not
//! looking at went blocked / done / exited, an agent tried to answer a prompt, a newer release exists, or a request
//! failed. At most 4, bottom right, tone-coloured accent bar and badge, an action button, a countdown bar that
//! pauses while the pointer is over the stack, a slide-in.

use std::time::{Duration, Instant};

use gpui::{Animation, AnimationExt, Context, Div, FontWeight, IntoElement, MouseButton, div, ease_out_quint, prelude::*, relative, rgb};
use crate::ui_scale::px;
use serde_json::{Value, json};

use crate::theme::{self, blocked, dim, done, error, fg, muted, working};
use crate::views::View;
use crate::widgets::{icon, kind_icon};
use crate::workspace_app::DroverApp;

pub const MAX_TOASTS: usize = 4;
const FIRST_UPDATE_CHECK: Duration = Duration::from_secs(15);
const RECHECK_UPDATE: Duration = Duration::from_secs(12 * 60 * 60);
/// Index of "Updates" in the Settings sections (views/settings.rs).
const UPDATES_SECTION: usize = 7;

#[derive(Clone, Copy, PartialEq)]
pub enum Tone { Blocked, Done, Exited, Error, Update }

impl Tone {
    pub fn color(self) -> u32 {
        match self { Tone::Blocked => blocked(), Tone::Done => done(), Tone::Exited => dim(), Tone::Error => error(), Tone::Update => working() }
    }
    fn glyph(self) -> &'static str {
        match self { Tone::Blocked | Tone::Error => "alert", Tone::Done => "check", Tone::Exited => "close", Tone::Update => "up" }
    }
    /// "Needs you" lives longest, as in the web UI.
    fn life(self) -> Duration {
        Duration::from_millis(match self { Tone::Blocked => 15000, Tone::Update => 10000, Tone::Error => 9000, _ => 6000 })
    }
}

/// What a click on the toast does.
#[derive(Clone)]
pub enum Target { Pane(String), Inbox, Updates, Nothing }

pub struct Toast {
    id: u64,
    title: String,
    sub: String,
    kind: String,
    tone: Tone,
    action: &'static str,
    target: Target,
    life: Duration,
    left: Duration,
}

pub struct Toasts {
    list: Vec<Toast>,
    next_id: u64,
    hovered: bool,
    last_tick: Option<Instant>,
    /// When to ask the daemon for a newer release, and the version already announced (once per version).
    update_due: Option<Instant>,
    update_announced: String,
}

impl Default for Toasts {
    fn default() -> Self {
        Toasts { list: vec![], next_id: 0, hovered: false, last_tick: None,
                 update_due: Some(Instant::now() + FIRST_UPDATE_CHECK), update_announced: String::new() }
    }
}

impl Toasts {
    pub fn push(&mut self, title: String, sub: String, kind: String, tone: Tone, action: &'static str, target: Target) {
        // One card per pane: a newer state replaces the older one instead of stacking.
        if let Target::Pane(p) = &target {
            self.list.retain(|t| !matches!(&t.target, Target::Pane(q) if q == p));
        }
        self.next_id += 1;
        let life = tone.life();
        self.list.push(Toast { id: self.next_id, title, sub, kind, tone, action, target, life, left: life });
        let extra = self.list.len().saturating_sub(MAX_TOASTS);
        self.list.drain(..extra);
    }

    pub fn error(&mut self, what: String) {
        self.push("Something went wrong".into(), what, String::new(), Tone::Error, "", Target::Nothing);
    }

    /// Count the lifetimes down (frozen while hovered). True while something is on screen, to keep the bar moving.
    pub fn tick(&mut self) -> bool {
        let now = Instant::now();
        let dt = self.last_tick.replace(now).map(|t| now - t).unwrap_or_default();
        if self.list.is_empty() {
            self.last_tick = None;
            return false;
        }
        if !self.hovered {
            for t in &mut self.list {
                t.left = t.left.saturating_sub(dt);
            }
            self.list.retain(|t| !t.left.is_zero());
        }
        true
    }
}

impl DroverApp {
    /// An event from the daemon, seen before the store applies it: raise a toast when it asks for one.
    pub fn toast_for_event(&mut self, name: &str, data: &Value) {
        match name {
            "status" => {
                let state = data["new"].as_str().unwrap_or("");
                let Ok(pane) = serde_json::from_value::<drover_proto::wire::Pane>(data["pane"].clone()) else { return };
                if state == "blocked" {
                    self.settled.remove(&pane.tab_id); // an agent that needs you brings its settled tab back
                }
                if !state.is_empty() {
                    self.fire_plugin_event(state, &pane);
                }
                let (tone, title, action) = match state {
                    "blocked" => (Tone::Blocked, format!("{} needs you", pane.name), "Answer"),
                    "done" => (Tone::Done, format!("{} finished", pane.name), "Open"),
                    "exited" => (Tone::Exited, format!("{} stopped", pane.name), "Open"),
                    _ => return,
                };
                // [notify] decides who gets a toast: an event whose channels leave "toast" out gets none.
                if self.store.notify.get(state).and_then(Value::as_array).is_some_and(|c| !c.iter().any(|x| x == "toast")) {
                    return;
                }
                let seen = self.window_active && self.view == View::Terminals && self.focused.as_deref() == Some(pane.id.as_str());
                if seen {
                    return;
                }
                let sub = if pane.title.is_empty() { pane.command.clone() } else { pane.title.clone() };
                self.toasts.push(title, sub, pane.kind.clone(), tone, action, Target::Pane(pane.id));
            }
            "policy" => self.toasts.push("An agent tried to answer a prompt".into(), "Open the Inbox to allow or deny it.".into(),
                String::new(), Tone::Blocked, "Review", Target::Inbox),
            _ => {}
        }
    }

    /// Ask the daemon (which asks GitHub) for a newer release soon after start and twice a day; toast once per version.
    pub fn check_update_when_due(&mut self) {
        if !self.connected || !self.toasts.update_due.is_some_and(|t| Instant::now() >= t) {
            return;
        }
        self.toasts.update_due = Some(Instant::now() + RECHECK_UPDATE);
        // Offline or GitHub rate limit: stay quiet, try again later.
        self.rpc_reply("update.check", json!({}), |app, r| {
            let Ok(info) = r else { return };
            let latest = info["latest"].as_str().unwrap_or("").to_string();
            if info["available"] != true || latest.is_empty() || latest == app.toasts.update_announced {
                return;
            }
            let installable = info["installable"] == true;
            let sub = if installable { "Install it from Settings → Updates" } else { "See Settings → Updates" };
            app.toasts.update_announced = latest.clone();
            app.toasts.push(format!("Drover v{latest} is available"), sub.into(), String::new(), Tone::Update,
                if installable { "Update" } else { "View" }, Target::Updates);
        });
    }

    /// "Test notification": the daemon's desktop popup, plus a sample in-app toast.
    pub fn test_notification(&mut self) {
        self.rpc("notify.test", json!({}));
        self.toasts.push("Drover notifications are on".into(), "You'll see one like this when an agent needs you or finishes.".into(),
            String::new(), Tone::Done, "", Target::Nothing);
    }

    fn run_toast(&mut self, id: u64, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let Some(i) = self.toasts.list.iter().position(|t| t.id == id) else { return };
        let t = self.toasts.list.remove(i);
        match t.target {
            Target::Pane(p) => { self.show_pane(&p); self.focus_pane(p, window, cx); }
            Target::Inbox => self.view = View::Inbox,
            Target::Updates => { self.view = View::Settings; self.settings.section = UPDATES_SECTION; }
            Target::Nothing => {}
        }
        cx.notify();
    }
}

/// The card's look, without behaviour: also the static sample in the Inbox's notification panel.
pub fn card_face(title: &str, sub: &str, kind: &str, tone: Tone) -> Div {
    let c = tone.color();
    // The agent's badge with a small tone glyph on its corner, or the tone glyph alone.
    let badge = div().relative().size(px(28.)).flex_none().flex().items_center().justify_center().rounded(px(8.))
        .bg(theme::tint(c, 0.14))
        .when(kind.is_empty(), |d| d.child(icon(tone.glyph(), 15., c)))
        .when(!kind.is_empty(), |d| d.child(kind_icon(kind))
            .child(div().absolute().right(px(-4.)).bottom(px(-4.)).size(px(14.)).rounded_full().flex().items_center().justify_center()
                .bg(rgb(c)).border_2().border_color(rgb(crate::theme::card())).child(icon(tone.glyph(), 9., theme::on_accent()))));
    let mut body = div().flex_1().min_w_0().flex().flex_col()
        .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(fg())).truncate().child(title.to_string()));
    if !sub.is_empty() {
        body = body.child(div().mt(px(2.)).text_size(px(12.)).text_color(rgb(muted())).max_h(px(34.)).overflow_hidden().child(sub.to_string()));
    }
    div().relative().overflow_hidden().flex().items_start().gap(px(10.)).py(px(11.)).pl(px(14.)).pr(px(10.))
        .rounded(px(12.)).bg(rgb(crate::theme::card())).border_1().border_color(theme::border2()).shadow_lg()
        .child(div().absolute().left_0().top_0().bottom_0().w(px(3.)).bg(rgb(c)))
        .child(badge).child(body)
}

fn card(t: &Toast, cx: &mut Context<DroverApp>) -> impl IntoElement {
    let id = t.id;
    let c = t.tone.color();
    let close = div().flex_none().size(px(20.)).flex().items_center().justify_center().text_size(px(15.)).text_color(rgb(dim()))
        .hover(|d| d.text_color(rgb(fg()))).child("×")
        .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
            cx.stop_propagation();
            app.toasts.list.retain(|t| t.id != id);
            cx.notify();
        }));
    let left = t.left.as_secs_f32() / t.life.as_secs_f32();
    card_face(&t.title, &t.sub, &t.kind, t.tone).id(("toast", id as usize)).cursor_pointer()
        .hover(|d| d.border_color(theme::tint(c, 0.45)))
        .when(!t.action.is_empty(), |d| d.child(div().flex_none().mt(px(2.)).h(px(24.)).px(px(10.)).rounded(px(6.)).flex().items_center()
            .text_size(px(11.5)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(c))
            .bg(theme::tint(c, 0.12)).border_1().border_color(theme::tint(c, 0.3)).hover(|d| d.bg(theme::tint(c, 0.22)))
            .child(t.action)))
        .child(close)
        .child(div().absolute().left_0().bottom_0().h(px(2.)).w(relative(left)).bg(theme::tint(c, 0.55)))
        .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, window, cx| app.run_toast(id, window, cx)))
        .with_animation(("toast-in", id as usize), Animation::new(Duration::from_millis(280)).with_easing(ease_out_quint()),
            |el, t| el.opacity(t).ml(px((1. - t) * 24.)))
}

/// The stack, bottom right above the status bar. None when empty.
pub fn overlay(app: &DroverApp, cx: &mut Context<DroverApp>) -> Option<gpui::Stateful<Div>> {
    if app.toasts.list.is_empty() {
        return None;
    }
    let mut stack = div().id("toasts").absolute().right(px(16.)).bottom(px(40.)).w(px(360.)).flex().flex_col().gap(px(10.))
        .on_hover(cx.listener(|app, h: &bool, _, _| app.toasts.hovered = *h));
    for t in &app.toasts.list {
        stack = stack.child(card(t, cx));
    }
    Some(stack)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane_toast(t: &mut Toasts, pane: &str, tone: Tone) {
        t.push(format!("{pane} x"), String::new(), "claude".into(), tone, "Open", Target::Pane(pane.into()));
    }

    #[test]
    fn keeps_at_most_four_and_drops_the_oldest() {
        let mut t = Toasts::default();
        for i in 0..6 { pane_toast(&mut t, &format!("p{i}"), Tone::Done); }
        assert_eq!(t.list.len(), MAX_TOASTS);
        assert_eq!(t.list[0].title, "p2 x");
    }

    #[test]
    fn a_newer_state_of_the_same_pane_replaces_the_old_toast() {
        let mut t = Toasts::default();
        pane_toast(&mut t, "p1", Tone::Blocked);
        pane_toast(&mut t, "p1", Tone::Done);
        assert_eq!(t.list.len(), 1);
        assert!(t.list[0].tone == Tone::Done);
    }

    #[test]
    fn errors_stack_and_do_not_replace_each_other() {
        let mut t = Toasts::default();
        t.error("a".into());
        t.error("b".into());
        assert_eq!(t.list.len(), 2);
    }

    #[test]
    fn expires_after_its_life_but_not_while_hovered() {
        let mut t = Toasts::default();
        pane_toast(&mut t, "p1", Tone::Done);
        assert!(t.tick());
        t.hovered = true;
        t.list[0].left = Duration::from_millis(1);
        std::thread::sleep(Duration::from_millis(5));
        t.tick();
        assert_eq!(t.list.len(), 1, "frozen while hovered");
        t.hovered = false;
        std::thread::sleep(Duration::from_millis(5));
        t.tick();
        assert!(t.list.is_empty());
        assert!(!t.tick(), "nothing on screen: no repaint needed");
    }

    #[test]
    fn blocked_lives_longest() {
        assert!(Tone::Blocked.life() > Tone::Update.life() && Tone::Update.life() > Tone::Done.life());
    }
}
