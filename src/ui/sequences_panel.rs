use crate::app::{DotToDotStudioApp, SequenceItem};
use eframe::egui;

// Draw the right side panel for sequence management.
//
// This panel currently supports:
// - adding sequences
// - removing the selected sequence
// - selecting a sequence from the list
// - editing the selected sequence
// - selecting and editing points of the selected sequence
//
// Important implementation detail:
// Some edits happen directly inside widgets bound to mutable sequence fields,
// for example:
// - sequence name
// - visible checkbox
// - color editor
//
// Because we borrow the selected sequence mutably while drawing those widgets,
// we must NOT call `app.mark_dirty()` immediately inside that same borrow scope.
// Instead, we collect small boolean flags first and call `app.mark_dirty()`
// afterwards, once the mutable borrow of the sequence has ended.
pub fn show_sequences_panel(ctx: &egui::Context, app: &mut DotToDotStudioApp) {
    egui::SidePanel::right("sequences_panel").resizable(true).default_size(300.0).show(ctx, |ui| {
        ui.heading(egui::RichText::new("Sequences").strong());
        ui.separator();

        // Top toolbar for sequence-level actions.
        ui.horizontal(|ui| {
            // Add a new empty sequence and select it immediately.
            if ui.button("Add").clicked() {
                let next_index = app.sequences.len() + 1;

                // Save undo state before changing the sequence list.
                app.push_undo_snapshot();

                app.sequences.push(SequenceItem {
                    name: format!("Sequence {next_index}"),
                    visible: true,
                    color: egui::Color32::from_rgb(120, 180, 255),
                    line_thickness: 3.0,
                    start_value: 1,
                    points: Vec::new(),
                });

                app.selected_sequence = Some(app.sequences.len() - 1);
                app.selected_point = None;

                // Adding a sequence changes the project state.
                app.mark_dirty();

                app.status_message = "Sequence added".to_string();
            }

            let can_remove = app.selected_sequence.is_some() && !app.sequences.is_empty();

            // Remove the currently selected sequence.
            if ui.add_enabled(can_remove, egui::Button::new("Remove")).clicked()
                && let Some(index) = app.selected_sequence
                && index < app.sequences.len()
            {
                // Save undo state before changing the sequence list.
                app.push_undo_snapshot();

                app.sequences.remove(index);

                // Keep selection valid after removal.
                if app.sequences.is_empty() {
                    app.selected_sequence = None;
                    app.selected_point = None;
                } else if index >= app.sequences.len() {
                    app.selected_sequence = Some(app.sequences.len() - 1);
                    app.selected_point = None;
                } else {
                    app.selected_sequence = Some(index);
                    app.selected_point = None;
                }

                // Removing a sequence changes the project state.
                app.mark_dirty();

                app.status_message = "Sequence removed".to_string();
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.label(egui::RichText::new("Sequence List").strong());

        // Show all sequences in a selectable list.
        //
        // The bullet indicates visibility:
        // - ● = visible
        // - ○ = hidden
        for (index, sequence) in app.sequences.iter().enumerate() {
            let is_selected = app.selected_sequence == Some(index);

            let visibility_marker = if sequence.visible { "●" } else { "○" };
            let label =
                format!("{} {} ({})", visibility_marker, sequence.name, sequence.points.len());

            if ui.selectable_label(is_selected, label).clicked() {
                app.selected_sequence = Some(index);
                app.selected_point = None;
                app.status_message = format!("Selected: {}", sequence.name);
            }
        }

        ui.add_space(12.0);
        ui.separator();
        ui.heading(egui::RichText::new("Selected Sequence").strong());
        ui.separator();

        if let Some(sequence_index) = app.selected_sequence {
            // These flags collect edits that happen directly inside widget bindings.
            //
            // We cannot call `app.mark_dirty()` while the selected sequence is mutably
            // borrowed through `app.sequences.get_mut(sequence_index)`, because that
            // would create a second mutable borrow of `app`.
            //
            // So we record what changed here and apply `app.mark_dirty()` afterwards.
            let mut sequence_name_changed = false;
            let mut sequence_visible_changed = false;
            let mut sequence_color_changed = false;
            let mut sequence_line_thickness_changed = false;

            // These actions are handled later, outside the mutable sequence borrow.
            let mut start_value_changed = false;
            let mut point_value_changed = false;
            let mut remove_selected_point = false;
            let mut renumber_from_start = false;
            let mut renumber_from_selected = false;

            if let Some(sequence) = app.sequences.get_mut(sequence_index) {
                ui.label("Name");
                if ui.text_edit_singleline(&mut sequence.name).changed() {
                    sequence_name_changed = true;
                }

                ui.add_space(8.0);

                if ui.checkbox(&mut sequence.visible, "Visible").changed() {
                    sequence_visible_changed = true;
                }

                ui.add_space(8.0);

                ui.label("Color");
                if ui.color_edit_button_srgba(&mut sequence.color).changed() {
                    sequence_color_changed = true;
                }

                ui.add_space(8.0);

                ui.label("Line Thickness");
                if ui
                    .add(
                        egui::DragValue::new(&mut sequence.line_thickness)
                            .speed(0.1)
                            .range(0.5..=20.0),
                    )
                    .changed()
                {
                    sequence_line_thickness_changed = true;
                }

                ui.add_space(8.0);

                ui.label("Start Value");
                if ui.add(egui::DragValue::new(&mut sequence.start_value).speed(1)).changed() {
                    start_value_changed = true;
                }

                ui.horizontal(|ui| {
                    if ui.button("Renumber From Start").clicked() {
                        renumber_from_start = true;
                    }

                    let can_remove_point =
                        app.selected_point.is_some() && !sequence.points.is_empty();

                    if ui.add_enabled(can_remove_point, egui::Button::new("Remove Point")).clicked()
                    {
                        remove_selected_point = true;
                    }
                });

                ui.add_space(10.0);
                ui.separator();
                ui.label(egui::RichText::new("Points").strong());

                // The point list can become quite long, so we place it inside
                // a vertical scroll area. This keeps the rest of the sequence
                // editor visible while still allowing access to all points.
                egui::ScrollArea::vertical().max_height(220.0).auto_shrink([false, false]).show(
                    ui,
                    |ui| {
                        let mut did_scroll_to_selected_point = false;

                        for (point_index, point) in sequence.points.iter_mut().enumerate() {
                            let is_selected = app.selected_point == Some(point_index);

                            let label = format!(
                                "{}: ({:.1}, {:.1})",
                                point.value, point.position.x, point.position.y
                            );

                            let response = ui.selectable_label(is_selected, label);

                            if response.clicked() {
                                app.selected_point = Some(point_index);
                                app.scroll_selected_point_into_view = true;
                                app.status_message = format!("Selected point {}", point.value);
                            }

                            // Re-check after click handling, because the click may have
                            // changed the selected point in this same frame.
                            let is_selected_now = app.selected_point == Some(point_index);

                            if is_selected_now
                                && app.scroll_selected_point_into_view
                                && !did_scroll_to_selected_point
                            {
                                response.scroll_to_me(Some(egui::Align::Center));
                                did_scroll_to_selected_point = true;
                            }
                        }

                        // Reset the one-shot request after we have scrolled to the row.
                        if did_scroll_to_selected_point {
                            app.scroll_selected_point_into_view = false;
                        }
                    },
                );
                ui.add_space(8.0);
                ui.separator();
                ui.label(egui::RichText::new("Selected Point").strong());

                if let Some(point_index) = app.selected_point {
                    if let Some(point) = sequence.points.get_mut(point_index) {
                        ui.label(format!(
                            "Position: ({:.1}, {:.1})",
                            point.position.x, point.position.y
                        ));

                        ui.label("Value");
                        if ui.add(egui::DragValue::new(&mut point.value).speed(1)).changed() {
                            point_value_changed = true;
                        }

                        if ui.button("Renumber Following Points").clicked() {
                            renumber_from_selected = true;
                        }
                    } else {
                        ui.label("No valid point selected.");
                    }
                } else {
                    ui.label("No point selected.");
                }
            } else {
                ui.label("No valid sequence selected.");
            }

            // Apply dirty state after the mutable borrow of `sequence` has ended.
            if sequence_name_changed
                || sequence_visible_changed
                || sequence_color_changed
                || sequence_line_thickness_changed
            {
                app.mark_dirty();
            }

            // Start value changes affect numbering, so we reuse the app logic that
            // renumbers the full sequence and also handles undo + status updates.
            if start_value_changed {
                app.renumber_selected_sequence_from_start();
            }

            // Changing the selected point's numeric value should also renumber
            // all following points consistently.
            if point_value_changed {
                app.renumber_selected_point_and_following();
            }

            if renumber_from_start {
                app.renumber_selected_sequence_from_start();
            }

            if renumber_from_selected {
                app.renumber_selected_point_and_following();
            }

            if remove_selected_point {
                app.remove_selected_point();
            }
        } else {
            ui.label("No sequence selected.");
        }
    });
}
