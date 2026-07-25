use crate::app::{DotToDotStudioApp, ExportScope};
use eframe::egui;

// Draw the export dialog.
//
// This dialog is the future entry point for all export functionality.
// For now it provides CSV export and a small set of options that can be
// expanded later without changing the surrounding app structure.
pub fn show_export_dialog(ctx: &egui::Context, app: &mut DotToDotStudioApp) {
    if !app.export_dialog.open {
        return;
    }

    let mut open = app.export_dialog.open;
    let mut close_after_export = false;
    let mut close_after_cancel = false;

    egui::Window::new("Export")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(420.0)
        .show(ctx, |ui| {
            ui.heading(egui::RichText::new("Export Options").strong());
            ui.separator();

            ui.label("Choose what should be exported:");

            ui.add_space(8.0);

            ui.checkbox(&mut app.export_dialog.export_points_as_csv, "Export points as CSV");

            ui.checkbox(
                &mut app.export_dialog.export_image_without_overlay,
                "Export image without overlay",
            );

            ui.checkbox(
                &mut app.export_dialog.export_image_with_overlay,
                "Export image with overlay",
            );

            ui.checkbox(
                &mut app.export_dialog.export_overlay_only,
                "Export overlay only (transparent PNG)",
            );

            ui.add_space(4.0);
            ui.label(egui::RichText::new("Vector (SVG)").small().weak());

            ui.checkbox(
                &mut app.export_dialog.export_overlay_as_svg,
                "Export overlay only (SVG, transparent, scalable)",
            );

            ui.checkbox(
                &mut app.export_dialog.export_image_with_overlay_as_svg,
                "Export image with overlay (SVG, embedded image)",
            );

            ui.checkbox(
                &mut app.export_dialog.include_points_in_overlay,
                "Include points in overlay exports",
            );

            ui.add_space(12.0);
            ui.separator();
            ui.label(egui::RichText::new("Scope").strong());

            ui.radio_value(
                &mut app.export_dialog.scope,
                ExportScope::SelectedSequence,
                "Selected sequence only",
            );

            ui.radio_value(
                &mut app.export_dialog.scope,
                ExportScope::AllSequences,
                "All sequences",
            );

            ui.add_space(12.0);
            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("Export").clicked() {
                    let mut did_export_anything = false;

                    if app.export_dialog.export_points_as_csv {
                        app.export_points_as_csv();
                        did_export_anything = true;
                    }

                    if app.export_dialog.export_image_without_overlay {
                        app.export_image_without_overlay();
                        did_export_anything = true;
                    }

                    if app.export_dialog.export_image_with_overlay {
                        app.export_image_with_overlay();
                        did_export_anything = true;
                    }

                    if app.export_dialog.export_overlay_only {
                        app.export_overlay_only();
                        did_export_anything = true;
                    }

                    if app.export_dialog.export_overlay_as_svg {
                        app.export_overlay_as_svg();
                        did_export_anything = true;
                    }

                    if app.export_dialog.export_image_with_overlay_as_svg {
                        app.export_image_with_overlay_as_svg();
                        did_export_anything = true;
                    }

                    if !did_export_anything {
                        app.status_message = "No export option selected".to_string();
                    }

                    close_after_export = true;
                }

                if ui.button("Cancel").clicked() {
                    close_after_cancel = true;
                }
            });
        });

    if close_after_export || close_after_cancel {
        open = false;
    }

    app.export_dialog.open = open;
}
