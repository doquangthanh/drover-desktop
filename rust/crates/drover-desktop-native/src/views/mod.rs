//! The views beside the terminals (Agents, Changes, Files, Inbox, Settings, Machines), drawn in the window instead
//! of opening the web UI. Each one is ported in its own phase (plans/261120-0900-native-gpui-views-port); until
//! then its body says so and offers the web view.

pub mod text_input;
pub mod ui;

use gpui::{Context, Div, MouseButton, Window, div, prelude::*, px};

use crate::workspace_app::DroverApp;
use ui::Kind;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Terminals,
    Agents,
    Changes,
    Files,
    Inbox,
    Settings,
    Machines,
}

impl View {
    pub fn title(self) -> &'static str {
        match self {
            View::Terminals => "Terminals",
            View::Agents => "Agents",
            View::Changes => "Changes",
            View::Files => "Files",
            View::Inbox => "Inbox",
            View::Settings => "Settings",
            View::Machines => "Machines",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            View::Terminals => "tab",
            View::Agents => "layers",
            View::Changes => "branch",
            View::Files => "folder",
            View::Inbox => "inbox",
            View::Settings => "gear",
            View::Machines => "server",
        }
    }

    /// The web UI's name for the same view (`#tree`, `#changes`...), for "Open in browser".
    pub fn web(self) -> &'static str {
        match self {
            View::Terminals => "",
            View::Agents => "tree",
            View::Changes => "changes",
            View::Files => "files",
            View::Inbox => "inbox",
            View::Settings => "settings",
            View::Machines => "machines",
        }
    }
}

impl DroverApp {
    /// Show `view`; the terminals take the keys back when they are shown again.
    pub fn set_view(&mut self, view: View, window: &mut Window, cx: &mut Context<Self>) {
        self.view = view;
        if view == View::Terminals {
            window.focus(&self.term_focus);
        }
        cx.notify();
    }
}

/// The main area while a view is shown.
pub fn body(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let view = app.view;
    let web = div().child(ui::button("Open in browser", Kind::Plain))
        .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, _| app.open_web(view.web())));
    let head = ui::header(view.icon(), view.title()).child(web);
    let content = match view {
        View::Agents => div().flex().flex_col().gap(px(10.)).p(px(16.)).max_w(px(420.))
            .child(app.agents_filter.clone())
            .child(ui::note(format!("Typed: {}", app.agents_filter.read(cx).text), false)),
        _ => ui::note(format!("{} is not drawn here yet: it comes in a later phase.", view.title()), false),
    };
    div().flex().flex_col().flex_1().min_h_0().min_w_0().child(head).child(content)
}
