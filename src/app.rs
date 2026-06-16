use crate::editor::{EditorState, SequenceItem, SequencePoint};
use crate::export_logic;
use crate::storage;
use crate::ui::{export, project_panel, sequences_panel};
use eframe::egui;
use egui::{ColorImage, Pos2, Rect, Sense, TextureHandle, Vec2};
use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};

// Export scope options for the export dialog.
//
// The export scope is still owned by the GUI-facing application shell because
// it belongs to the export dialog state rather than the reusable editor core.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ExportScope {
    SelectedSequence,
    AllSequences,
}

// UI state for the export dialog.
//
// This remains in the app adapter layer because it is tied directly to the
// current GUI implementation.
pub struct ExportDialogState {
    pub open: bool,
    pub export_points_as_csv: bool,
    pub export_image_without_overlay: bool,
    pub export_image_with_overlay: bool,
    pub export_overlay_only: bool,
    pub include_points_in_overlay: bool,
    pub scope: ExportScope,
}

// This struct stores the full UI/application state.
//
// Compared to the previous iteration, the actual editable project/editor state
// now lives inside `EditorState`. That is an important separation step:
// - `editor`: reusable functional editing state
// - remaining fields: GUI state, IO orchestration, and app shell behavior
pub struct DotToDotStudioApp {
    /// GUI-independent editor state.
    pub editor: EditorState,

    // GPU texture handle for the currently loaded image.
    // `None` means that no image has been imported yet.
    pub image_texture: Option<TextureHandle>,

    // File name of the imported image (for display in the status bar).
    pub image_name: Option<String>,

    // Original image size in pixels: [width, height].
    pub image_size: Option<[usize; 2]>,

    // Size of the imported image file in bytes.
    pub image_size_bytes: Option<usize>,

    // Raw original file bytes of the imported image.
    pub image_bytes: Option<Vec<u8>>,

    // General status text shown in the status bar.
    pub status_message: String,

    // State for the export dialog.
    pub export_dialog: ExportDialogState,

    // Close confirmation flag. When true, the app will prompt the user to confirm discarding unsaved changes.
    pub allow_close_without_prompt: bool,

    // Current zoom factor for the image viewer.
    // 1.0 means 100% size.
    pub zoom: f32,

    // Pan/translation offset inside the viewport.
    // This moves the image when the user drags the view.
    pub pan: Vec2,

    // The currently available drawing area of the central viewport.
    // This is useful for operations like "Fit Image".
    pub viewport_size: Vec2,

    // Current mouse position in view/screen coordinates.
    // This is the pointer position inside the egui UI area.
    pub mouse_view_pos: Option<Pos2>,

    // Current mouse position in image coordinates.
    // This is only set when the pointer is actually over the image.
    pub mouse_image_pos: Option<Pos2>,
}

impl Default for DotToDotStudioApp {
    fn default() -> Self {
        Self {
            editor: EditorState::default(),
            image_texture: None,
            image_name: None,
            image_size: None,
            image_size_bytes: None,
            image_bytes: None,
            status_message: "Ready".to_string(),
            export_dialog: ExportDialogState {
                open: false,
                export_points_as_csv: false,
                export_image_without_overlay: false,
                export_image_with_overlay: true,
                export_overlay_only: false,
                include_points_in_overlay: true,
                scope: ExportScope::SelectedSequence,
            },
            allow_close_without_prompt: false,
            zoom: 1.0,
            pan: Vec2::ZERO,
            viewport_size: Vec2::ZERO,
            mouse_view_pos: None,
            mouse_image_pos: None,
        }
    }
}

impl DotToDotStudioApp {
    // Open a native file dialog, load an image from disk,
    // keep the original file bytes for later database storage,
    // decode the image, and upload it as an egui texture.
    //
    // This remains in the GUI/app layer because it depends on:
    // - file dialogs
    // - egui texture creation
    // - UI-facing status reporting
    fn import_image(&mut self, ctx: &egui::Context) {
        let file =
            FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "bmp"]).pick_file();

        let Some(path) = file else {
            self.status_message = "Image import cancelled".to_string();
            return;
        };

        let file_bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) => {
                self.status_message = format!("Failed to read image file: {err}");
                return;
            }
        };

        match image::load_from_memory(&file_bytes) {
            Ok(dynamic_image) => {
                let rgba_image = dynamic_image.to_rgba8();
                let width = rgba_image.width() as usize;
                let height = rgba_image.height() as usize;
                let pixels = rgba_image.into_raw();

                let color_image = ColorImage::from_rgba_unmultiplied([width, height], &pixels);

                let texture = ctx.load_texture(
                    "imported_image",
                    color_image,
                    egui::TextureOptions::default(),
                );

                self.image_texture = Some(texture);
                self.image_name = path.file_name().map(|n| n.to_string_lossy().to_string());
                self.image_size = Some([width, height]);
                self.image_size_bytes = Some(file_bytes.len());
                self.image_bytes = Some(file_bytes);
                self.editor.mark_dirty();
                self.status_message = format!("Imported image: {}", path.display());
            }
            Err(err) => {
                self.status_message = format!("Failed to decode image: {err}");
            }
        }
    }

    // Decode raw image bytes and upload them as an egui texture.
    //
    // This is deliberately GUI-specific because the result is a GPU texture handle
    // owned by the current GUI toolkit.
    fn load_texture_from_image_bytes(
        &self,
        ctx: &egui::Context,
        image_bytes: &[u8],
    ) -> Result<TextureHandle, image::ImageError> {
        let dynamic_image = image::load_from_memory(image_bytes)?;
        let rgba_image = dynamic_image.to_rgba8();
        let width = rgba_image.width() as usize;
        let height = rgba_image.height() as usize;
        let pixels = rgba_image.into_raw();

        let color_image = ColorImage::from_rgba_unmultiplied([width, height], &pixels);

        Ok(ctx.load_texture("loaded_project_image", color_image, egui::TextureOptions::default()))
    }

    // Open a save dialog and write the current project state into a SQLite database file.
    //
    // This still lives in the app shell because it owns:
    // - file dialog orchestration
    // - UI-facing status text
    // - the combination of editor state and image asset state
    fn save_project(&mut self) {
        if self.image_bytes.is_none() {
            self.status_message = "Cannot save project without an embedded image".to_string();
            return;
        }

        let file = FileDialog::new()
            .add_filter("DotToDotStudio Project", &["db", "sqlite"])
            .set_file_name("project.sqlite")
            .save_file();

        let Some(path) = file else {
            self.status_message = "Save cancelled".to_string();
            return;
        };

        match storage::save_project_to_sqlite(&path, self) {
            Ok(()) => {
                self.editor.clear_dirty();
                self.allow_close_without_prompt = false;
                self.status_message = format!("Project saved: {}", path.display());
            }
            Err(err) => {
                self.status_message = format!("Failed to save project: {err}");
            }
        }
    }

    // Reset the application to a fresh empty project state.
    //
    // The editor core is reset through `EditorState::new_project`, while the app
    // shell resets its own GUI-specific and image-specific state.
    fn new_project(&mut self) {
        self.image_texture = None;
        self.image_name = None;
        self.image_size = None;
        self.image_size_bytes = None;
        self.image_bytes = None;

        self.editor.new_project();

        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
        self.mouse_view_pos = None;
        self.mouse_image_pos = None;

        self.allow_close_without_prompt = false;
        self.status_message = "New project created".to_string();
    }

    // Open a project file dialog, load the project from SQLite,
    // rebuild the embedded image texture, and restore the editor state.
    fn load_project(&mut self, ctx: &egui::Context) {
        let file =
            FileDialog::new().add_filter("DotToDotStudio Project", &["db", "sqlite"]).pick_file();

        let Some(path) = file else {
            self.status_message = "Open project cancelled".to_string();
            return;
        };

        match storage::load_project_from_sqlite(&path) {
            Ok(loaded) => {
                let texture = if let Some(image_bytes) = &loaded.image_bytes {
                    match self.load_texture_from_image_bytes(ctx, image_bytes) {
                        Ok(texture) => Some(texture),
                        Err(err) => {
                            self.status_message =
                                format!("Project loaded, but image decode failed: {err}");
                            None
                        }
                    }
                } else {
                    None
                };

                self.image_name = loaded.image_name;
                self.image_size = loaded.image_size;
                self.image_size_bytes = loaded.image_size_bytes;
                self.image_bytes = loaded.image_bytes;
                self.image_texture = texture;

                self.editor.project_name = loaded.project_name;
                self.editor.origin_url = loaded.origin_url;
                self.editor.comment = loaded.comment;
                self.editor.sequences = loaded
                    .sequences
                    .into_iter()
                    .map(|sequence| SequenceItem {
                        name: sequence.name,
                        visible: sequence.visible,
                        color: egui::Color32::from_rgba_unmultiplied(
                            sequence.color[0],
                            sequence.color[1],
                            sequence.color[2],
                            sequence.color[3],
                        ),
                        line_thickness: sequence.line_thickness,
                        start_value: sequence.start_value,
                        points: sequence
                            .points
                            .into_iter()
                            .map(|point| SequencePoint {
                                position: Pos2::new(point.x, point.y),
                                value: point.value,
                            })
                            .collect(),
                    })
                    .collect();

                self.editor.selected_sequence =
                    if self.editor.sequences.is_empty() { None } else { Some(0) };

                self.editor.selected_point = None;
                self.editor.dragged_point = None;
                self.editor.undo_stack.clear();
                self.editor.scroll_selected_point_into_view = false;
                self.editor.clear_dirty();

                self.allow_close_without_prompt = false;
                self.zoom = 1.0;
                self.pan = Vec2::ZERO;

                self.status_message = format!("Project loaded: {}", path.display());
            }
            Err(err) => {
                self.status_message = format!("Failed to load project: {err}");
            }
        }
    }

    // Export the embedded image bytes back to a normal image file on disk.
    //
    // This legacy menu action is now intentionally implemented through the
    // more explicit "without overlay" export path.
    fn export_image(&mut self) {
        self.export_image_without_overlay();
    }

    // Draw the top menu bar.
    fn draw_menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    let has_image = self.image_bytes.is_some();
                    let can_import_image = self.image_bytes.is_none();

                    if ui.button("New Project...\tCtrl+N").clicked() {
                        if self.confirm_discard_unsaved_changes("New Project") {
                            self.new_project();
                        } else {
                            self.status_message = "New project cancelled".to_string();
                        }
                        ui.close();
                    }

                    if ui.button("Open Project...\tCtrl+L").clicked() {
                        if self.confirm_discard_unsaved_changes("Open Project") {
                            self.load_project(ctx);
                        } else {
                            self.status_message = "Open project cancelled".to_string();
                        }
                        ui.close();
                    }

                    ui.separator();

                    if ui
                        .add_enabled(can_import_image, egui::Button::new("Import Image...\tCtrl+O"))
                        .clicked()
                    {
                        self.import_image(ctx);
                        ui.close();
                    }

                    if ui.add_enabled(has_image, egui::Button::new("Export Image...")).clicked() {
                        self.export_image();
                        ui.close();
                    }

                    ui.separator();

                    if ui.button("Export...").clicked() {
                        self.export_dialog.open = true;
                        ui.close();
                    }

                    ui.separator();

                    if ui
                        .add_enabled(has_image, egui::Button::new("Save Project...\tCtrl+S"))
                        .clicked()
                    {
                        self.save_project();
                        ui.close();
                    }

                    ui.separator();

                    if ui.button("Quit\tCtrl+Q").clicked() {
                        self.request_quit(ctx);
                        ui.close();
                    }
                });

                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo\tCtrl+Z").clicked() {
                        if self.editor.undo() {
                            self.status_message = "Undo".to_string();
                        } else {
                            self.status_message = "Nothing to undo".to_string();
                        }
                        ui.close();
                    }
                });

                ui.menu_button("View", |ui| {
                    if ui.button("Fit Image\tCtrl+0").clicked() {
                        self.fit_image();
                        ui.close();
                    }

                    if ui.button("Actual Size (100%)\tCtrl+1").clicked() {
                        self.actual_size();
                        ui.close();
                    }

                    if ui.button("Zoom In\tCtrl++").clicked() {
                        self.zoom_in();
                        ui.close();
                    }

                    if ui.button("Zoom Out\tCtrl+-").clicked() {
                        self.zoom_out();
                        ui.close();
                    }

                    if ui.button("Reset View\tCtrl+R").clicked() {
                        self.reset_view();
                        ui.close();
                    }
                });
            });
        });
    }

    // Draw the status bar at the bottom of the window.
    fn draw_status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(&self.status_message);

                if self.editor.has_unsaved_changes {
                    ui.separator();
                    ui.label(
                        egui::RichText::new("Unsaved changes")
                            .color(egui::Color32::YELLOW)
                            .strong(),
                    );
                }

                ui.separator();
                ui.label(format!("Zoom: {:.2}x", self.zoom));

                ui.separator();
                ui.label(format!("Pan: {:.1}, {:.1}", self.pan.x, self.pan.y));

                if let Some(name) = &self.image_name {
                    ui.separator();
                    ui.label(format!("Image: {}", name));
                }

                if let Some(view_pos) = self.mouse_view_pos {
                    ui.separator();
                    ui.label(format!("Mouse view: {:.1}, {:.1}", view_pos.x, view_pos.y));
                }

                if let Some(image_pos) = self.mouse_image_pos {
                    ui.separator();
                    ui.label(format!("Mouse image: {:.1}, {:.1}", image_pos.x, image_pos.y));
                }
            });
        });
    }

    // Draw the main work area in the center of the window.
    fn draw_central_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(texture) = &self.image_texture {
                let texture_id = texture.id();
                let texture_size = texture.size_vec2();

                let available_size = ui.available_size();
                self.viewport_size = available_size;

                let (response, painter) =
                    ui.allocate_painter(available_size, Sense::click_and_drag());

                let rect = response.rect;

                let image_size = texture_size * self.zoom;
                let image_top_left = rect.min + self.pan;
                let image_rect = Rect::from_min_size(image_top_left, image_size);

                self.update_mouse_position(&response, image_rect);

                if response.drag_started_by(egui::PointerButton::Primary)
                    && let Some(image_pos) = self.mouse_image_pos
                    && let Some((sequence_index, point_index)) =
                        self.editor.find_point_near_image_position(image_pos)
                {
                    self.editor.push_undo_snapshot();

                    self.editor.selected_sequence = Some(sequence_index);
                    self.editor.selected_point = Some(point_index);
                    self.editor.dragged_point = Some((sequence_index, point_index));

                    let point_value =
                        self.editor.sequences[sequence_index].points[point_index].value;

                    self.status_message = format!("Dragging point {}", point_value);
                }

                if response.clicked_by(egui::PointerButton::Primary)
                    && self.editor.dragged_point.is_none()
                {
                    if let Some(image_pos) = self.mouse_image_pos {
                        let did_select_existing = if let Some((sequence_name, point_value)) =
                            self.editor.select_point_near_image_position(image_pos)
                        {
                            self.status_message =
                                format!("Selected point {} in {}", point_value, sequence_name);
                            true
                        } else {
                            false
                        };

                        if !did_select_existing {
                            match self.editor.add_point_to_selected_sequence(image_pos) {
                                Ok((point_value, sequence_name)) => {
                                    self.status_message = format!(
                                        "Added point {} to {} at ({:.1}, {:.1})",
                                        point_value, sequence_name, image_pos.x, image_pos.y
                                    );
                                }
                                Err(message) => {
                                    self.status_message = message.to_string();
                                }
                            }
                        }
                    } else {
                        self.status_message = "Click was outside the image".to_string();
                    }
                }

                if response.hovered() {
                    let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);

                    if scroll_delta != 0.0
                        && let Some(pointer_pos) = response.hover_pos()
                    {
                        let zoom_factor = (scroll_delta * 0.0015).exp();
                        self.zoom_at_pointer(pointer_pos, rect, image_rect, zoom_factor);
                    }
                }

                if response.dragged_by(egui::PointerButton::Primary)
                    && self.editor.dragged_point.is_some()
                    && let Some(image_pos) = self.mouse_image_pos
                    && let Some(point_value) = self.editor.move_dragged_point_to(image_pos)
                {
                    self.status_message = format!(
                        "Moved point {} to ({:.1}, {:.1})",
                        point_value, image_pos.x, image_pos.y
                    );
                }

                if response.dragged() && self.editor.dragged_point.is_none() {
                    let delta = ui.input(|i| i.pointer.delta());
                    self.pan += delta;
                }

                if response.drag_stopped_by(egui::PointerButton::Primary) {
                    self.editor.dragged_point = None;
                }

                painter.image(
                    texture_id,
                    image_rect,
                    Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                    egui::Color32::WHITE,
                );

                self.draw_sequence_points(&painter, image_rect);

                painter.rect_stroke(
                    rect,
                    0.0,
                    egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
                    egui::StrokeKind::Inside,
                );
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(100.0);

                    ui.heading(
                        egui::RichText::new("Dot-To-Dot Studio")
                            .strong()
                            .size(30.0)
                            .monospace()
                            .color(egui::Color32::from_rgb(180, 220, 255)),
                    );

                    ui.label(egui::RichText::new("No image loaded.").italics());
                    ui.label("Use File -> Import Image...");
                });
            }
        });
    }

    // Currently unused helper.
    fn ui_placeholder(&mut self, _frame: &mut eframe::Frame) {}

    // Reset the current view to default values.
    fn reset_view(&mut self) {
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
        self.status_message = "View reset".to_string();
    }

    // Increase zoom by a small step.
    fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 1.05).clamp(0.05, 20.0);
        self.status_message = format!("Zoom: {:.2}x", self.zoom);
    }

    // Decrease zoom by a small step.
    fn zoom_out(&mut self) {
        self.zoom = (self.zoom * 0.95).clamp(0.05, 20.0);
        self.status_message = format!("Zoom: {:.2}x", self.zoom);
    }

    // Zoom the image around the current pointer position.
    fn zoom_at_pointer(
        &mut self,
        pointer_pos: Pos2,
        viewport_rect: Rect,
        image_rect: Rect,
        zoom_factor: f32,
    ) {
        let old_zoom = self.zoom;
        let new_zoom = (self.zoom * zoom_factor).clamp(0.05, 20.0);

        if (new_zoom - old_zoom).abs() < f32::EPSILON {
            return;
        }

        let local_before_zoom = pointer_pos - image_rect.min;
        let image_pos = Pos2::new(local_before_zoom.x / old_zoom, local_before_zoom.y / old_zoom);

        self.zoom = new_zoom;

        let new_image_top_left = Pos2::new(
            pointer_pos.x - image_pos.x * new_zoom,
            pointer_pos.y - image_pos.y * new_zoom,
        );

        self.pan = new_image_top_left - viewport_rect.min;

        self.status_message = format!("Zoom: {:.2}x", self.zoom);
    }

    // Set zoom to actual/original image size.
    fn actual_size(&mut self) {
        self.zoom = 1.0;
        self.status_message = "Zoom set to 100%".to_string();
    }

    // Compute a zoom factor so the whole image fits inside the viewport.
    fn fit_image(&mut self) {
        let Some([width, height]) = self.image_size else {
            self.status_message = "No image loaded".to_string();
            return;
        };

        if self.viewport_size.x <= 0.0 || self.viewport_size.y <= 0.0 {
            self.status_message = "Viewport size not available".to_string();
            return;
        }

        let image_width = width as f32;
        let image_height = height as f32;

        let scale_x = self.viewport_size.x / image_width;
        let scale_y = self.viewport_size.y / image_height;

        self.zoom = scale_x.min(scale_y).clamp(0.05, 20.0);
        self.pan = Vec2::ZERO;

        self.status_message = format!("Image fitted to view ({:.2}x)", self.zoom);
    }

    // Handle keyboard shortcuts globally.
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let mut import_image = false;
        let mut save_project = false;
        let mut load_project = false;
        let mut new_project = false;
        let mut quit = false;
        let mut fit_image = false;
        let mut actual_size = false;
        let mut zoom_in = false;
        let mut zoom_out = false;
        let mut reset_view = false;
        let mut undo = false;
        let mut jump_to_sequence_start = false;
        let mut jump_to_sequence_end = false;
        let mut select_previous_point = false;
        let mut select_next_point = false;

        let keyboard_captured_by_ui = ctx.egui_wants_keyboard_input();

        ctx.input(|input| {
            let command = input.modifiers.command;

            if command && input.key_pressed(egui::Key::N) {
                new_project = true;
            }

            if command && input.key_pressed(egui::Key::L) {
                load_project = true;
            }

            if command && input.key_pressed(egui::Key::O) {
                import_image = true;
            }

            if command && input.key_pressed(egui::Key::S) {
                save_project = true;
            }

            if command && input.key_pressed(egui::Key::Q) {
                quit = true;
            }

            if command && input.key_pressed(egui::Key::Num0) {
                fit_image = true;
            }

            if command && input.key_pressed(egui::Key::Num1) {
                actual_size = true;
            }

            if command
                && (input.key_pressed(egui::Key::Plus) || input.key_pressed(egui::Key::Equals))
            {
                zoom_in = true;
            }

            if command && input.key_pressed(egui::Key::Minus) {
                zoom_out = true;
            }

            if command && input.key_pressed(egui::Key::R) {
                reset_view = true;
            }

            if command && input.key_pressed(egui::Key::Z) {
                undo = true;
            }

            if !keyboard_captured_by_ui && input.key_pressed(egui::Key::Home) {
                jump_to_sequence_start = true;
            }

            if !keyboard_captured_by_ui && input.key_pressed(egui::Key::End) {
                jump_to_sequence_end = true;
            }

            if !keyboard_captured_by_ui && input.key_pressed(egui::Key::ArrowUp) {
                select_previous_point = true;
            }

            if !keyboard_captured_by_ui && input.key_pressed(egui::Key::ArrowDown) {
                select_next_point = true;
            }
        });

        if new_project {
            if self.confirm_discard_unsaved_changes("New Project") {
                self.new_project();
            } else {
                self.status_message = "New project cancelled".to_string();
            }
        }

        if load_project {
            if self.confirm_discard_unsaved_changes("Open Project") {
                self.load_project(ctx);
            } else {
                self.status_message = "Open project cancelled".to_string();
            }
        }

        if import_image {
            if self.image_bytes.is_none() {
                self.import_image(ctx);
            } else {
                self.status_message =
                    "Import Image is only available for a new empty project".to_string();
            }
        }

        if save_project {
            if self.image_bytes.is_some() {
                self.save_project();
            } else {
                self.status_message = "Cannot save project without an embedded image".to_string();
            }
        }

        if quit {
            self.request_quit(ctx);
        }

        if fit_image {
            self.fit_image();
        }

        if actual_size {
            self.actual_size();
        }

        if zoom_in {
            self.zoom_in();
        }

        if zoom_out {
            self.zoom_out();
        }

        if reset_view {
            self.reset_view();
        }

        if undo {
            if self.editor.undo() {
                self.status_message = "Undo".to_string();
            } else {
                self.status_message = "Nothing to undo".to_string();
            }
        }

        if jump_to_sequence_start {
            match self.editor.jump_to_selected_sequence_start() {
                Ok(sequence_name) => {
                    self.status_message = format!("Jumped to first point in {}", sequence_name);
                }
                Err("No sequence selected") => {
                    self.status_message = "No sequence selected".to_string();
                }
                Err("Selected sequence is invalid") => {
                    self.status_message = "Selected sequence is invalid".to_string();
                }
                Err("Sequence has no points") => {
                    if let Some(sequence_index) = self.editor.selected_sequence {
                        if let Some(sequence) = self.editor.sequences.get(sequence_index) {
                            self.status_message =
                                format!("Sequence {} has no points", sequence.name);
                        } else {
                            self.status_message = "Selected sequence is invalid".to_string();
                        }
                    } else {
                        self.status_message = "No sequence selected".to_string();
                    }
                }
                Err(_) => {
                    self.status_message = "Could not jump to first point".to_string();
                }
            }
        }

        if jump_to_sequence_end {
            match self.editor.jump_to_selected_sequence_end() {
                Ok(sequence_name) => {
                    self.status_message = format!("Jumped to last point in {}", sequence_name);
                }
                Err("No sequence selected") => {
                    self.status_message = "No sequence selected".to_string();
                }
                Err("Selected sequence is invalid") => {
                    self.status_message = "Selected sequence is invalid".to_string();
                }
                Err("Sequence has no points") => {
                    if let Some(sequence_index) = self.editor.selected_sequence {
                        if let Some(sequence) = self.editor.sequences.get(sequence_index) {
                            self.status_message =
                                format!("Sequence {} has no points", sequence.name);
                        } else {
                            self.status_message = "Selected sequence is invalid".to_string();
                        }
                    } else {
                        self.status_message = "No sequence selected".to_string();
                    }
                }
                Err(_) => {
                    self.status_message = "Could not jump to last point".to_string();
                }
            }
        }

        if select_previous_point {
            match self.editor.select_previous_point_in_sequence() {
                Ok((point_value, sequence_name)) => {
                    self.status_message =
                        format!("Selected point {} in {}", point_value, sequence_name);
                }
                Err("No sequence selected") => {
                    self.status_message = "No sequence selected".to_string();
                }
                Err("Selected sequence is invalid") => {
                    self.status_message = "Selected sequence is invalid".to_string();
                }
                Err("Sequence has no points") => {
                    if let Some(sequence_index) = self.editor.selected_sequence {
                        if let Some(sequence) = self.editor.sequences.get(sequence_index) {
                            self.status_message =
                                format!("Sequence {} has no points", sequence.name);
                        } else {
                            self.status_message = "Selected sequence is invalid".to_string();
                        }
                    } else {
                        self.status_message = "No sequence selected".to_string();
                    }
                }
                Err(_) => {
                    self.status_message = "Could not select previous point".to_string();
                }
            }
        }

        if select_next_point {
            match self.editor.select_next_point_in_sequence() {
                Ok((point_value, sequence_name)) => {
                    self.status_message =
                        format!("Selected point {} in {}", point_value, sequence_name);
                }
                Err("No sequence selected") => {
                    self.status_message = "No sequence selected".to_string();
                }
                Err("Selected sequence is invalid") => {
                    self.status_message = "Selected sequence is invalid".to_string();
                }
                Err("Sequence has no points") => {
                    if let Some(sequence_index) = self.editor.selected_sequence {
                        if let Some(sequence) = self.editor.sequences.get(sequence_index) {
                            self.status_message =
                                format!("Sequence {} has no points", sequence.name);
                        } else {
                            self.status_message = "Selected sequence is invalid".to_string();
                        }
                    } else {
                        self.status_message = "No sequence selected".to_string();
                    }
                }
                Err(_) => {
                    self.status_message = "Could not select next point".to_string();
                }
            }
        }
    }

    // Convert the current mouse position into image coordinates.
    fn update_mouse_position(&mut self, response: &egui::Response, image_rect: Rect) {
        let Some(pointer_pos) = response.hover_pos() else {
            self.mouse_view_pos = None;
            self.mouse_image_pos = None;
            return;
        };

        self.mouse_view_pos = Some(pointer_pos);

        if image_rect.contains(pointer_pos) {
            let local = pointer_pos - image_rect.min;
            let image_x = local.x / self.zoom;
            let image_y = local.y / self.zoom;

            self.mouse_image_pos = Some(Pos2::new(image_x, image_y));
        } else {
            self.mouse_image_pos = None;
        }
    }

    // Draw all visible sequences on top of the image.
    fn draw_sequence_points(&self, painter: &egui::Painter, image_rect: Rect) {
        for (sequence_index, sequence) in self.editor.sequences.iter().enumerate() {
            if !sequence.visible {
                continue;
            }

            let screen_points: Vec<Pos2> = sequence
                .points
                .iter()
                .map(|point| {
                    Pos2::new(
                        image_rect.min.x + point.position.x * self.zoom,
                        image_rect.min.y + point.position.y * self.zoom,
                    )
                })
                .collect();

            for window in screen_points.windows(2) {
                let start = window[0];
                let end = window[1];

                painter.line_segment(
                    [start, end],
                    egui::Stroke::new(sequence.line_thickness, sequence.color),
                );
            }

            for (point_index, point) in sequence.points.iter().enumerate() {
                let screen_pos = screen_points[point_index];

                let is_selected = self.editor.selected_sequence == Some(sequence_index)
                    && self.editor.selected_point == Some(point_index);

                let point_radius = if is_selected { 9.0 } else { 6.0 };
                let stroke_color =
                    if is_selected { egui::Color32::YELLOW } else { egui::Color32::BLACK };

                painter.circle_filled(screen_pos, point_radius, sequence.color);
                painter.circle_stroke(
                    screen_pos,
                    point_radius,
                    egui::Stroke::new(2.0, stroke_color),
                );

                painter.text(
                    screen_pos + Vec2::new(8.0, -8.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{}", point.value),
                    egui::FontId::proportional(16.0),
                    sequence.color,
                );
            }
        }
    }

    // Ask the user whether they want to discard unsaved changes.
    fn confirm_discard_unsaved_changes(&self, action_name: &str) -> bool {
        if !self.editor.has_unsaved_changes {
            return true;
        }

        let result = MessageDialog::new()
            .set_level(MessageLevel::Warning)
            .set_title("Unsaved changes")
            .set_description(format!(
                "You have unsaved changes.\n\nDo you really want to continue with \"{}\" and discard them?",
                action_name
            ))
            .set_buttons(MessageButtons::YesNo)
            .show();

        result == MessageDialogResult::Yes
    }

    // Close the application, asking for confirmation first if there are unsaved changes.
    fn request_quit(&mut self, ctx: &egui::Context) {
        if self.confirm_discard_unsaved_changes("Quit") {
            self.allow_close_without_prompt = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            self.allow_close_without_prompt = false;
            self.status_message = "Quit cancelled".to_string();
        }
    }

    // Handle an external window close request.
    fn handle_close_request(&mut self, ctx: &egui::Context) {
        if self.confirm_discard_unsaved_changes("Close Window") {
            self.allow_close_without_prompt = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            self.allow_close_without_prompt = false;
            self.status_message = "Window close cancelled".to_string();
        }
    }

    // Return the sequence indices that should be exported for the current export scope.
    fn export_sequence_indices(&self) -> Vec<usize> {
        match self.export_dialog.scope {
            ExportScope::SelectedSequence => self.editor.selected_sequence.into_iter().collect(),
            ExportScope::AllSequences => (0..self.editor.sequences.len()).collect(),
        }
    }

    // Export points as CSV according to the currently selected export scope.
    pub fn export_points_as_csv(&mut self) {
        let (default_file_name, csv_content) = match self.export_dialog.scope {
            ExportScope::SelectedSequence => {
                let Some(sequence_index) = self.editor.selected_sequence else {
                    self.status_message = "No sequence selected for CSV export".to_string();
                    return;
                };

                let Some(sequence) = self.editor.sequences.get(sequence_index) else {
                    self.status_message = "Selected sequence is invalid".to_string();
                    return;
                };

                if sequence.points.is_empty() {
                    self.status_message =
                        format!("Sequence {} has no points to export", sequence.name);
                    return;
                }

                let file_name = format!("{}.csv", export_logic::sanitize_file_name(&sequence.name));

                let csv = export_logic::build_csv_for_selected_sequence(sequence_index, sequence);

                (file_name, csv)
            }
            ExportScope::AllSequences => {
                if self.editor.sequences.is_empty() {
                    self.status_message = "There are no sequences to export".to_string();
                    return;
                }

                let has_any_points =
                    self.editor.sequences.iter().any(|sequence| !sequence.points.is_empty());

                if !has_any_points {
                    self.status_message = "There are no points to export".to_string();
                    return;
                }

                let csv = export_logic::build_csv_for_all_sequences(&self.editor.sequences);
                ("all_sequences.csv".to_string(), csv)
            }
        };

        let file = FileDialog::new()
            .add_filter("CSV", &["csv"])
            .set_file_name(&default_file_name)
            .save_file();

        let Some(path) = file else {
            self.status_message = "CSV export cancelled".to_string();
            return;
        };

        match std::fs::write(&path, csv_content) {
            Ok(()) => {
                self.status_message = format!("CSV exported: {}", path.display());
            }
            Err(err) => {
                self.status_message = format!("Failed to export CSV: {err}");
            }
        }
    }

    // Export the embedded original image without any overlay.
    pub fn export_image_without_overlay(&mut self) {
        let Some(image_bytes) = &self.image_bytes else {
            self.status_message = "No embedded image available for export".to_string();
            return;
        };

        let default_file_name =
            self.image_name.clone().unwrap_or_else(|| "exported_image.png".to_string());

        let file = FileDialog::new().set_file_name(&default_file_name).save_file();

        let Some(path) = file else {
            self.status_message = "Image export cancelled".to_string();
            return;
        };

        match std::fs::write(&path, image_bytes) {
            Ok(()) => {
                self.status_message = format!("Exported image without overlay: {}", path.display());
            }
            Err(err) => {
                self.status_message = format!("Failed to export image: {err}");
            }
        }
    }

    // Export the embedded image with visible sequence overlays.
    pub fn export_image_with_overlay(&mut self) {
        let Some(image_bytes) = &self.image_bytes else {
            self.status_message = "No embedded image available for overlay export".to_string();
            return;
        };

        let base_image = match image::load_from_memory(image_bytes) {
            Ok(dynamic_image) => dynamic_image.to_rgba8(),
            Err(err) => {
                self.status_message = format!("Failed to decode image for overlay export: {err}");
                return;
            }
        };

        let sequence_indices = self.export_sequence_indices();

        if sequence_indices.is_empty() {
            self.status_message = "No sequence selected for overlay export".to_string();
            return;
        }

        if !export_logic::has_visible_overlay_content(
            &self.editor.sequences,
            &sequence_indices,
            self.export_dialog.include_points_in_overlay,
        ) {
            self.status_message = "Nothing to export for the selected overlay scope".to_string();
            return;
        }

        let rgba_image = export_logic::render_image_with_overlay(
            &base_image,
            &self.editor.sequences,
            &sequence_indices,
            self.export_dialog.include_points_in_overlay,
        );

        let base_name = self
            .image_name
            .as_deref()
            .map(export_logic::sanitize_file_name)
            .unwrap_or_else(|| "exported_image".to_string());

        let default_file_name = format!("{base_name}_overlay.png");

        let file = FileDialog::new()
            .add_filter("PNG", &["png"])
            .set_file_name(&default_file_name)
            .save_file();

        let Some(path) = file else {
            self.status_message = "Overlay image export cancelled".to_string();
            return;
        };

        match rgba_image.save(&path) {
            Ok(()) => {
                self.status_message = format!("Exported image with overlay: {}", path.display());
            }
            Err(err) => {
                self.status_message = format!("Failed to save overlay image: {err}");
            }
        }
    }

    // Export only the overlay as a transparent PNG.
    pub fn export_overlay_only(&mut self) {
        let Some([width, height]) = self.image_size else {
            self.status_message = "No image size available for overlay export".to_string();
            return;
        };

        let sequence_indices = self.export_sequence_indices();

        if sequence_indices.is_empty() {
            self.status_message = "No sequence selected for overlay export".to_string();
            return;
        }

        if !export_logic::has_visible_overlay_content(
            &self.editor.sequences,
            &sequence_indices,
            self.export_dialog.include_points_in_overlay,
        ) {
            self.status_message = "Nothing to export for the selected overlay scope".to_string();
            return;
        }

        let rgba_image = export_logic::render_overlay_image(
            width as u32,
            height as u32,
            &self.editor.sequences,
            &sequence_indices,
            self.export_dialog.include_points_in_overlay,
        );

        let default_file_name = "overlay_only.png".to_string();

        let file = FileDialog::new()
            .add_filter("PNG", &["png"])
            .set_file_name(&default_file_name)
            .save_file();

        let Some(path) = file else {
            self.status_message = "Overlay-only export cancelled".to_string();
            return;
        };

        match rgba_image.save(&path) {
            Ok(()) => {
                self.status_message = format!("Exported overlay only: {}", path.display());
            }
            Err(err) => {
                self.status_message = format!("Failed to save overlay-only image: {err}");
            }
        }
    }
}

impl eframe::App for DotToDotStudioApp {
    // `update()` is the heart of an egui app.
    //
    // It is called repeatedly every frame. In immediate mode GUI, we rebuild
    // the full UI each frame from the current state.
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let close_requested = ctx.input(|i| i.viewport().close_requested());

        if close_requested {
            if self.allow_close_without_prompt {
                // Allow this close request to proceed without showing the dialog again.
            } else if self.editor.has_unsaved_changes {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.handle_close_request(ctx);
            }
        }

        self.handle_shortcuts(ctx);
        self.draw_menu_bar(ctx);
        self.draw_status_bar(ctx);

        project_panel::show_project_panel(ctx, self);
        sequences_panel::show_sequences_panel(ctx, self);
        export::show_export_dialog(ctx, self);

        self.draw_central_panel(ctx);
        self.ui_placeholder(frame);
    }

    // The current setup also expects this method.
    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {}
}
