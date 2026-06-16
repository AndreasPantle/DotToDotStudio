use crate::app::DotToDotStudioApp;
use crate::editor::SequenceItem;
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
// we must NOT call dirty/renumber/remove logic that requires another mutable
// borrow of the editor state inside that same borrow scope.
pub fn show_sequences_panel(ctx: &egui::Context, app: &mut DotToDotStudioApp) {
    egui::SidePanel::right("sequences_panel").resizable(true).default_size(300.0).show(ctx, |ui| {
        ui.heading(egui::RichText::new("Sequences").strong());
        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("Add").clicked() {
                let next_index = app.editor.sequences.len() + 1;

                app.editor.push_undo_snapshot();

                app.editor.sequences.push(SequenceItem {
                    name: format!("Sequence {next_index}"),
                    visible: true,
                    color: egui::Color32::from_rgb(120, 180, 255),
                    line_thickness: 3.0,
                    start_value: 1,
                    points: Vec::new(),
                });

                app.editor.selected_sequence = Some(app.editor.sequences.len() - 1);
                app.editor.selected_point = None;

                app.editor.mark_dirty();
                app.status_message = "Sequence added".to_string();
            }

            let can_remove =
                app.editor.selected_sequence.is_some() && !app.editor.sequences.is_empty();

            if ui.add_enabled(can_remove, egui::Button::new("Remove")).clicked()
                && let Some(index) = app.editor.selected_sequence
                && index < app.editor.sequences.len()
            {
                app.editor.push_undo_snapshot();

                app.editor.sequences.remove(index);

                if app.editor.sequences.is_empty() {
                    app.editor.selected_sequence = None;
                    app.editor.selected_point = None;
                } else if index >= app.editor.sequences.len() {
                    app.editor.selected_sequence = Some(app.editor.sequences.len() - 1);
                    app.editor.selected_point = None;
                } else {
                    app.editor.selected_sequence = Some(index);
                    app.editor.selected_point = None;
                }

                app.editor.mark_dirty();
                app.status_message = "Sequence removed".to_string();
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.label(egui::RichText::new("Sequence List").strong());

        for (index, sequence) in app.editor.sequences.iter().enumerate() {
            let is_selected = app.editor.selected_sequence == Some(index);

            let visibility_marker = if sequence.visible { "●" } else { "○" };
            let label =
                format!("{} {} ({})", visibility_marker, sequence.name, sequence.points.len());

            if ui.selectable_label(is_selected, label).clicked() {
                app.editor.selected_sequence = Some(index);
                app.editor.selected_point = None;
                app.status_message = format!("Selected: {}", sequence.name);
            }
        }

        ui.add_space(12.0);
        ui.separator();
        ui.heading(egui::RichText::new("Selected Sequence").strong());
        ui.separator();

        if let Some(sequence_index) = app.editor.selected_sequence {
            let mut sequence_name_changed = false;
            let mut sequence_visible_changed = false;
            let mut sequence_color_changed = false;
            let mut sequence_line_thickness_changed = false;

            let mut start_value_changed = false;
            let mut point_value_changed = false;
            let mut remove_selected_point = false;
            let mut renumber_from_start = false;
            let mut renumber_from_selected = false;

            if let Some(sequence) = app.editor.sequences.get_mut(sequence_index) {
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
                        app.editor.selected_point.is_some() && !sequence.points.is_empty();

                    if ui.add_enabled(can_remove_point, egui::Button::new("Remove Point")).clicked()
                    {
                        remove_selected_point = true;
                    }
                });

                ui.add_space(10.0);
                ui.separator();
                ui.label(egui::RichText::new("Points").strong());

                egui::ScrollArea::vertical().max_height(220.0).auto_shrink([false, false]).show(
                    ui,
                    |ui| {
                        let mut did_scroll_to_selected_point = false;

                        for (point_index, point) in sequence.points.iter_mut().enumerate() {
                            let is_selected = app.editor.selected_point == Some(point_index);

                            let label = format!(
                                "{}: ({:.1}, {:.1})",
                                point.value, point.position.x, point.position.y
                            );

                            let response = ui.selectable_label(is_selected, label);

                            if response.clicked() {
                                app.editor.selected_point = Some(point_index);
                                app.editor.scroll_selected_point_into_view = true;
                                app.status_message = format!("Selected point {}", point.value);
                            }

                            let is_selected_now = app.editor.selected_point == Some(point_index);

                            if is_selected_now
                                && app.editor.scroll_selected_point_into_view
                                && !did_scroll_to_selected_point
                            {
                                response.scroll_to_me(Some(egui::Align::Center));
                                did_scroll_to_selected_point = true;
                            }
                        }

                        if did_scroll_to_selected_point {
                            app.editor.scroll_selected_point_into_view = false;
                        }
                    },
                );

                ui.add_space(8.0);
                ui.separator();
                ui.label(egui::RichText::new("Selected Point").strong());

                if let Some(point_index) = app.editor.selected_point {
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

            if sequence_name_changed
                || sequence_visible_changed
                || sequence_color_changed
                || sequence_line_thickness_changed
            {
                app.editor.mark_dirty();
            }

            if start_value_changed {
                match app.editor.renumber_selected_sequence_from_start() {
                    Ok((sequence_name, start_value)) => {
                        app.status_message =
                            format!("Renumbered sequence {} from {}", sequence_name, start_value);
                    }
                    Err(message) => {
                        app.status_message = message.to_string();
                    }
                }
            }

            if point_value_changed {
                match app.editor.renumber_selected_point_and_following() {
                    Ok((sequence_name, point_index)) => {
                        app.status_message = format!(
                            "Renumbered points after point {} in {}",
                            point_index + 1,
                            sequence_name
                        );
                    }
                    Err(message) => {
                        app.status_message = message.to_string();
                    }
                }
            }

            if renumber_from_start {
                match app.editor.renumber_selected_sequence_from_start() {
                    Ok((sequence_name, start_value)) => {
                        app.status_message =
                            format!("Renumbered sequence {} from {}", sequence_name, start_value);
                    }
                    Err(message) => {
                        app.status_message = message.to_string();
                    }
                }
            }

            if renumber_from_selected {
                match app.editor.renumber_selected_point_and_following() {
                    Ok((sequence_name, point_index)) => {
                        app.status_message = format!(
                            "Renumbered points after point {} in {}",
                            point_index + 1,
                            sequence_name
                        );
                    }
                    Err(message) => {
                        app.status_message = message.to_string();
                    }
                }
            }

            if remove_selected_point {
                match app.editor.remove_selected_point() {
                    Ok(sequence_name) => {
                        app.status_message = format!("Removed point from {}", sequence_name);
                    }
                    Err(message) => {
                        app.status_message = message.to_string();
                    }
                }
            }
        } else {
            ui.label("No sequence selected.");
        }
    });
}
