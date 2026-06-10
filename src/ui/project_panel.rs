use crate::app::DotToDotStudioApp;
use eframe::egui;

// Draw the left side panel for project and image metadata.
//
// This panel contains editable project-level fields like:
// - project name
// - origin URL
// - comment
//
// It also shows read-only image metadata such as:
// - file name
// - image dimensions
// - file size
pub fn show_project_panel(ctx: &egui::Context, app: &mut DotToDotStudioApp) {
    egui::SidePanel::left("project_panel").resizable(true).default_size(260.0).show(ctx, |ui| {
        ui.heading(egui::RichText::new("Project").strong());
        ui.separator();

        ui.label("Name");
        if ui.text_edit_singleline(&mut app.project_name).changed() {
            app.mark_dirty();
        }

        ui.add_space(8.0);

        ui.label("Origin URL");
        if ui.text_edit_singleline(&mut app.origin_url).changed() {
            app.mark_dirty();
        }

        if !app.origin_url.trim().is_empty() {
            ui.add_space(4.0);

            ui.hyperlink_to("Open Origin URL", app.origin_url.trim());
        }

        ui.add_space(8.0);

        ui.label("Comment / Description");
        if ui
            .add(
                egui::TextEdit::multiline(&mut app.comment)
                    .desired_rows(6)
                    .desired_width(f32::INFINITY),
            )
            .changed()
        {
            app.mark_dirty();
        }

        ui.add_space(12.0);
        ui.separator();
        ui.heading(egui::RichText::new("Image").strong());
        ui.separator();

        let file_name = app.image_name.as_deref().unwrap_or("No image loaded");
        ui.label(format!("File name: {file_name}"));

        if let Some([width, height]) = app.image_size {
            ui.label(format!("Dimensions: {} × {} px", width, height));
        } else {
            ui.label("Dimensions: -");
        }

        if let Some(bytes) = app.image_size_bytes {
            ui.label(format!("File size: {} bytes", bytes));
        } else {
            ui.label("File size: -");
        }
    });
}
