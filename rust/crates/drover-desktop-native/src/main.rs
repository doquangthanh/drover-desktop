//! drover native desktop app (GPUI). One more client of drover-server, like the web UI and `dr tui`.

mod actions;
mod assets;
mod context_menu;
mod copy_mode;
mod dialogs;
mod drag;
mod find;
mod font_fallback;
mod font_prefs;
mod keymap;
mod ssh_hosts;
mod ui_config;
mod plugin_actions;
mod plugins;
mod kitty_command;
mod kitty_graphics;
mod machines_edit;
mod new_pane_menu;
mod keys;
mod layout_view;
mod peek;
mod picker;
mod powerline;
mod sidebar;
mod sidebar_fold;
mod space_tree;
mod scrollbar;
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
        // 1500x900, but never taller or wider than the screen allows: on a 1536x864 screen (1080p at 125 %) a
        // centered 900 px window starts above the top edge and its title bar, with close/minimize/maximize, is lost.
        let screen = cx.primary_display().map(|d| d.bounds().size).unwrap_or(size(px(1500.), px(900.)));
        let bounds = Bounds::centered(None, size(px(1500.).min(screen.width * 0.95), px(900.).min(screen.height * 0.85)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                // Windows: no white system title bar; the tab bar carries minimize / maximize / close (topbar.rs).
                titlebar: Some(TitlebarOptions { title: Some("Drover".into()), appears_transparent: topbar::CUSTOM_TITLEBAR, ..Default::default() }),
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
