mod app;
mod editor;
mod export_logic;
mod model;
mod storage;
mod ui;

// Import the main application type from the `app` module.
// This struct contains the application state and UI logic.
use app::DotToDotStudioApp;

fn configure_egui_style(ctx: &eframe::egui::Context) {
    use eframe::egui;

    // Read the current global egui style so we can customize it.
    let mut style = (*ctx.global_style()).clone();

    // Adjust spacing to make the UI feel a bit roomier and more tool-like.
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.window_margin = egui::Margin::same(10);

    // Start from egui's dark theme and refine it.
    style.visuals = egui::Visuals::dark();

    // Round corners slightly to give the UI a softer modern/editor look.
    style.visuals.window_corner_radius = 10.0.into();
    style.visuals.menu_corner_radius = 8.0.into();
    style.visuals.widgets.noninteractive.corner_radius = 8.0.into();
    style.visuals.widgets.inactive.corner_radius = 8.0.into();
    style.visuals.widgets.hovered.corner_radius = 8.0.into();
    style.visuals.widgets.active.corner_radius = 8.0.into();

    // Set background and panel colors.
    style.visuals.panel_fill = egui::Color32::from_rgb(20, 22, 28);
    style.visuals.window_fill = egui::Color32::from_rgb(24, 26, 32);
    style.visuals.faint_bg_color = egui::Color32::from_rgb(32, 35, 42);
    style.visuals.extreme_bg_color = egui::Color32::from_rgb(12, 14, 18);

    // Set widget background colors for different interaction states.
    style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(28, 30, 36);
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(36, 40, 48);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(52, 58, 70);
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(70, 90, 130);

    // Set text/foreground colors for widgets.
    style.visuals.widgets.inactive.fg_stroke.color = egui::Color32::from_rgb(180, 190, 210);
    style.visuals.widgets.hovered.fg_stroke.color = egui::Color32::from_rgb(220, 230, 255);
    style.visuals.widgets.active.fg_stroke.color = egui::Color32::from_rgb(255, 255, 255);

    // Apply the customized style globally.
    ctx.set_global_style(style);
}

fn main() -> eframe::Result {
    // NativeOptions configures how the desktop window should be created.
    // For now we use the default settings:
    // - a native OS window
    // - default size and renderer settings
    //let options = eframe::NativeOptions::default();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_maximized(true),
        ..Default::default()
    };
    // Fullscreen
    // let options = eframe::NativeOptions {
    //     viewport: eframe::egui::ViewportBuilder::default().with_fullscreen(true),
    //     ..Default::default()
    // };

    // Start the native eframe application.
    //
    // Parameters:
    // 1. Window title
    // 2. Native window options
    // 3. A closure that constructs the app instance
    //
    // The closure receives a CreationContext (`cc`), which gives access
    // to the egui context during application startup.
    eframe::run_native(
        "Dot-To-Dot Studio",
        options,
        Box::new(|cc| {
            // Apply a custom global egui style before the app starts.
            configure_egui_style(&cc.egui_ctx);

            // Create the initial application state.
            Ok(Box::new(DotToDotStudioApp::default()))
        }),
    )
}
