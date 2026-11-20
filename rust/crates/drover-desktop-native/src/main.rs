//! drover native desktop app (GPUI). One more client of drover-server, like the web UI and `dr tui`.

mod actions;
mod assets;
mod dialogs;
mod layout_view;
mod sidebar;
mod terminal_input;
mod terminal_paint;
mod terminal_selection;
mod theme;
mod topbar;
mod views;
mod widgets;
mod workspace_app;

use gpui::{App, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, prelude::*, px, size};

use workspace_app::DroverApp;

fn main() {
    Application::new().with_assets(assets::Assets).run(|cx: &mut App| {
        assets::load_fonts(cx);
        let bounds = Bounds::centered(None, size(px(1500.), px(900.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions { title: Some("Drover".into()), ..Default::default() }),
                ..Default::default()
            },
            |_, cx| cx.new(DroverApp::new),
        )
        .expect("open window");
        cx.activate(true);
    });
}
