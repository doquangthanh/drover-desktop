//! The views beside the terminals (Agents, Changes, Files, Inbox, Settings, Machines), drawn in the window instead
//! of opening the web UI. Each one is ported in its own phase (plans/261120-0900-native-gpui-views-port); until
//! then its body says so and offers the web view.

pub mod agents;
pub mod changes;
pub mod feedback;
pub mod files;
pub mod inbox;
pub mod machines;
pub mod machines_add_dialog;
pub mod panel;
pub mod settings;
pub mod settings_alerts;
pub mod settings_look;
pub mod settings_rules;
pub mod settings_server;
pub mod text_input;
pub mod ui;
pub mod whats_new;

use gpui::{Context, Div, MouseButton, Window, div, prelude::*};

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
    WhatsNew,
    Feedback,
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
            View::WhatsNew => "What's new",
            View::Feedback => "Feedback",
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
            View::WhatsNew => "info",
            View::Feedback => "more",
        }
    }

    /// Drawn in this window (all of them now; a view that is not ported yet would open the web UI instead).
    pub fn is_native(self) -> bool {
        true
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
            View::WhatsNew | View::Feedback => "",
        }
    }
}

impl DroverApp {
    /// Show `view`; the terminals take the keys back when they are shown again.
    pub fn set_view(&mut self, view: View, window: &mut Window, cx: &mut Context<Self>) {
        crate::actions::trace(&format!("view: {view:?}"));
        if !view.is_native() {
            self.open_web(view.web());
            return;
        }
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
    let head = ui::header(view.icon(), view.title()).when(!view.web().is_empty(), |d| d.child(web));
    let content = match view {
        View::Agents => agents::body(app, cx),
        View::Changes => changes::panel_body(app, cx),
        View::Files => files::panel_body(app, cx),
        View::Inbox => inbox::body(app, cx),
        View::Settings => settings::body(app, cx),
        View::Machines => machines::body(app, cx),
        View::WhatsNew => whats_new::body(app, cx),
        View::Feedback => feedback::body(app, cx),
        _ => ui::note(format!("{} is not drawn here yet.", view.title()), false),
    };
    div().flex().flex_col().flex_1().min_h_0().min_w_0().child(head).child(content)
}
