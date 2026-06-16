use egui::Pos2;

/// A single editable point inside a sequence.
///
/// The point is stored in image-space coordinates so it stays stable even when
/// the GUI zoom level or pan offset changes.
#[derive(Clone)]
pub struct SequencePoint {
    /// Point position in image space.
    pub position: Pos2,

    /// Numeric label/value of this point.
    pub value: i32,
}

/// A lightweight editor-facing sequence model.
///
/// This type intentionally contains only the properties that the current editor
/// needs for interactive editing and rendering.
#[derive(Clone)]
pub struct SequenceItem {
    /// Human-readable sequence name shown in the UI.
    pub name: String,

    /// Whether this sequence should currently be visible.
    pub visible: bool,

    /// Display color used by the current UI.
    pub color: egui::Color32,

    /// Thickness of rendered line segments for this sequence.
    pub line_thickness: f32,

    /// First numeric value of the sequence.
    pub start_value: i32,

    /// All points that belong to this sequence.
    pub points: Vec<SequencePoint>,
}

/// A snapshot of editor state used for undo.
///
/// For now, undo stores:
/// - all sequences
/// - selected sequence
/// - selected point
#[derive(Clone)]
pub struct EditorSnapshot {
    pub sequences: Vec<SequenceItem>,
    pub selected_sequence: Option<usize>,
    pub selected_point: Option<usize>,
}

/// GUI-independent editor state.
///
/// This struct holds the editable project data and the current editing
/// selection/navigation state. It deliberately does not contain GUI renderer
/// resources such as textures, zoom, pan, or mouse hover information.
pub struct EditorState {
    /// Editable project metadata.
    pub project_name: String,
    pub origin_url: String,
    pub comment: String,

    /// All editable sequences.
    pub sequences: Vec<SequenceItem>,

    /// Current sequence selection.
    pub selected_sequence: Option<usize>,

    /// Current point selection inside the selected sequence.
    pub selected_point: Option<usize>,

    /// When true, the GUI should scroll the selected point row into view.
    pub scroll_selected_point_into_view: bool,

    /// Currently dragged point as (sequence_index, point_index).
    pub dragged_point: Option<(usize, usize)>,

    /// Undo stack.
    pub undo_stack: Vec<EditorSnapshot>,

    /// Dirty flag for unsaved changes.
    pub has_unsaved_changes: bool,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
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
            has_unsaved_changes: false,
        }
    }
}

impl EditorState {
    /// Reset the editor to a new empty project.
    pub fn new_project(&mut self) {
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
        self.scroll_selected_point_into_view = false;
        self.clear_dirty();
    }

    /// Save the current editable state so it can be restored with Undo later.
    pub fn push_undo_snapshot(&mut self) {
        self.undo_stack.push(EditorSnapshot {
            sequences: self.sequences.clone(),
            selected_sequence: self.selected_sequence,
            selected_point: self.selected_point,
        });

        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
    }

    /// Restore the most recent editor snapshot.
    ///
    /// Returns `true` when an undo step was applied, `false` otherwise.
    pub fn undo(&mut self) -> bool {
        let Some(snapshot) = self.undo_stack.pop() else {
            return false;
        };

        self.sequences = snapshot.sequences;
        self.selected_sequence = snapshot.selected_sequence;
        self.selected_point = snapshot.selected_point;
        self.dragged_point = None;

        true
    }

    /// Mark the current project state as modified.
    pub fn mark_dirty(&mut self) {
        self.has_unsaved_changes = true;
    }

    /// Mark the current project state as saved/clean.
    pub fn clear_dirty(&mut self) {
        self.has_unsaved_changes = false;
    }

    /// Add a point to the currently selected sequence.
    ///
    /// Returns the added point value and sequence name on success.
    pub fn add_point_to_selected_sequence(
        &mut self,
        image_pos: Pos2,
    ) -> Result<(i32, String), &'static str> {
        let Some(sequence_index) = self.selected_sequence else {
            return Err("No sequence selected");
        };

        let Some(sequence) = self.sequences.get(sequence_index) else {
            return Err("Selected sequence is invalid");
        };

        let point_value = sequence.start_value + sequence.points.len() as i32;

        self.push_undo_snapshot();

        let sequence_name = {
            let Some(sequence) = self.sequences.get_mut(sequence_index) else {
                return Err("Selected sequence is invalid");
            };

            sequence.points.push(SequencePoint { position: image_pos, value: point_value });

            self.selected_point = Some(sequence.points.len() - 1);
            self.scroll_selected_point_into_view = true;

            sequence.name.clone()
        };

        self.mark_dirty();

        Ok((point_value, sequence_name))
    }

    /// Renumber all points of the selected sequence starting from its start value.
    ///
    /// Returns the sequence name and start value on success.
    pub fn renumber_selected_sequence_from_start(
        &mut self,
    ) -> Result<(String, i32), &'static str> {
        let Some(sequence_index) = self.selected_sequence else {
            return Err("No sequence selected");
        };

        let Some(sequence) = self.sequences.get(sequence_index) else {
            return Err("Selected sequence is invalid");
        };

        let sequence_name = sequence.name.clone();
        let start_value = sequence.start_value;

        self.push_undo_snapshot();

        let Some(sequence) = self.sequences.get_mut(sequence_index) else {
            return Err("Selected sequence is invalid");
        };

        for (index, point) in sequence.points.iter_mut().enumerate() {
            point.value = sequence.start_value + index as i32;
        }

        self.mark_dirty();

        Ok((sequence_name, start_value))
    }

    /// Renumber all points from the selected point onward.
    ///
    /// Returns the sequence name and the selected point index on success.
    pub fn renumber_selected_point_and_following(
        &mut self,
    ) -> Result<(String, usize), &'static str> {
        let Some(sequence_index) = self.selected_sequence else {
            return Err("No sequence selected");
        };

        let Some(point_index) = self.selected_point else {
            return Err("No point selected");
        };

        let Some(sequence) = self.sequences.get(sequence_index) else {
            return Err("Selected sequence is invalid");
        };

        if point_index >= sequence.points.len() {
            return Err("Selected point is invalid");
        }

        let start_value = sequence.points[point_index].value;
        let sequence_name = sequence.name.clone();

        self.push_undo_snapshot();

        let Some(sequence) = self.sequences.get_mut(sequence_index) else {
            return Err("Selected sequence is invalid");
        };

        for i in (point_index + 1)..sequence.points.len() {
            sequence.points[i].value = start_value + (i - point_index) as i32;
        }

        self.mark_dirty();

        Ok((sequence_name, point_index))
    }

    /// Remove the currently selected point from the selected sequence.
    ///
    /// Returns the sequence name on success.
    pub fn remove_selected_point(&mut self) -> Result<String, &'static str> {
        let Some(sequence_index) = self.selected_sequence else {
            return Err("No sequence selected");
        };

        let Some(point_index) = self.selected_point else {
            return Err("No point selected");
        };

        let Some(sequence) = self.sequences.get(sequence_index) else {
            return Err("Selected sequence is invalid");
        };

        if point_index >= sequence.points.len() {
            return Err("Selected point is invalid");
        }

        self.push_undo_snapshot();

        let sequence_name = {
            let Some(sequence) = self.sequences.get_mut(sequence_index) else {
                return Err("Selected sequence is invalid");
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

        Ok(sequence_name)
    }

    /// Try to select an existing point near the given image position.
    ///
    /// Returns the selected sequence name and point value on success.
    pub fn select_point_near_image_position(
        &mut self,
        image_pos: Pos2,
    ) -> Option<(String, i32)> {
        let (sequence_index, point_index) = self.find_point_near_image_position(image_pos)?;

        self.selected_sequence = Some(sequence_index);
        self.selected_point = Some(point_index);
        self.scroll_selected_point_into_view = true;

        let sequence_name = self.sequences[sequence_index].name.clone();
        let point_value = self.sequences[sequence_index].points[point_index].value;

        Some((sequence_name, point_value))
    }

    /// Find the nearest visible point close to the given image position.
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
                        Some((_, _, best_distance_sq)) if distance_sq < best_distance_sq => {
                            best_match = Some((sequence_index, point_index, distance_sq));
                        }
                        None => {
                            best_match = Some((sequence_index, point_index, distance_sq));
                        }
                        _ => {}
                    }
                }
            }
        }

        best_match.map(|(sequence_index, point_index, _)| (sequence_index, point_index))
    }

    /// Move the currently dragged point to a new image position.
    ///
    /// Returns the moved point value on success.
    pub fn move_dragged_point_to(&mut self, image_pos: Pos2) -> Option<i32> {
        let (sequence_index, point_index) = self.dragged_point?;

        let point_value = {
            let sequence = self.sequences.get_mut(sequence_index)?;
            let point = sequence.points.get_mut(point_index)?;
            point.position = image_pos;
            point.value
        };

        self.mark_dirty();
        Some(point_value)
    }

    /// Select the first point of the currently selected sequence.
    ///
    /// Returns the sequence name on success.
    pub fn jump_to_selected_sequence_start(&mut self) -> Result<String, &'static str> {
        let Some(sequence_index) = self.selected_sequence else {
            return Err("No sequence selected");
        };

        let Some(sequence) = self.sequences.get(sequence_index) else {
            return Err("Selected sequence is invalid");
        };

        if sequence.points.is_empty() {
            return Err("Sequence has no points");
        }

        self.selected_point = Some(0);
        self.scroll_selected_point_into_view = true;

        Ok(sequence.name.clone())
    }

    /// Select the last point of the currently selected sequence.
    ///
    /// Returns the sequence name on success.
    pub fn jump_to_selected_sequence_end(&mut self) -> Result<String, &'static str> {
        let Some(sequence_index) = self.selected_sequence else {
            return Err("No sequence selected");
        };

        let Some(sequence) = self.sequences.get(sequence_index) else {
            return Err("Selected sequence is invalid");
        };

        if sequence.points.is_empty() {
            return Err("Sequence has no points");
        }

        let last_index = sequence.points.len() - 1;

        self.selected_point = Some(last_index);
        self.scroll_selected_point_into_view = true;

        Ok(sequence.name.clone())
    }

    /// Select the previous point in the currently selected sequence.
    ///
    /// Returns the selected point value and sequence name on success.
    pub fn select_previous_point_in_sequence(&mut self) -> Result<(i32, String), &'static str> {
        let Some(sequence_index) = self.selected_sequence else {
            return Err("No sequence selected");
        };

        let Some(sequence) = self.sequences.get(sequence_index) else {
            return Err("Selected sequence is invalid");
        };

        if sequence.points.is_empty() {
            return Err("Sequence has no points");
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

        Ok((point_value, sequence_name))
    }

    /// Select the next point in the currently selected sequence.
    ///
    /// Returns the selected point value and sequence name on success.
    pub fn select_next_point_in_sequence(&mut self) -> Result<(i32, String), &'static str> {
        let Some(sequence_index) = self.selected_sequence else {
            return Err("No sequence selected");
        };

        let Some(sequence) = self.sequences.get(sequence_index) else {
            return Err("Selected sequence is invalid");
        };

        if sequence.points.is_empty() {
            return Err("Sequence has no points");
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

        Ok((point_value, sequence_name))
    }
}
