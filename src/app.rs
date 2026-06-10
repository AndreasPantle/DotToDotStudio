use crate::storage;
use crate::ui::{export, project_panel, sequences_panel};
use eframe::egui;
use egui::{ColorImage, Pos2, Rect, Sense, TextureHandle, Vec2};
use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};

// A single editable point inside a sequence.
//
// Each point stores:
// - its position in image coordinates
// - its numeric value shown in the editor and on the canvas
#[derive(Clone)]
pub struct SequencePoint {
    // Point position in image space.
    pub position: Pos2,

    // Numeric label/value of this point.
    pub value: i32,
}

// A small UI-focused sequence type used by the current editor state.
//
// For now, this is intentionally simpler than the full domain model in `model.rs`.
// It stores only the properties that we currently want to edit in the UI.
#[derive(Clone)]
pub struct SequenceItem {
    // Human-readable sequence name shown in the right panel.
    pub name: String,

    // Whether this sequence should be visible in the editor.
    pub visible: bool,

    // Display color used for this sequence inside the UI.
    //
    // We use `egui::Color32` here because this struct is currently part of the UI state,
    // not the long-term persistence/domain model.
    pub color: egui::Color32,

    // Thickness of rendered line segments for this sequence.
    pub line_thickness: f32,

    // First numeric value of the sequence.
    //
    // Example:
    // If `start_value` is 5, new points will be numbered 5, 6, 7, ...
    pub start_value: i32,

    // All points that belong to this sequence.
    //
    // The coordinates are stored in image space, not screen space.
    // That means they stay stable even when zoom and pan change.
    pub points: Vec<SequencePoint>,
}

// Export scope options for the export dialog.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ExportScope {
    SelectedSequence,
    AllSequences,
}

// UI state for the export dialog.
//
// This state is intentionally small for now and will grow as the export
// feature becomes more capable.
pub struct ExportDialogState {
    pub open: bool,
    pub export_points_as_csv: bool,
    pub export_image_without_overlay: bool,
    pub export_image_with_overlay: bool,
    pub export_overlay_only: bool,
    pub include_points_in_overlay: bool,
    pub scope: ExportScope,
}

// A snapshot of the editable editor state used for Undo.
//
// For now we only store the parts of the state that are changed by editing:
// - all sequences
// - selected sequence
// - selected point
#[derive(Clone)]
pub struct EditorSnapshot {
    pub sequences: Vec<SequenceItem>,
    pub selected_sequence: Option<usize>,
    pub selected_point: Option<usize>,
}

// This struct stores the full UI/application state.
// In egui/eframe, the app state lives inside a struct like this,
// and the `update()` method redraws the UI every frame based on that state.
pub struct DotToDotStudioApp {
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

    // Editable project metadata.
    pub project_name: String,
    pub origin_url: String,
    pub comment: String,

    // Simple sequence list for the right panel.
    pub sequences: Vec<SequenceItem>,
    pub selected_sequence: Option<usize>,
    pub selected_point: Option<usize>,

    // Scrolling the selected point into view is triggered when a point is selected from the right panel list.
    pub scroll_selected_point_into_view: bool,
    pub dragged_point: Option<(usize, usize)>,
    pub undo_stack: Vec<EditorSnapshot>,

    // State for the export dialog.
    pub export_dialog: ExportDialogState,

    // Dirty flag to track unsaved changes.
    pub has_unsaved_changes: bool,

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
            image_texture: None,
            image_name: None,
            image_size: None,
            image_size_bytes: None,
            image_bytes: None,
            status_message: "Ready".to_string(),
            project_name: "Untitled Project".to_string(),
            origin_url: String::new(),
            comment: String::new(),
            sequences: vec![SequenceItem {
                name: "Sequence 1".to_string(),
                visible: true,
                color: egui::Color32::from_rgb(120, 180, 255),
                line_thickness: 3.0,
                start_value: 1,
                points: Vec::new(),
            }],
            selected_sequence: Some(0),
            selected_point: None,
            scroll_selected_point_into_view: false,
            dragged_point: None,
            undo_stack: Vec::new(),
            export_dialog: ExportDialogState {
                open: false,
                export_points_as_csv: false,
                export_image_without_overlay: false,
                export_image_with_overlay: true,
                export_overlay_only: false,
                include_points_in_overlay: true,
                scope: ExportScope::SelectedSequence,
            },
            has_unsaved_changes: false,
            allow_close_without_prompt: false,
            zoom: 1.0,
            pan: Vec2::ZERO,
            viewport_size: Vec2::ZERO,
            mouse_view_pos: None,
            mouse_image_pos: None,
        }
    }
}

fn sanitize_file_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => ch,
        })
        .collect();

    let trimmed = sanitized.trim();

    if trimmed.is_empty() { "export".to_string() } else { trimmed.to_string() }
}

fn csv_escape(value: &str) -> String {
    let escaped = value.replace('"', "\"\"");

    if escaped.contains(',') || escaped.contains('"') || escaped.contains('\n') {
        format!("\"{}\"", escaped)
    } else {
        escaped
    }
}

fn draw_filled_circle(
    image: &mut image::RgbaImage,
    center_x: i32,
    center_y: i32,
    radius: i32,
    color: image::Rgba<u8>,
) {
    let radius_sq = radius * radius;

    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy <= radius_sq {
                let x = center_x + dx;
                let y = center_y + dy;

                if x >= 0 && y >= 0 && (x as u32) < image.width() && (y as u32) < image.height() {
                    image.put_pixel(x as u32, y as u32, color);
                }
            }
        }
    }
}

// fn draw_line(
//     image: &mut image::RgbaImage,
//     start_x: f32,
//     start_y: f32,
//     end_x: f32,
//     end_y: f32,
//     color: image::Rgba<u8>,
// ) {
//     let dx = end_x - start_x;
//     let dy = end_y - start_y;

//     let steps = dx.abs().max(dy.abs()) as i32;

//     if steps <= 0 {
//         let x = start_x.round() as i32;
//         let y = start_y.round() as i32;

//         if x >= 0 && y >= 0 && (x as u32) < image.width() && (y as u32) < image.height() {
//             image.put_pixel(x as u32, y as u32, color);
//         }

//         return;
//     }

//     for step in 0..=steps {
//         let t = step as f32 / steps as f32;
//         let x = start_x + dx * t;
//         let y = start_y + dy * t;

//         let px = x.round() as i32;
//         let py = y.round() as i32;

//         if px >= 0 && py >= 0 && (px as u32) < image.width() && (py as u32) < image.height() {
//             image.put_pixel(px as u32, py as u32, color);
//         }
//     }
// }

fn draw_thick_line(
    image: &mut image::RgbaImage,
    start_x: f32,
    start_y: f32,
    end_x: f32,
    end_y: f32,
    thickness: f32,
    color: image::Rgba<u8>,
) {
    let dx = end_x - start_x;
    let dy = end_y - start_y;

    let steps = dx.abs().max(dy.abs()) as i32;
    let radius = (thickness.max(1.0) * 0.5).round() as i32;

    if steps <= 0 {
        draw_filled_circle(
            image,
            start_x.round() as i32,
            start_y.round() as i32,
            radius.max(1),
            color,
        );
        return;
    }

    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let x = start_x + dx * t;
        let y = start_y + dy * t;

        draw_filled_circle(image, x.round() as i32, y.round() as i32, radius.max(1), color);
    }
}

impl DotToDotStudioApp {
    // Open a native file dialog, load an image from disk,
    // keep the original file bytes for later database storage,
    // decode the image, and upload it as an egui texture.
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
                // Convert the imported image into RGBA pixel data.
                let rgba_image = dynamic_image.to_rgba8();
                let width = rgba_image.width() as usize;
                let height = rgba_image.height() as usize;
                let pixels = rgba_image.into_raw();

                // Convert raw RGBA bytes into an egui image type.
                let color_image = ColorImage::from_rgba_unmultiplied([width, height], &pixels);

                // Upload the image into a GPU texture so egui can display it.
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
                self.mark_dirty();
                self.status_message = format!("Imported image: {}", path.display());
            }
            Err(err) => {
                self.status_message = format!("Failed to decode image: {err}");
            }
        }
    }

    // Decode raw image bytes and upload them as an egui texture.
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
                self.clear_dirty();
                self.allow_close_without_prompt = false;
                self.status_message = format!("Project saved: {}", path.display());
            }
            Err(err) => {
                self.status_message = format!("Failed to save project: {err}");
            }
        }
    }

    // Reset the application to a fresh empty project state.
    fn new_project(&mut self) {
        self.image_texture = None;
        self.image_name = None;
        self.image_size = None;
        self.image_size_bytes = None;
        self.image_bytes = None;

        self.project_name = "Untitled Project".to_string();
        self.origin_url = String::new();
        self.comment = String::new();

        self.sequences = vec![SequenceItem {
            name: "Sequence 1".to_string(),
            visible: true,
            color: egui::Color32::from_rgb(120, 180, 255),
            line_thickness: 3.0,
            start_value: 1,
            points: Vec::new(),
        }];

        self.selected_sequence = Some(0);
        self.selected_point = None;
        self.dragged_point = None;
        self.undo_stack.clear();

        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
        self.mouse_view_pos = None;
        self.mouse_image_pos = None;

        self.allow_close_without_prompt = false;
        self.clear_dirty();
        self.scroll_selected_point_into_view = false;

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

                self.project_name = loaded.project_name;
                self.origin_url = loaded.origin_url;
                self.comment = loaded.comment;
                self.image_name = loaded.image_name;
                self.image_size = loaded.image_size;
                self.image_size_bytes = loaded.image_size_bytes;
                self.image_bytes = loaded.image_bytes;
                self.image_texture = texture;

                self.sequences = loaded
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

                self.selected_sequence = if self.sequences.is_empty() { None } else { Some(0) };

                self.selected_point = None;
                self.dragged_point = None;
                self.undo_stack.clear();
                self.allow_close_without_prompt = false;
                self.zoom = 1.0;
                self.pan = Vec2::ZERO;
                self.scroll_selected_point_into_view = false;

                self.clear_dirty();

                self.status_message = format!("Project loaded: {}", path.display());
            }
            Err(err) => {
                self.status_message = format!("Failed to load project: {err}");
            }
        }
    }

    // Export the embedded image bytes back to a normal image file on disk.
    fn export_image(&mut self) {
        self.export_image_without_overlay();
    }

    // Draw the top menu bar.
    // Right now it contains:
    // - File menu
    // - View menu
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
                        self.undo();
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
    // It shows general app status and also information about:
    // - zoom
    // - pan
    // - current image name
    // - current mouse position
    fn draw_status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(&self.status_message);

                if self.has_unsaved_changes {
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
    // If an image is loaded, it is displayed here and can be zoomed/panned.
    // If no image is loaded yet, we show a simple placeholder message.
    fn draw_central_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(texture) = &self.image_texture {
                // Copy small values out of the texture first.
                // This avoids borrow conflicts later when we need `&mut self`.
                let texture_id = texture.id();
                let texture_size = texture.size_vec2();

                let available_size = ui.available_size();
                self.viewport_size = available_size;

                // Allocate a painter area that reacts to dragging.
                // The painter is used to draw the image manually.
                let (response, painter) =
                    ui.allocate_painter(available_size, Sense::click_and_drag());

                let rect = response.rect;

                // Compute the image rectangle on screen.
                // The displayed size depends on the current zoom.
                // The top-left corner depends on the pan offset.
                let image_size = texture_size * self.zoom;
                let image_top_left = rect.min + self.pan;
                let image_rect = Rect::from_min_size(image_top_left, image_size);

                // Update the mouse position in both view coordinates and image coordinates.
                self.update_mouse_position(&response, image_rect);

                // Start dragging a point if the primary mouse button starts a drag
                // over an existing point.
                if response.drag_started_by(egui::PointerButton::Primary)
                    && let Some(image_pos) = self.mouse_image_pos
                    && let Some((sequence_index, point_index)) =
                        self.find_point_near_image_position(image_pos)
                {
                    self.push_undo_snapshot();

                    self.selected_sequence = Some(sequence_index);
                    self.selected_point = Some(point_index);
                    self.dragged_point = Some((sequence_index, point_index));

                    let point_value = self.sequences[sequence_index].points[point_index].value;

                    self.status_message = format!("Dragging point {}", point_value);
                }

                // On primary click:
                // - first try to select an existing nearby point
                // - if no point is nearby, add a new point
                //
                // Ignore plain click handling while a point drag is active.
                if response.clicked_by(egui::PointerButton::Primary) && self.dragged_point.is_none()
                {
                    if let Some(image_pos) = self.mouse_image_pos {
                        let did_select_existing = self.select_point_near_image_position(image_pos);

                        if !did_select_existing {
                            self.add_point_to_selected_sequence(image_pos);
                        }
                    } else {
                        self.status_message = "Click was outside the image".to_string();
                    }
                }

                // Mouse wheel zooms the view when the viewport is hovered.
                //
                // We zoom around the current mouse position so the image point
                // under the cursor stays in place.
                if response.hovered() {
                    let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);

                    if scroll_delta != 0.0
                        && let Some(pointer_pos) = response.hover_pos()
                    {
                        let zoom_factor = (scroll_delta * 0.0015).exp();
                        self.zoom_at_pointer(pointer_pos, rect, image_rect, zoom_factor);
                    }
                }

                // If a point is currently being dragged, move it with the mouse.
                if response.dragged_by(egui::PointerButton::Primary)
                    && self.dragged_point.is_some()
                    && let Some(image_pos) = self.mouse_image_pos
                {
                    self.move_dragged_point_to(image_pos);
                }

                // Dragging moves the image only when we are not dragging a point.
                if response.dragged() && self.dragged_point.is_none() {
                    let delta = ui.input(|i| i.pointer.delta());
                    self.pan += delta;
                }

                // Stop point dragging when the primary mouse drag ends.
                if response.drag_stopped_by(egui::PointerButton::Primary) {
                    self.dragged_point = None;
                }

                // Draw the image into the painter area.
                // UV coordinates from (0,0) to (1,1) mean:
                // use the full texture.
                painter.image(
                    texture_id,
                    image_rect,
                    Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                    egui::Color32::WHITE,
                );

                // Draw all sequence points on top of the image.
                self.draw_sequence_points(&painter, image_rect);

                // Draw a border around the viewport.
                painter.rect_stroke(
                    rect,
                    0.0,
                    egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
                    egui::StrokeKind::Inside,
                );
            } else {
                // Placeholder UI shown before an image is loaded.
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
    // It exists because your current trait setup expects this structure.
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
    //
    // The goal is that the image point under the mouse cursor stays visually
    // under the cursor after zooming.
    fn zoom_at_pointer(
        &mut self,
        pointer_pos: Pos2,
        viewport_rect: Rect,
        image_rect: Rect,
        zoom_factor: f32,
    ) {
        let old_zoom = self.zoom;
        let new_zoom = (self.zoom * zoom_factor).clamp(0.05, 20.0);

        // If clamping prevented any real zoom change, stop early.
        if (new_zoom - old_zoom).abs() < f32::EPSILON {
            return;
        }

        // Convert the pointer position into image coordinates using the old zoom.
        let local_before_zoom = pointer_pos - image_rect.min;
        let image_pos = Pos2::new(local_before_zoom.x / old_zoom, local_before_zoom.y / old_zoom);

        // Update zoom first.
        self.zoom = new_zoom;

        // Compute the new top-left position of the image so that the same image point
        // remains under the mouse cursor after zooming.
        let new_image_top_left = Pos2::new(
            pointer_pos.x - image_pos.x * new_zoom,
            pointer_pos.y - image_pos.y * new_zoom,
        );

        // Convert that back into pan relative to the viewport origin.
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
    // We first collect actions into boolean flags and execute them afterwards.
    // This pattern avoids borrow conflicts and keeps the code readable.
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
            self.undo();
        }

        if jump_to_sequence_start {
            self.jump_to_selected_sequence_start();
        }

        if jump_to_sequence_end {
            self.jump_to_selected_sequence_end();
        }

        if select_previous_point {
            self.select_previous_point_in_sequence();
        }

        if select_next_point {
            self.select_next_point_in_sequence();
        }
    }

    // Save the current editable state so it can be restored with Undo later.
    pub fn push_undo_snapshot(&mut self) {
        self.undo_stack.push(EditorSnapshot {
            sequences: self.sequences.clone(),
            selected_sequence: self.selected_sequence,
            selected_point: self.selected_point,
        });

        // Keep the undo stack bounded so it cannot grow forever.
        //
        // For now, 100 steps is more than enough for this small editor.
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
    }

    // Restore the most recent editor snapshot.
    pub fn undo(&mut self) {
        let Some(snapshot) = self.undo_stack.pop() else {
            self.status_message = "Nothing to undo".to_string();
            return;
        };

        self.sequences = snapshot.sequences;
        self.selected_sequence = snapshot.selected_sequence;
        self.selected_point = snapshot.selected_point;
        self.dragged_point = None;

        self.status_message = "Undo".to_string();
    }

    // Add a point to the currently selected sequence.
    //
    // The point must already be in image coordinates.
    fn add_point_to_selected_sequence(&mut self, image_pos: Pos2) {
        let Some(sequence_index) = self.selected_sequence else {
            self.status_message = "No sequence selected".to_string();
            return;
        };

        let Some(sequence) = self.sequences.get(sequence_index) else {
            self.status_message = "Selected sequence is invalid".to_string();
            return;
        };

        let point_value = sequence.start_value + sequence.points.len() as i32;

        self.push_undo_snapshot();

        let sequence_name = {
            let Some(sequence) = self.sequences.get_mut(sequence_index) else {
                self.status_message = "Selected sequence is invalid".to_string();
                return;
            };

            sequence.points.push(SequencePoint { position: image_pos, value: point_value });

            self.selected_point = Some(sequence.points.len() - 1);
            self.scroll_selected_point_into_view = true;

            sequence.name.clone()
        };

        self.mark_dirty();

        self.status_message = format!(
            "Added point {} to {} at ({:.1}, {:.1})",
            point_value, sequence_name, image_pos.x, image_pos.y
        );
    }

    // Convert the current mouse position into image coordinates.
    //
    // `response.hover_pos()` gives the pointer position in UI/view coordinates.
    // If the pointer is inside the displayed image rectangle, we convert it back
    // into original image pixel coordinates by undoing pan and zoom.
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

    // Renumber all points of the selected sequence starting from its start value.
    pub fn renumber_selected_sequence_from_start(&mut self) {
        let Some(sequence_index) = self.selected_sequence else {
            self.status_message = "No sequence selected".to_string();
            return;
        };

        let Some(sequence) = self.sequences.get(sequence_index) else {
            self.status_message = "Selected sequence is invalid".to_string();
            return;
        };

        let sequence_name = sequence.name.clone();
        let start_value = sequence.start_value;

        self.push_undo_snapshot();

        let Some(sequence) = self.sequences.get_mut(sequence_index) else {
            self.status_message = "Selected sequence is invalid".to_string();
            return;
        };

        for (index, point) in sequence.points.iter_mut().enumerate() {
            point.value = sequence.start_value + index as i32;
        }

        self.mark_dirty();

        self.status_message = format!("Renumbered sequence {} from {}", sequence_name, start_value);
    }

    // Renumber all points from the selected point onward.
    //
    // The selected point keeps its current value.
    // Every following point gets incremented by 1.
    pub fn renumber_selected_point_and_following(&mut self) {
        let Some(sequence_index) = self.selected_sequence else {
            self.status_message = "No sequence selected".to_string();
            return;
        };

        let Some(point_index) = self.selected_point else {
            self.status_message = "No point selected".to_string();
            return;
        };

        let Some(sequence) = self.sequences.get(sequence_index) else {
            self.status_message = "Selected sequence is invalid".to_string();
            return;
        };

        if point_index >= sequence.points.len() {
            self.status_message = "Selected point is invalid".to_string();
            return;
        }

        let start_value = sequence.points[point_index].value;
        let sequence_name = sequence.name.clone();

        self.push_undo_snapshot();

        let Some(sequence) = self.sequences.get_mut(sequence_index) else {
            self.status_message = "Selected sequence is invalid".to_string();
            return;
        };

        for i in (point_index + 1)..sequence.points.len() {
            sequence.points[i].value = start_value + (i - point_index) as i32;
        }

        self.mark_dirty();

        self.status_message =
            format!("Renumbered points after point {} in {}", point_index + 1, sequence_name);
    }

    // Remove the currently selected point from the selected sequence.
    pub fn remove_selected_point(&mut self) {
        let Some(sequence_index) = self.selected_sequence else {
            self.status_message = "No sequence selected".to_string();
            return;
        };

        let Some(point_index) = self.selected_point else {
            self.status_message = "No point selected".to_string();
            return;
        };

        let Some(sequence) = self.sequences.get(sequence_index) else {
            self.status_message = "Selected sequence is invalid".to_string();
            return;
        };

        if point_index >= sequence.points.len() {
            self.status_message = "Selected point is invalid".to_string();
            return;
        }

        self.push_undo_snapshot();

        let sequence_name = {
            let Some(sequence) = self.sequences.get_mut(sequence_index) else {
                self.status_message = "Selected sequence is invalid".to_string();
                return;
            };

            sequence.points.remove(point_index);

            if sequence.points.is_empty() {
                self.selected_point = None;
            } else if point_index >= sequence.points.len() {
                self.selected_point = Some(sequence.points.len() - 1);
            }

            sequence.name.clone()
        };

        self.mark_dirty();

        self.status_message = format!("Removed point from {}", sequence_name);
    }

    // Try to select an existing point near the given image position.
    //
    // Returns `true` if a point was found and selected.
    // Returns `false` if no nearby point exists.
    pub fn select_point_near_image_position(&mut self, image_pos: Pos2) -> bool {
        if let Some((sequence_index, point_index)) = self.find_point_near_image_position(image_pos)
        {
            self.selected_sequence = Some(sequence_index);
            self.selected_point = Some(point_index);
            self.scroll_selected_point_into_view = true;

            let sequence_name = self.sequences[sequence_index].name.clone();
            let point_value = self.sequences[sequence_index].points[point_index].value;

            self.status_message = format!("Selected point {} in {}", point_value, sequence_name);

            true
        } else {
            false
        }
    }

    // Draw all visible sequences on top of the image.
    //
    // For each visible sequence we currently render:
    // - connecting lines between consecutive points
    // - point markers
    // - numeric labels next to each point
    //
    // All point coordinates are stored in image space, so we convert them
    // into screen space using the current image rectangle and zoom factor.
    fn draw_sequence_points(&self, painter: &egui::Painter, image_rect: Rect) {
        for (sequence_index, sequence) in self.sequences.iter().enumerate() {
            if !sequence.visible {
                continue;
            }

            // Convert all points of the current sequence from image coordinates
            // into screen coordinates.
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

            // Draw connecting line segments between consecutive points.
            for window in screen_points.windows(2) {
                let start = window[0];
                let end = window[1];

                //painter.line_segment([start, end], egui::Stroke::new(2.0, sequence.color));
                // Changed thikness from 2.0 to 3.0 for better visibility.
                //painter.line_segment([start, end], egui::Stroke::new(3.0, sequence.color));
                painter.line_segment(
                    [start, end],
                    egui::Stroke::new(sequence.line_thickness, sequence.color),
                );
            }

            // Draw all points and their labels.
            for (point_index, point) in sequence.points.iter().enumerate() {
                let screen_pos = screen_points[point_index];

                let is_selected = self.selected_sequence == Some(sequence_index)
                    && self.selected_point == Some(point_index);

                //let point_radius = if is_selected { 6.0 } else { 4.0 };
                let point_radius = if is_selected { 9.0 } else { 6.0 };
                let stroke_color =
                    if is_selected { egui::Color32::YELLOW } else { egui::Color32::BLACK };

                painter.circle_filled(screen_pos, point_radius, sequence.color);
                painter.circle_stroke(
                    screen_pos,
                    point_radius,
                    //egui::Stroke::new(1.5, stroke_color),
                    egui::Stroke::new(2.0, stroke_color),
                );

                painter.text(
                    screen_pos + Vec2::new(8.0, -8.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{}", point.value),
                    //egui::FontId::proportional(14.0),
                    egui::FontId::proportional(16.0),
                    sequence.color,
                );
            }
        }
    }

    // Find the nearest visible point close to the given image position.
    //
    // Returns:
    // - Some((sequence_index, point_index)) if a nearby point was found
    // - None otherwise
    pub fn find_point_near_image_position(&self, image_pos: Pos2) -> Option<(usize, usize)> {
        let pick_radius = 8.0;
        let pick_radius_sq = pick_radius * pick_radius;

        let mut best_match: Option<(usize, usize, f32)> = None;

        for (sequence_index, sequence) in self.sequences.iter().enumerate() {
            if !sequence.visible {
                continue;
            }

            for (point_index, point) in sequence.points.iter().enumerate() {
                let dx = point.position.x - image_pos.x;
                let dy = point.position.y - image_pos.y;
                let distance_sq = dx * dx + dy * dy;

                if distance_sq <= pick_radius_sq {
                    match best_match {
                        Some((_, _, best_distance_sq)) => {
                            if distance_sq < best_distance_sq {
                                best_match = Some((sequence_index, point_index, distance_sq));
                            }
                        }
                        None => {
                            best_match = Some((sequence_index, point_index, distance_sq));
                        }
                    }
                }
            }
        }

        best_match.map(|(sequence_index, point_index, _)| (sequence_index, point_index))
    }

    // Move the currently dragged point to a new image position.
    pub fn move_dragged_point_to(&mut self, image_pos: Pos2) {
        let Some((sequence_index, point_index)) = self.dragged_point else {
            return;
        };

        let point_value = {
            let Some(sequence) = self.sequences.get_mut(sequence_index) else {
                return;
            };

            let Some(point) = sequence.points.get_mut(point_index) else {
                return;
            };

            point.position = image_pos;
            point.value
        };

        self.mark_dirty();

        self.status_message =
            format!("Moved point {} to ({:.1}, {:.1})", point_value, image_pos.x, image_pos.y);
    }

    // Mark the current project state as modified.
    pub fn mark_dirty(&mut self) {
        self.has_unsaved_changes = true;
    }

    // Mark the current project state as saved/clean.
    pub fn clear_dirty(&mut self) {
        self.has_unsaved_changes = false;
    }

    // Ask the user whether they want to discard unsaved changes.
    //
    // Returns true if it is safe to continue with the destructive action.
    fn confirm_discard_unsaved_changes(&self, action_name: &str) -> bool {
        if !self.has_unsaved_changes {
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

    // Handle an external window close request, for example when the user clicks
    // the window manager close button.
    //
    // If the project has unsaved changes, ask for confirmation first.
    fn handle_close_request(&mut self, ctx: &egui::Context) {
        if self.confirm_discard_unsaved_changes("Close Window") {
            self.allow_close_without_prompt = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            self.allow_close_without_prompt = false;
            self.status_message = "Window close cancelled".to_string();
        }
    }

    // Select the first point of the currently selected sequence.
    fn jump_to_selected_sequence_start(&mut self) {
        let Some(sequence_index) = self.selected_sequence else {
            self.status_message = "No sequence selected".to_string();
            return;
        };

        let Some(sequence) = self.sequences.get(sequence_index) else {
            self.status_message = "Selected sequence is invalid".to_string();
            return;
        };

        if sequence.points.is_empty() {
            self.status_message = format!("Sequence {} has no points", sequence.name);
            return;
        }

        self.selected_point = Some(0);
        self.scroll_selected_point_into_view = true;

        self.status_message = format!("Jumped to first point in {}", sequence.name);
    }

    // Select the last point of the currently selected sequence.
    fn jump_to_selected_sequence_end(&mut self) {
        let Some(sequence_index) = self.selected_sequence else {
            self.status_message = "No sequence selected".to_string();
            return;
        };

        let Some(sequence) = self.sequences.get(sequence_index) else {
            self.status_message = "Selected sequence is invalid".to_string();
            return;
        };

        if sequence.points.is_empty() {
            self.status_message = format!("Sequence {} has no points", sequence.name);
            return;
        }

        let last_index = sequence.points.len() - 1;

        self.selected_point = Some(last_index);
        self.scroll_selected_point_into_view = true;

        self.status_message = format!("Jumped to last point in {}", sequence.name);
    }

    // Select the previous point in the currently selected sequence.
    fn select_previous_point_in_sequence(&mut self) {
        let Some(sequence_index) = self.selected_sequence else {
            self.status_message = "No sequence selected".to_string();
            return;
        };

        let Some(sequence) = self.sequences.get(sequence_index) else {
            self.status_message = "Selected sequence is invalid".to_string();
            return;
        };

        if sequence.points.is_empty() {
            self.status_message = format!("Sequence {} has no points", sequence.name);
            return;
        }

        let new_index = match self.selected_point {
            Some(index) if index > 0 => index - 1,
            Some(_) => 0,
            None => sequence.points.len() - 1,
        };

        let point_value = sequence.points[new_index].value;
        let sequence_name = sequence.name.clone();

        self.selected_point = Some(new_index);
        self.scroll_selected_point_into_view = true;

        self.status_message = format!("Selected point {} in {}", point_value, sequence_name);
    }

    // Select the next point in the currently selected sequence.
    fn select_next_point_in_sequence(&mut self) {
        let Some(sequence_index) = self.selected_sequence else {
            self.status_message = "No sequence selected".to_string();
            return;
        };

        let Some(sequence) = self.sequences.get(sequence_index) else {
            self.status_message = "Selected sequence is invalid".to_string();
            return;
        };

        if sequence.points.is_empty() {
            self.status_message = format!("Sequence {} has no points", sequence.name);
            return;
        }

        let last_index = sequence.points.len() - 1;

        let new_index = match self.selected_point {
            Some(index) if index < last_index => index + 1,
            Some(_) => last_index,
            None => 0,
        };

        let point_value = sequence.points[new_index].value;
        let sequence_name = sequence.name.clone();

        self.selected_point = Some(new_index);
        self.scroll_selected_point_into_view = true;

        self.status_message = format!("Selected point {} in {}", point_value, sequence_name);
    }

    // Export points as CSV according to the currently selected export scope.
    pub fn export_points_as_csv(&mut self) {
        let (default_file_name, csv_content) = match self.export_dialog.scope {
            ExportScope::SelectedSequence => {
                let Some(sequence_index) = self.selected_sequence else {
                    self.status_message = "No sequence selected for CSV export".to_string();
                    return;
                };

                let Some(sequence) = self.sequences.get(sequence_index) else {
                    self.status_message = "Selected sequence is invalid".to_string();
                    return;
                };

                if sequence.points.is_empty() {
                    self.status_message =
                        format!("Sequence {} has no points to export", sequence.name);
                    return;
                }

                let file_name = format!("{}.csv", sanitize_file_name(&sequence.name));
                let csv = self.build_csv_for_selected_sequence(sequence_index);

                (file_name, csv)
            }
            ExportScope::AllSequences => {
                if self.sequences.is_empty() {
                    self.status_message = "There are no sequences to export".to_string();
                    return;
                }

                let has_any_points =
                    self.sequences.iter().any(|sequence| !sequence.points.is_empty());

                if !has_any_points {
                    self.status_message = "There are no points to export".to_string();
                    return;
                }

                let csv = self.build_csv_for_all_sequences();
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

    // Build CSV output for the currently selected sequence.
    fn build_csv_for_selected_sequence(&self, sequence_index: usize) -> String {
        let sequence = &self.sequences[sequence_index];

        let mut csv = String::new();
        csv.push_str("sequence_index,sequence_name,start_value,point_index,point_value,x,y\n");

        for (point_index, point) in sequence.points.iter().enumerate() {
            csv.push_str(&format!(
                "{},{},{},{},{},{:.3},{:.3}\n",
                sequence_index,
                csv_escape(&sequence.name),
                sequence.start_value,
                point_index,
                point.value,
                point.position.x,
                point.position.y
            ));
        }

        csv
    }

    // Build CSV output for all sequences in the project.
    fn build_csv_for_all_sequences(&self) -> String {
        let mut csv = String::new();
        csv.push_str("sequence_index,sequence_name,start_value,point_index,point_value,x,y\n");

        for (sequence_index, sequence) in self.sequences.iter().enumerate() {
            for (point_index, point) in sequence.points.iter().enumerate() {
                csv.push_str(&format!(
                    "{},{},{},{},{},{:.3},{:.3}\n",
                    sequence_index,
                    csv_escape(&sequence.name),
                    sequence.start_value,
                    point_index,
                    point.value,
                    point.position.x,
                    point.position.y
                ));
            }
        }

        csv
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
    //
    // The first version draws:
    // - connecting lines
    // - point markers
    //
    // Numeric labels can be added later with a text rendering library.
    pub fn export_image_with_overlay(&mut self) {
        let Some(image_bytes) = &self.image_bytes else {
            self.status_message = "No embedded image available for overlay export".to_string();
            return;
        };

        let mut rgba_image = match image::load_from_memory(image_bytes) {
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

        let mut drew_anything = false;

        for sequence_index in sequence_indices {
            let sequence = &self.sequences[sequence_index];

            if !sequence.visible {
                continue;
            }

            let overlay_color = image::Rgba([
                sequence.color.r(),
                sequence.color.g(),
                sequence.color.b(),
                sequence.color.a(),
            ]);

            for window in sequence.points.windows(2) {
                let start = &window[0];
                let end = &window[1];

                // draw_line(
                //     &mut rgba_image,
                //     start.position.x,
                //     start.position.y,
                //     end.position.x,
                //     end.position.y,
                //     overlay_color,
                // );

                draw_thick_line(
                    &mut rgba_image,
                    start.position.x,
                    start.position.y,
                    end.position.x,
                    end.position.y,
                    sequence.line_thickness,
                    overlay_color,
                );

                drew_anything = true;
            }

            if self.export_dialog.include_points_in_overlay {
                for point in &sequence.points {
                    draw_filled_circle(
                        &mut rgba_image,
                        point.position.x.round() as i32,
                        point.position.y.round() as i32,
                        5,
                        overlay_color,
                    );

                    drew_anything = true;
                }
            }
        }

        if !drew_anything {
            self.status_message = "Nothing to export for the selected overlay scope".to_string();
            return;
        }

        let base_name = self
            .image_name
            .as_deref()
            .map(sanitize_file_name)
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

    // Return the sequence indices that should be exported for the current export scope.
    fn export_sequence_indices(&self) -> Vec<usize> {
        match self.export_dialog.scope {
            ExportScope::SelectedSequence => self.selected_sequence.into_iter().collect(),
            ExportScope::AllSequences => (0..self.sequences.len()).collect(),
        }
    }

    // Export only the overlay as a transparent PNG.
    //
    // This renders visible sequence overlays onto a transparent image without
    // including the original background image.
    pub fn export_overlay_only(&mut self) {
        let Some([width, height]) = self.image_size else {
            self.status_message = "No image size available for overlay export".to_string();
            return;
        };

        let mut rgba_image =
            image::RgbaImage::from_pixel(width as u32, height as u32, image::Rgba([0, 0, 0, 0]));

        let sequence_indices = self.export_sequence_indices();

        if sequence_indices.is_empty() {
            self.status_message = "No sequence selected for overlay export".to_string();
            return;
        }

        let mut drew_anything = false;

        for sequence_index in sequence_indices {
            let sequence = &self.sequences[sequence_index];

            if !sequence.visible {
                continue;
            }

            let overlay_color = image::Rgba([
                sequence.color.r(),
                sequence.color.g(),
                sequence.color.b(),
                sequence.color.a(),
            ]);

            for window in sequence.points.windows(2) {
                let start = &window[0];
                let end = &window[1];

                // draw_line(
                //     &mut rgba_image,
                //     start.position.x,
                //     start.position.y,
                //     end.position.x,
                //     end.position.y,
                //     overlay_color,
                // );

                draw_thick_line(
                    &mut rgba_image,
                    start.position.x,
                    start.position.y,
                    end.position.x,
                    end.position.y,
                    sequence.line_thickness,
                    overlay_color,
                );

                drew_anything = true;
            }

            if self.export_dialog.include_points_in_overlay {
                for point in &sequence.points {
                    draw_filled_circle(
                        &mut rgba_image,
                        point.position.x.round() as i32,
                        point.position.y.round() as i32,
                        5,
                        overlay_color,
                    );

                    drew_anything = true;
                }
            }
        }

        if !drew_anything {
            self.status_message = "Nothing to export for the selected overlay scope".to_string();
            return;
        }

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
    // It is called repeatedly every frame.
    // In immediate mode GUI, we rebuild the full UI each frame from the current state.
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let close_requested = ctx.input(|i| i.viewport().close_requested());

        if close_requested {
            if self.allow_close_without_prompt {
                // Allow this close request to proceed without showing the dialog again.
            } else if self.has_unsaved_changes {
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

    // Your current setup expects this method as well.
    // We do not use it yet, because the real UI is built in `update()`.
    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {}
}
