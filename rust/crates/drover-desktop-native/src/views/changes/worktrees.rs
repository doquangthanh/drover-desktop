//! Changes view, left panel (`worktree-list.tsx`): one row per worktree (the agent working in it, its branch and
//! how many files it changed), then Agent here / Remove for a task worktree and Clean up.

use drover_proto::changes::Worktree;
use drover_proto::wire::Pane;
use gpui::{Context, Div, MouseButton, div, prelude::*, rgb};
use crate::ui_scale::px;
use serde_json::json;

use crate::theme::{self, accent, blocked, dim, fg, muted, yellow, tint};
use crate::views::ui::{self, Kind};
use crate::widgets::{kind_icon, state_mark};
use crate::workspace_app::DroverApp;

/// The agent whose folder is inside `path` (task worktrees only).
fn agent_in<'a>(app: &'a DroverApp, path: &str) -> Option<&'a Pane> {
    app.store.panes.iter().find(|p| p.kind != "shell" && !p.cwd.is_empty() && p.cwd.starts_with(path))
}

fn count(t: &Worktree) -> Div {
    match t.changes.filter(|n| *n > 0) {
        Some(n) => div().text_color(rgb(yellow())).child(format!("{n} files")),
        None => div().text_color(rgb(dim())).child("clean"),
    }
}

fn row(app: &DroverApp, t: &Worktree, agent: Option<&Pane>, on: bool, cx: &mut Context<DroverApp>) -> Div {
    let path = t.path.clone();
    let branch = t.branch.clone().unwrap_or("(detached)".into());
    let top = div().flex().items_center().gap(px(7.))
        .children(agent.map(|a| kind_icon(&a.kind)))
        .child(div().flex_1().min_w_0().truncate().text_color(rgb(fg())).child(agent.map(|a| a.name.clone()).unwrap_or(branch.clone())))
        .child(match agent {
            Some(a) => div().child(state_mark(&a.state, a.exit_code, gpui::SharedString::from(format!("wt-{}", a.id)))),
            None if t.missing => div().text_size(px(11.)).text_color(rgb(dim())).child("folder deleted"),
            None => count(t).text_size(px(11.)),
        });
    let sub = match agent {
        Some(_) => div().flex().gap(px(4.)).child(div().text_color(rgb(if on { accent() } else { dim() })).child(branch)).child("·").child(count(t)),
        None => div().truncate().child(t.path.clone()),
    };
    let _ = app;
    div().flex_none().flex().flex_col().gap(px(3.)).px(px(10.)).py(px(8.)).rounded(px(8.))
        .when(t.missing, |d| d.opacity(0.5))
        .when(on, |d| d.bg(theme::selected())).when(!on && !t.missing, |d| d.hover(|d| d.bg(theme::hover())))
        .child(top).child(sub.text_size(px(11.)).text_color(rgb(dim())))
        .when(!t.missing, |d| d.on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
            app.changes.picked_path = Some(path.clone());
            app.changes.picked_file = None;
            app.changes.confirm_remove = false;
            app.changes.show_trees = false;
            cx.notify();
        })))
}

pub fn panel(app: &DroverApp, trees: &[Worktree], path: Option<&str>, cx: &mut Context<DroverApp>) -> Div {
    let mut list = div().id("changes-worktrees").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().gap(px(2.)).p(px(6.));
    for (i, t) in trees.iter().enumerate() {
        let agent = if i == 0 { None } else { agent_in(app, &t.path) };
        list = list.child(row(app, t, agent, Some(t.path.as_str()) == path, cx));
    }
    let missing = trees.iter().filter(|t| t.missing).count();
    let current = trees.iter().position(|t| Some(t.path.as_str()) == path);
    let mut foot = div().flex_none().flex().flex_col().gap(px(8.)).p(px(10.)).border_t_1().border_color(theme::border());
    if missing > 0 {
        let root = match &app.changes.info { Some(Ok(i)) => i.root.clone(), _ => String::new() };
        foot = foot.child(ui::button(&format!("Clean up {missing} deleted"), Kind::Plain)
            .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
                app.changes_run("Clean up", "worktree.prune", json!({"repo": root}));
                cx.notify();
            })));
    }
    if let Some(t) = current.filter(|i| *i > 0).map(|i| &trees[i]) {
        let dirty = t.changes.unwrap_or(0) > 0;
        foot = foot.child(div().flex().gap(px(8.))
            .child(ui::button("Agent here", Kind::Plain).on_mouse_down(MouseButton::Left, {
                let cwd = t.path.clone();
                cx.listener(move |app, ev: &gpui::MouseDownEvent, _, cx| { app.open_agent_here_menu(ev.position, cwd.clone()); cx.notify(); })
            }))
            .child(ui::button("Remove", Kind::Danger).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| {
                app.changes.confirm_remove = true;
                cx.notify();
            }))));
        if app.changes.confirm_remove {
            let (gone, name) = (t.path.clone(), agent_in(app, &t.path).map(|a| a.name.clone()).or(t.branch.clone()).unwrap_or_default());
            let detail = if dirty {
                format!("It has {} uncommitted files, so Drover won't delete it unless you force it.", t.changes.unwrap_or(0))
            } else {
                format!("{} will be deleted.", t.path)
            };
            foot = foot.child(div().flex().flex_col().gap(px(8.)).p(px(10.)).rounded(px(8.)).bg(tint(blocked(), 0.1))
                .child(div().text_color(rgb(fg())).child(format!("Remove “{name}”?")))
                .child(div().text_size(px(12.)).text_color(rgb(muted())).child(detail))
                .child(div().flex().gap(px(8.))
                    .child(ui::button(if dirty { "Remove anyway" } else { "Remove" }, Kind::Danger)
                        .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
                            let params = if dirty { json!({"path": gone, "force": true}) } else { json!({"path": gone}) };
                            app.changes.confirm_remove = false;
                            app.changes.picked_path = None;
                            app.changes_run("Remove", "worktree.remove", params);
                            cx.notify();
                        })))
                    .child(ui::button("Cancel", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| {
                        app.changes.confirm_remove = false;
                        cx.notify();
                    })))));
        }
    }
    div().flex().flex_col().flex_1().min_h_0().child(list).child(foot)
}
