//! The pop-up of Ctrl+P (jump to a terminal), Ctrl+Shift+P / F1 (all commands), F2 (rename) and Ctrl+F (find in the
//! focused terminal, find.rs), as the web UI's palette: a field on top, a list under it; ↑↓ pick, Enter runs, Esc closes.

use gpui::{Context, Div, Entity, Focusable, FontWeight, KeyDownEvent, MouseButton, Window, div, prelude::*, rgb, rgba};
use crate::ui_scale::px;
use serde_json::json;

use crate::palette_search::{fold, mark_ranges, search};
use crate::theme::{self, accent, dim, fg, muted, UI_FONT};
use crate::views::View;
use crate::views::text_input::{InputEvent, TextInput};
use crate::widgets::{kind_icon, state_mark};
use crate::workspace_app::DroverApp;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Jump,
    /// Like Jump, but Enter peeks at the pane (peek.rs) instead of going there.
    Peek,
    Commands,
    Rename,
    RenameSpace,
    RenameTab,
    RenameMachine,
    /// The chat being renamed (chat/).
    RenameChat,
    InstallPlugin,
    NewWorktree,
    /// Check errors (quickfix.rs): file:line of failed checks; Enter opens the file there.
    Quickfix,
    Find,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    NewTab, CloseTab, NextTab, PrevTab, SplitRight, SplitDown, ClosePane, Zoom, Sidebar,
    Agents, Files, Changes, Inbox, Machines, Settings, NextBlocked, NextDone, Shortcuts, Palette, ReloadConfig, PluginsReload, PluginsFolder, PluginInstall, Jump, Rename, RenameSpace, RenameTab, CopyMode, ClearPane, Find, SettleTab, UnsettleTabs, ZoomIn, ZoomOut, ZoomReset, SelectAll, AppZoomIn, AppZoomOut, AppZoomReset, WhatsNew, Feedback, Peek, WideEditor, SendSelection, Quickfix, Broadcast, OpenFolder, Chats, NewChat,
}

const COMMANDS: [(&str, &str, Cmd); 48] = [
    ("New tab", "Ctrl+N", Cmd::NewTab), ("Close tab", "Ctrl+W", Cmd::CloseTab), ("Next tab", "Ctrl+Tab", Cmd::NextTab),
    ("Previous tab", "Ctrl+Shift+Tab", Cmd::PrevTab), ("Split right", "Ctrl+\\", Cmd::SplitRight),
    ("Split down", "Ctrl+K Ctrl+\\", Cmd::SplitDown), ("Close pane", "Ctrl+K W", Cmd::ClosePane), ("Zoom pane", "Ctrl+K Ctrl+M", Cmd::Zoom),
    ("Toggle sidebar", "Ctrl+B", Cmd::Sidebar), ("Agents panel", "Ctrl+Shift+A", Cmd::Agents), ("Files panel", "Ctrl+Shift+E", Cmd::Files),
    ("Changes panel", "Ctrl+Shift+G", Cmd::Changes), ("Inbox", "Ctrl+Shift+I", Cmd::Inbox), ("Machines", "Ctrl+Shift+M", Cmd::Machines),
    ("Settings", "Ctrl+,", Cmd::Settings), ("Next agent that needs you", "F8", Cmd::NextBlocked), ("Next finished agent", "Shift+F8", Cmd::NextDone),
    ("Keyboard shortcuts", "Ctrl+K Ctrl+S", Cmd::Shortcuts), ("Jump to a terminal", "Ctrl+P", Cmd::Jump), ("Peek at a terminal (stay where you are)", "Ctrl+K P", Cmd::Peek), ("Rename agent", "F2", Cmd::Rename),
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
    ("Files: wide editor / agent beside it", "Ctrl+J in the editor", Cmd::WideEditor), ("Send the editor selection to an agent", "Alt+S", Cmd::SendSelection),
    ("Check errors: jump to file:line from failed checks", "", Cmd::Quickfix),
    ("Send to several agents", "", Cmd::Broadcast), ("Open a folder as a new space…", "", Cmd::OpenFolder),
    ("Chats", "", Cmd::Chats), ("New chat in this folder", "", Cmd::NewChat),
];

/// The shortcut texts the palette shows (for tests that check a key is real).
#[cfg(test)]
pub fn command_keys() -> Vec<&'static str> { COMMANDS.iter().map(|(_, k, _)| *k).filter(|k| !k.is_empty()).collect() }

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
    /// Quickfix: the items once check.list answered.
    pub fixes: Vec<crate::quickfix::FixItem>,
}

/// One row: what it shows and what Enter does.
enum Row {
    Pane(String),
    Cmd(Cmd),
    /// A plugin action, by its "plugin.id.action" name.
    Plugin(String),
    Fix(crate::quickfix::FixItem),
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
            Kind::Peek => ("Peek at a terminal or agent…", String::new()),
            Kind::Commands => ("Type a command…", String::new()),
            Kind::Find => ("Find in this terminal (Enter: older match)", String::new()),
            Kind::Rename => {
                let Some(p) = self.focused.as_deref().and_then(|id| self.store.pane(id)) else { return };
                ("New name", p.name.clone())
            }
            Kind::RenameSpace | Kind::RenameTab => return self.rename_picked(window, cx),
            Kind::RenameMachine | Kind::RenameChat => return,
            Kind::InstallPlugin => return self.open_plugin_install(window, cx),
            Kind::NewWorktree => ("Task name: a new branch dr/<task> in its own folder", String::new()),
            Kind::Quickfix => ("Filter the check errors…", String::new()),
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
            InputEvent::Cancel | InputEvent::Pick(_) | InputEvent::PasteImage => {}
        }).detach();
        window.focus(&input.read(cx).focus_handle(cx));
        self.picker = Some(Picker { kind, input, selected: 0, pane: self.focused.clone(), note: None, target, query: text, fixes: vec![] });
        cx.notify();
    }

    pub fn close_picker(&mut self, cx: &mut Context<Self>) {
        self.picker = None;
        self.want_term_focus = true;
        cx.notify();
    }

    fn picker_rows(&self, cx: &gpui::App) -> Vec<(Row, String, String, Vec<usize>)> {
        let Some(p) = &self.picker else { return vec![] };
        let q = fold(p.input.read(cx).value().trim());
        let has = |hay: String| q.is_empty() || fold(&hay).contains(&q);
        match p.kind {
            Kind::Jump | Kind::Peek => self.store.panes.iter().filter(|x| {
                let space = self.store.spaces.iter().find(|s| s.id == x.space_id).map(|s| s.name.as_str()).unwrap_or("");
                has(format!("{} {} {} {}", x.name, x.title, space, x.cwd))
            }).map(|x| {
                let space = self.store.spaces.iter().find(|s| s.id == x.space_id).map(|s| s.name.clone()).unwrap_or_default();
                (Row::Pane(x.id.clone()), x.name.clone(), format!("{space} · {}", crate::widgets::short_path(&x.cwd)), vec![])
            }).collect(),
            Kind::Commands => {
                let plugins = self.plugin_actions();
                let all: Vec<(Row, String, String)> = COMMANDS.iter().map(|(l, k, c)| (Row::Cmd(*c), l.to_string(), k.to_string()))
                    .chain(plugins.into_iter().map(|(id, l)| (Row::Plugin(id), l, "plugin".to_string()))).collect();
                let labels: Vec<&str> = all.iter().map(|r| r.1.as_str()).collect();
                let query = p.input.read(cx).value();
                let hits = search(&labels, &query);
                let mut rows: Vec<(Row, String, String, Vec<usize>)> = Vec::new();
                let mut taken = vec![false; all.len()];
                if query.trim().is_empty() {
                    // Nothing typed: what suits this screen first, then everything else.
                    for want in suggested(self.screen()) {
                        if let Some(i) = all.iter().position(|(r, _, _)| matches!(r, Row::Cmd(c) if c == want)) {
                            taken[i] = true;
                            let (_, label, keys) = &all[i];
                            rows.push((Row::Cmd(*want), label.clone(), if keys.is_empty() { "suggested".into() } else { format!("suggested · {keys}") }, vec![]));
                        }
                    }
                }
                let mut all: Vec<Option<(Row, String, String)>> = all.into_iter().map(Some).collect();
                for h in hits {
                    if taken[h.index] { continue; }
                    if let Some((row, label, keys)) = all[h.index].take() { rows.push((row, label, keys, h.marks)); }
                }
                rows
            }
            Kind::Quickfix => p.fixes.iter().filter(|f| has(format!("{} {} {}", f.path, f.step, f.text))).map(|f| {
                let at = f.line.map(|l| format!("{}:{l}", f.path)).unwrap_or_else(|| f.path.clone());
                (Row::Fix(f.clone()), at, format!("{} · {}", f.step, f.text.chars().take(60).collect::<String>()), vec![])
            }).collect(),
            Kind::Rename | Kind::RenameSpace | Kind::RenameTab | Kind::RenameMachine | Kind::RenameChat | Kind::InstallPlugin | Kind::NewWorktree | Kind::Find => vec![],
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
            Kind::RenameChat => {
                let name = self.picker.as_ref().map(|p| p.input.read(cx).text.trim().to_string()).unwrap_or_default();
                if let (Some(id), false) = (self.picker.as_ref().and_then(|p| p.target.clone()), name.is_empty()) {
                    self.rpc("chat.rename", json!({"chat": id, "name": name}));
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
                    Some(Row::Pane(id)) if kind == Kind::Peek => self.open_peek(&id, window, cx),
                    Some(Row::Pane(id)) => { self.show_pane(&id); window.focus(&self.term_focus); }
                    Some(Row::Cmd(c)) => self.run_cmd(c, window, cx),
                    Some(Row::Plugin(id)) => self.run_plugin_action(&id),
                    Some(Row::Fix(f)) => self.open_in_files(&f.path, f.line, f.col, cx),
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
            Cmd::Shortcuts => self.open_shortcuts(window, cx),
            Cmd::Jump => self.open_picker(Kind::Jump, window, cx),
            Cmd::Peek => self.open_picker(Kind::Peek, window, cx),
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
            Cmd::WideEditor => { self.docs.wide = !self.docs.wide; if self.panel() != Some(View::Files) { self.toggle_panel(View::Files, cx) } }
            Cmd::SendSelection => self.files_send_selection(cx),
            Cmd::Quickfix => self.open_quickfix(window, cx),
            Cmd::Broadcast => self.open_broadcast(window, cx),
            Cmd::OpenFolder => self.open_folder_picker(window, cx),
            Cmd::Chats => self.open_chat_view(window, cx),
            Cmd::NewChat => self.new_chat(window, cx),
        }
        cx.notify();
    }
}

/// The label with the letters that matched what was typed in the accent colour and bold.
fn highlighted(label: String, marks: &[usize]) -> gpui::AnyElement {
    if marks.is_empty() { return label.into_any_element(); }
    let style = gpui::HighlightStyle { color: Some(rgb(accent()).into()), font_weight: Some(FontWeight::BOLD), ..Default::default() };
    let ranges = mark_ranges(&label, marks).into_iter().map(|r| (r, style)).collect::<Vec<_>>();
    gpui::StyledText::new(label).with_highlights(ranges).into_any_element()
}

/// Commands worth offering first on each screen (only ones that exist as palette commands).
fn suggested(screen: View) -> &'static [Cmd] {
    match screen {
        View::Terminals => &[Cmd::NewTab, Cmd::SplitRight, Cmd::Find, Cmd::NextBlocked, Cmd::Files],
        View::Files => &[Cmd::SendSelection, Cmd::WideEditor, Cmd::Quickfix, Cmd::Changes, Cmd::Jump],
        View::Changes => &[Cmd::Quickfix, Cmd::Files, Cmd::NextBlocked, Cmd::NextDone],
        View::Inbox => &[Cmd::NextBlocked, Cmd::Files, Cmd::Changes],
        _ => &[],
    }
}

impl DroverApp {
    /// What the user is looking at: the open side panel (Files, Changes, Agents) of the focused agent, else the main view.
    pub fn screen(&self) -> View {
        if self.view == View::Terminals { self.panel().unwrap_or(View::Terminals) } else { self.view }
    }
}

pub fn overlay(app: &DroverApp, cx: &mut Context<DroverApp>) -> Option<Div> {
    let p = app.picker.as_ref()?;
    let title = match p.kind { Kind::Jump => "Jump to", Kind::Peek => "Peek at",Kind::Commands => "Commands", Kind::Rename => "Rename agent", Kind::RenameSpace => "Rename space", Kind::RenameTab => "Rename tab", Kind::RenameMachine => "Rename machine", Kind::RenameChat => "Rename chat", Kind::InstallPlugin => "Install a plugin", Kind::NewWorktree => "New worktree", Kind::Find => "Find in terminal", Kind::Quickfix => "Check errors" };
    let mut list = div().flex().flex_col().gap(px(1.)).max_h(px(420.)).overflow_hidden();
    // Keep the picked row on screen: 14 rows are drawn, from where the selection needs them.
    let first = p.selected.saturating_sub(13);
    for (i, (row, label, detail, marks)) in app.picker_rows(cx).into_iter().enumerate().skip(first).take(14) {
        let on = i == p.selected;
        let lead = match &row {
            Row::Pane(id) => app.store.pane(id).map(|x| div().flex().items_center().gap(px(6.)).child(kind_icon(&x.kind))
                .child(state_mark(&x.state, x.exit_code, gpui::SharedString::from(format!("pk-{id}"))))),
            Row::Cmd(_) | Row::Plugin(_) | Row::Fix(_) => None,
        };
        list = list.child(div().h(px(32.)).flex_none().flex().items_center().gap(px(10.)).px(px(10.)).rounded(px(7.))
            .when(on, |d| d.bg(theme::selected())).hover(|d| d.bg(theme::hover()))
            .children(lead)
            .child(div().flex_1().min_w_0().truncate().text_color(rgb(fg())).child(highlighted(label, &marks)))
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
