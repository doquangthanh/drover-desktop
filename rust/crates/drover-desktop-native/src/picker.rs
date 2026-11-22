//! The pop-up of Ctrl+P (jump to a terminal), Ctrl+Shift+P / F1 (all commands), F2 (rename) and Ctrl+F (find in the
//! focused terminal, find.rs), as the web UI's palette: a field on top, a list under it; ↑↓ pick, Enter runs, Esc closes.

use gpui::{Context, Div, Entity, Focusable, FontWeight, KeyDownEvent, MouseButton, Window, div, prelude::*, rgb, rgba};
use crate::ui_scale::px;
use serde_json::json;

use crate::theme::{self, accent, dim, fg, muted, UI_FONT};
use crate::views::View;
use crate::views::text_input::{InputEvent, TextInput};
use crate::widgets::{kind_icon, state_mark};
use crate::workspace_app::DroverApp;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Jump,
    Commands,
    Rename,
    RenameSpace,
    RenameTab,
    RenameMachine,
    InstallPlugin,
    NewWorktree,
    Find,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    NewTab, CloseTab, NextTab, PrevTab, SplitRight, SplitDown, ClosePane, Zoom, Sidebar,
    Agents, Files, Changes, Inbox, Machines, Settings, NextBlocked, NextDone, Shortcuts, Palette, ReloadConfig, PluginsReload, PluginsFolder, PluginInstall, Jump, Rename, RenameSpace, RenameTab, CopyMode, ClearPane, Find, SettleTab, UnsettleTabs, ZoomIn, ZoomOut, ZoomReset, SelectAll, AppZoomIn, AppZoomOut, AppZoomReset, WhatsNew, Feedback,
}

const COMMANDS: [(&str, &str, Cmd); 40] = [
    ("New tab", "Ctrl+N", Cmd::NewTab), ("Close tab", "Ctrl+W", Cmd::CloseTab), ("Next tab", "Ctrl+Tab", Cmd::NextTab),
    ("Previous tab", "Ctrl+Shift+Tab", Cmd::PrevTab), ("Split right", "Ctrl+\\", Cmd::SplitRight),
    ("Split down", "Ctrl+K Ctrl+\\", Cmd::SplitDown), ("Close pane", "Ctrl+K W", Cmd::ClosePane), ("Zoom pane", "Ctrl+K Ctrl+M", Cmd::Zoom),
    ("Toggle sidebar", "Ctrl+B", Cmd::Sidebar), ("Agents panel", "Ctrl+Shift+A", Cmd::Agents), ("Files panel", "Ctrl+Shift+E", Cmd::Files),
    ("Changes panel", "Ctrl+Shift+G", Cmd::Changes), ("Inbox", "Ctrl+Shift+I", Cmd::Inbox), ("Machines", "Ctrl+Shift+M", Cmd::Machines),
    ("Settings", "Ctrl+,", Cmd::Settings), ("Next agent that needs you", "F8", Cmd::NextBlocked), ("Next finished agent", "Shift+F8", Cmd::NextDone),
    ("Keyboard shortcuts", "Ctrl+K Ctrl+S", Cmd::Shortcuts), ("Jump to a terminal", "Ctrl+P", Cmd::Jump), ("Rename agent", "F2", Cmd::Rename),
    ("Rename space", "F2 after picking a space", Cmd::RenameSpace), ("Rename tab", "F2 after picking a tab", Cmd::RenameTab),
    ("Copy mode (select with the keyboard)", "Ctrl+K [", Cmd::CopyMode), ("Clear screen and scrollback", "", Cmd::ClearPane),
    ("Find in terminal", "Ctrl+F", Cmd::Find), ("Put this tab away (settle)", "", Cmd::SettleTab),
    ("Bring back settled tabs", "", Cmd::UnsettleTabs), ("Terminal text bigger", "Ctrl+=", Cmd::ZoomIn),
    ("Terminal text smaller", "Ctrl+-", Cmd::ZoomOut), ("Terminal text normal size", "Ctrl+0", Cmd::ZoomReset),
    ("Select all in terminal", "", Cmd::SelectAll),
    ("Interface bigger", "Ctrl+Alt+=", Cmd::AppZoomIn), ("Interface smaller", "Ctrl+Alt+-", Cmd::AppZoomOut),
    ("Interface normal size", "Ctrl+Alt+0", Cmd::AppZoomReset),
    ("What's new", "", Cmd::WhatsNew), ("Send feedback", "", Cmd::Feedback),
    ("Reload config (your shortcuts in keys.toml)", "", Cmd::ReloadConfig), ("Plugins: reload", "", Cmd::PluginsReload),
    ("Plugins: open the folder", "", Cmd::PluginsFolder), ("Plugins: install from GitHub", "", Cmd::PluginInstall),
];

pub struct Picker {
    pub kind: Kind,
    pub input: Entity<TextInput>,
    pub selected: usize,
    /// Find: the pane searched and what the last match said.
    pub pane: Option<String>,
    pub note: Option<String>,
    /// RenameSpace / RenameTab: the id being renamed.
    pub target: Option<String>,
    /// The field's text, kept here for find.rs.
    pub query: String,
}

/// One row: what it shows and what Enter does.
enum Row {
    Pane(String),
    Cmd(Cmd),
    /// A plugin action, by its "plugin.id.action" name.
    Plugin(String),
}

impl DroverApp {
    /// Rename the given tab (double-click on it), or the shown one.
    pub fn open_rename_tab(&mut self, tab: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = tab.or_else(|| self.tab.clone()) else { return };
        let Some(name) = self.store.tab(&id).map(|t| t.name.clone()) else { return };
        self.open_picker_with(Kind::RenameTab, ("New name", name), Some(id), window, cx);
    }

    /// F2: rename what was picked last: the space, the tab, or the focused agent.
    pub fn rename_picked(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use crate::workspace_app::Area;
        match self.area {
            Area::Space => self.open_rename_space(None, window, cx),
            Area::Tab => self.open_rename_tab(None, window, cx),
            Area::Pane => self.open_picker(Kind::Rename, window, cx),
        }
    }

    /// Rename the given space (double-click in the sidebar), or the current one.
    pub fn open_rename_space(&mut self, space: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = space.or_else(|| self.space.clone()) else { return };
        let Some(name) = self.store.spaces.iter().find(|s| s.id == id).map(|s| s.name.clone()) else { return };
        self.open_picker_with(Kind::RenameSpace, ("New name", name), Some(id), window, cx);
    }

    pub fn open_picker(&mut self, kind: Kind, window: &mut Window, cx: &mut Context<Self>) {
        let (placeholder, text) = match kind {
            Kind::Jump => ("Jump to a terminal or agent…", String::new()),
            Kind::Commands => ("Type a command…", String::new()),
            Kind::Find => ("Find in this terminal (Enter: older match)", String::new()),
            Kind::Rename => {
                let Some(p) = self.focused.as_deref().and_then(|id| self.store.pane(id)) else { return };
                ("New name", p.name.clone())
            }
            Kind::RenameSpace | Kind::RenameTab => return self.rename_picked(window, cx),
            Kind::RenameMachine => return,
            Kind::InstallPlugin => return self.open_plugin_install(window, cx),
            Kind::NewWorktree => ("Task name: a new branch dr/<task> in its own folder", String::new()),
        };
        self.open_picker_with(kind, (placeholder, text), None, window, cx);
    }

    pub fn open_picker_with(&mut self, kind: Kind, (placeholder, text): (&'static str, String), target: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| { let mut i = TextInput::new(placeholder, cx); i.set_text(&text, cx); i });
        cx.subscribe_in(&input, window, |app, input, ev, window, cx| match ev {
            InputEvent::Submit => app.picker_enter(window, cx),
            InputEvent::Changed => {
                let text = input.read(cx).value();
                if let Some(p) = app.picker.as_mut() { p.selected = 0; p.query = text; }
                if app.picker.as_ref().is_some_and(|p| p.kind == Kind::Find) { app.find_step(true); }
                cx.notify();
            }
            InputEvent::Cancel => {}
        }).detach();
        window.focus(&input.read(cx).focus_handle(cx));
        self.picker = Some(Picker { kind, input, selected: 0, pane: self.focused.clone(), note: None, target, query: text });
        cx.notify();
    }

    pub fn close_picker(&mut self, cx: &mut Context<Self>) {
        self.picker = None;
        self.want_term_focus = true;
        cx.notify();
    }

    fn picker_rows(&self, cx: &gpui::App) -> Vec<(Row, String, String)> {
        let Some(p) = &self.picker else { return vec![] };
        let q = fold(p.input.read(cx).value().trim());
        let has = |hay: String| q.is_empty() || fold(&hay).contains(&q);
        match p.kind {
            Kind::Jump => self.store.panes.iter().filter(|x| {
                let space = self.store.spaces.iter().find(|s| s.id == x.space_id).map(|s| s.name.as_str()).unwrap_or("");
                has(format!("{} {} {} {}", x.name, x.title, space, x.cwd))
            }).map(|x| {
                let space = self.store.spaces.iter().find(|s| s.id == x.space_id).map(|s| s.name.clone()).unwrap_or_default();
                (Row::Pane(x.id.clone()), x.name.clone(), format!("{space} · {}", crate::widgets::short_path(&x.cwd)))
            }).collect(),
            Kind::Commands => COMMANDS.iter().filter(|(l, _, _)| has(l.to_string()))
                .map(|(l, k, c)| (Row::Cmd(*c), l.to_string(), k.to_string()))
                .chain(self.plugin_actions().into_iter().filter(|(_, l)| has(l.clone())).map(|(id, l)| (Row::Plugin(id), l, "plugin".to_string())))
                .collect(),
            Kind::Rename | Kind::RenameSpace | Kind::RenameTab | Kind::RenameMachine | Kind::InstallPlugin | Kind::NewWorktree | Kind::Find => vec![],
        }
    }

    fn picker_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(kind) = self.picker.as_ref().map(|p| p.kind) else { return };
        match kind {
            Kind::Find => { self.find_step(false); cx.notify(); }
            Kind::Rename => {
                let name = self.picker.as_ref().map(|p| p.input.read(cx).text.trim().to_string()).unwrap_or_default();
                if let (Some(pane), false) = (self.picker.as_ref().and_then(|p| p.pane.clone()), name.is_empty()) {
                    self.rpc("pane.rename", json!({"pane": pane, "name": name}));
                }
                self.close_picker(cx);
            }
            Kind::InstallPlugin => {
                let input = self.picker.as_ref().map(|p| p.input.read(cx).text.trim().to_string()).unwrap_or_default();
                self.close_picker(cx);
                if !input.is_empty() { self.ask_install_plugin(&input, cx); }
            }
            Kind::NewWorktree => {
                let task = self.picker.as_ref().map(|p| p.input.read(cx).text.trim().to_string()).unwrap_or_default();
                self.close_picker(cx);
                if !task.is_empty() { self.start_in_space("", Some(&task), cx); }
            }
            Kind::RenameMachine => {
                let name = self.picker.as_ref().map(|p| p.input.read(cx).text.trim().to_string()).unwrap_or_default();
                if let (Some(old), false) = (self.picker.as_ref().and_then(|p| p.target.clone()), name.is_empty()) {
                    self.rename_machine(old, name);
                }
                self.close_picker(cx);
            }
            Kind::RenameSpace | Kind::RenameTab => {
                let name = self.picker.as_ref().map(|p| p.input.read(cx).text.trim().to_string()).unwrap_or_default();
                if let (Some(id), false) = (self.picker.as_ref().and_then(|p| p.target.clone()), name.is_empty()) {
                    if kind == Kind::RenameTab { self.rpc("tab.rename", json!({"tab": id, "name": name})); }
                    else { self.rpc("space.rename", json!({"space": id, "name": name})); }
                }
                self.close_picker(cx);
            }
            _ => {
                let rows = self.picker_rows(cx);
                let at = self.picker.as_ref().map(|p| p.selected).unwrap_or(0);
                self.close_picker(cx);
                match rows.into_iter().nth(at).map(|r| r.0) {
                    Some(Row::Pane(id)) => { self.show_pane(&id); window.focus(&self.term_focus); }
                    Some(Row::Cmd(c)) => self.run_cmd(c, window, cx),
                    Some(Row::Plugin(id)) => self.run_plugin_action(&id),
                    None => {}
                }
            }
        }
    }

    /// ↑↓ move, Esc closes (the field keeps the other keys).
    pub fn picker_key(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let n = self.picker_rows(cx).len();
        let Some(p) = self.picker.as_mut() else { return };
        match ev.keystroke.key.as_str() {
            "escape" => self.close_picker(cx),
            "down" if n > 0 => p.selected = (p.selected + 1) % n,
            "up" if n > 0 => p.selected = (p.selected + n - 1) % n,
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    pub fn run_cmd(&mut self, c: Cmd, window: &mut Window, cx: &mut Context<Self>) {
        let pane = self.focused.clone();
        match c {
            Cmd::NewTab => self.new_tab(cx),
            Cmd::CloseTab => if let Some(t) = self.tab.clone() { self.ask_close_tab(&t, cx) },
            Cmd::NextTab => self.step_tab(1, cx),
            Cmd::PrevTab => self.step_tab(-1, cx),
            Cmd::SplitRight => if let Some(p) = pane { self.split(&p, "row") },
            Cmd::SplitDown => if let Some(p) = pane { self.split(&p, "col") },
            Cmd::ClosePane => if let Some(p) = pane { self.ask_close_pane(&p, cx) },
            Cmd::Zoom => if let Some(p) = pane { self.toggle_zoom(&p) },
            Cmd::Sidebar => self.sidebar_open = !self.sidebar_open,
            Cmd::Agents => self.toggle_panel(View::Agents, cx),
            Cmd::Files => self.toggle_panel(View::Files, cx),
            Cmd::Changes => self.toggle_panel(View::Changes, cx),
            Cmd::Inbox => self.set_view(View::Inbox, window, cx),
            Cmd::Machines => self.set_view(View::Machines, window, cx),
            Cmd::Settings => self.set_view(View::Settings, window, cx),
            Cmd::WhatsNew => self.set_view(View::WhatsNew, window, cx),
            Cmd::Feedback => self.set_view(View::Feedback, window, cx),
            Cmd::NextBlocked => self.next_with_state("blocked", window, cx),
            Cmd::NextDone => self.next_with_state("done", window, cx),
            Cmd::Shortcuts => self.show_shortcuts = true,
            Cmd::Jump => self.open_picker(Kind::Jump, window, cx),
            Cmd::Rename => self.open_picker(Kind::Rename, window, cx),
            Cmd::RenameSpace => self.open_rename_space(None, window, cx),
            Cmd::RenameTab => self.open_rename_tab(None, window, cx),
            Cmd::CopyMode => self.enter_copy_mode(cx),
            Cmd::Palette => self.open_picker(Kind::Commands, window, cx),
            Cmd::PluginsReload => self.reload_plugins(true),
            Cmd::PluginsFolder => self.open_plugins_folder(),
            Cmd::PluginInstall => self.open_plugin_install(window, cx),
            Cmd::ReloadConfig => { self.settings.files = None; self.load_keymap(true); }
            Cmd::ClearPane => self.clear_focused_pane(),
            Cmd::Find => self.open_picker(Kind::Find, window, cx),
            Cmd::SettleTab => self.settle_current_tab(cx),
            Cmd::UnsettleTabs => self.unsettle_all(cx),
            Cmd::AppZoomIn => self.zoom_app(1, cx),
            Cmd::AppZoomOut => self.zoom_app(-1, cx),
            Cmd::AppZoomReset => self.zoom_app(0, cx),
            Cmd::ZoomIn => self.zoom_terminal(1.0),
            Cmd::ZoomOut => self.zoom_terminal(-1.0),
            Cmd::ZoomReset => self.zoom_terminal(0.0),
            Cmd::SelectAll => self.select_all(cx),
        }
        cx.notify();
    }
}

/// Lower case without Vietnamese accents: "filé" finds "Files", "Viet" finds "Việt" (Telex composes while you type).
fn fold(s: &str) -> String {
    const GROUPS: [(&str, char); 6] = [
        ("àáảãạăằắẳẵặâầấẩẫậ", 'a'), ("èéẻẽẹêềếểễệ", 'e'), ("ìíỉĩị", 'i'),
        ("òóỏõọôồốổỗộơờớởỡợ", 'o'), ("ùúủũụưừứửữự", 'u'), ("ỳýỷỹỵ", 'y'),
    ];
    s.to_lowercase().chars().map(|c| match c {
        'đ' => 'd',
        c => GROUPS.iter().find(|(set, _)| set.contains(c)).map(|(_, base)| *base).unwrap_or(c),
    }).collect()
}

pub fn overlay(app: &DroverApp, cx: &mut Context<DroverApp>) -> Option<Div> {
    let p = app.picker.as_ref()?;
    let title = match p.kind { Kind::Jump => "Jump to", Kind::Commands => "Commands", Kind::Rename => "Rename agent", Kind::RenameSpace => "Rename space", Kind::RenameTab => "Rename tab", Kind::RenameMachine => "Rename machine", Kind::InstallPlugin => "Install a plugin", Kind::NewWorktree => "New worktree", Kind::Find => "Find in terminal" };
    let mut list = div().flex().flex_col().gap(px(1.)).max_h(px(420.)).overflow_hidden();
    for (i, (row, label, detail)) in app.picker_rows(cx).into_iter().take(14).enumerate() {
        let on = i == p.selected;
        let lead = match &row {
            Row::Pane(id) => app.store.pane(id).map(|x| div().flex().items_center().gap(px(6.)).child(kind_icon(&x.kind))
                .child(state_mark(&x.state, x.exit_code, gpui::SharedString::from(format!("pk-{id}"))))),
            Row::Cmd(_) | Row::Plugin(_) => None,
        };
        list = list.child(div().h(px(32.)).flex_none().flex().items_center().gap(px(10.)).px(px(10.)).rounded(px(7.))
            .when(on, |d| d.bg(theme::selected())).hover(|d| d.bg(theme::hover()))
            .children(lead)
            .child(div().flex_1().min_w_0().truncate().text_color(rgb(fg())).child(label))
            .child(div().flex_none().text_size(px(11.5)).text_color(rgb(if matches!(row, Row::Cmd(_) | Row::Plugin(_)) { accent() } else { dim() })).child(detail))
            .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, window, cx| {
                if let Some(p) = app.picker.as_mut() { p.selected = i; }
                app.picker_enter(window, cx);
                cx.stop_propagation();
            })));
    }
    let card = div().w(px(600.)).mt(px(90.)).p(px(12.)).flex().flex_col().gap(px(8.)).rounded(px(12.)).bg(rgb(crate::theme::card()))
        .border_1().border_color(theme::border2()).shadow_lg().font_family(UI_FONT)
        .child(div().px(px(4.)).text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(muted())).child(title))
        .child(p.input.clone())
        .children(p.note.clone().map(|n| div().px(px(4.)).text_size(px(12.)).text_color(rgb(dim())).child(n)))
        .child(list)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
    Some(div().absolute().top_0().left_0().size_full().flex().flex_col().items_center().bg(rgba(0x00000059))
        .occlude().on_key_down(cx.listener(DroverApp::picker_key)).child(card)
        .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.close_picker(cx))))
}
