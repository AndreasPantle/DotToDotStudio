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
        let mut project_name = app.editor.project_name.clone();
        if ui.text_edit_singleline(&mut project_name).changed() {
            app.set_project_name(project_name);
        }

        ui.add_space(8.0);

        ui.label("Origin URL");
        let mut origin_url = app.editor.origin_url.clone();
        if ui.text_edit_singleline(&mut origin_url).changed() {
            app.set_origin_url(origin_url);
        }

        if !app.editor.origin_url.trim().is_empty() {
            ui.add_space(4.0);
            ui.hyperlink_to("Open Origin URL", app.editor.origin_url.trim());
        }

        ui.add_space(8.0);

        ui.label("Comment / Description");
        let mut comment = app.editor.comment.clone();
        if ui
            .add(
                egui::TextEdit::multiline(&mut comment)
                    .desired_rows(6)
                    .desired_width(f32::INFINITY),
            )
            .changed()
        {
            app.set_comment(comment);
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
