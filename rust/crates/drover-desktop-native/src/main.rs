//! drover native desktop app (GPUI). One more client of drover-server, like the web UI and `dr tui`.

mod actions;
mod assets;
mod dialogs;
mod drag;
mod find;
mod font_fallback;
mod font_prefs;
mod keys;
mod layout_view;
mod picker;
mod powerline;
mod sidebar;
mod terminal_input;
mod terminal_mouse;
mod terminal_paint;
mod terminal_selection;
mod theme;
mod toasts;
mod ui_scale;
mod topbar;
mod views;
mod widgets;
mod workspace_app;

use gpui::{App, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, prelude::*, px, size};

use workspace_app::DroverApp;

fn main() {
    ui_scale::init();
    Application::new().with_assets(assets::Assets).run(|cx: &mut App| {
        assets::load_fonts(cx);
        let bounds = Bounds::centered(None, size(px(1500.), px(900.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions { title: Some("Drover".into()), ..Default::default() }),
                // WM_CLASS (X11) / app_id (Wayland) = the desktop entry's name, so the dock shows Drover's icon.
                app_id: Some("drover".into()),
                ..Default::default()
            },
            |_, cx| cx.new(DroverApp::new),
        )
        .expect("open window");
        cx.activate(true);
    });
}
