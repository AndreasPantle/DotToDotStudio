use crate::app::SequenceItem;

/// Sanitize a string so it can safely be used as a file name.
///
/// This is intentionally conservative and replaces characters that are commonly
/// invalid or problematic on desktop operating systems.
///
/// The function does not try to preserve extensions or perform path logic.
/// It simply turns a free-form label such as a sequence name into a file-name-safe
/// fragment that can be used by the caller.
pub fn sanitize_file_name(name: &str) -> String {
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

/// Escape a string for use as one CSV field.
///
/// Rules used here:
/// - Double quotes are doubled
/// - Fields containing comma, quote, or newline are quoted
///
/// This is a simple and sufficient CSV escaping strategy for the exports
/// produced by this application.
fn csv_escape(value: &str) -> String {
    let escaped = value.replace('"', "\"\"");

    if escaped.contains(',') || escaped.contains('"') || escaped.contains('\n') {
        format!("\"{}\"", escaped)
    } else {
        escaped
    }
}

/// Build CSV output for one selected sequence.
///
/// The exported format intentionally includes some sequence metadata
/// on every row so the CSV remains self-describing when viewed on its own.
pub fn build_csv_for_selected_sequence(sequence_index: usize, sequence: &SequenceItem) -> String {
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

/// Build CSV output for all sequences in the project.
///
/// Every point becomes one row. Sequence information is repeated per row,
/// which keeps the result easy to consume in spreadsheet tools and scripts.
pub fn build_csv_for_all_sequences(sequences: &[SequenceItem]) -> String {
    let mut csv = String::new();
    csv.push_str("sequence_index,sequence_name,start_value,point_index,point_value,x,y\n");

    for (sequence_index, sequence) in sequences.iter().enumerate() {
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

/// Draw a filled circle onto an RGBA image.
///
/// This is used both for point markers and as the primitive that creates
/// thicker line strokes by repeatedly stamping circles along a line path.
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

/// Draw a visually thick line by stamping filled circles along the line path.
///
/// This is intentionally simple and robust:
/// - it does not try to be anti-aliased
/// - it does not depend on external geometry libraries
/// - it respects the per-sequence line thickness used by the editor
///
/// For this application's current export needs, this trade-off is acceptable.
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

/// Return true if at least one visible exportable element exists for the given
/// sequence selection.
///
/// "Exportable" here means:
/// - a visible sequence
/// - with at least one line segment, or
/// - with at least one point when point export is enabled
///
/// This helper is useful for early validation before opening a save dialog.
pub fn has_visible_overlay_content(
    sequences: &[SequenceItem],
    sequence_indices: &[usize],
    include_points: bool,
) -> bool {
    sequence_indices.iter().any(|&sequence_index| {
        let sequence = &sequences[sequence_index];

        if !sequence.visible {
            return false;
        }

        let has_lines = sequence.points.len() >= 2;
        let has_points = include_points && !sequence.points.is_empty();

        has_lines || has_points
    })
}

/// Render a transparent overlay image for the selected sequences.
///
/// The result contains only the overlay graphics:
/// - lines
/// - optional points
///
/// The background is fully transparent.
pub fn render_overlay_image(
    width: u32,
    height: u32,
    sequences: &[SequenceItem],
    sequence_indices: &[usize],
    include_points: bool,
) -> image::RgbaImage {
    let mut rgba_image = image::RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, 0]));

    for &sequence_index in sequence_indices {
        let sequence = &sequences[sequence_index];

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

            draw_thick_line(
                &mut rgba_image,
                start.position.x,
                start.position.y,
                end.position.x,
                end.position.y,
                sequence.line_thickness,
                overlay_color,
            );
        }

        if include_points {
            for point in &sequence.points {
                draw_filled_circle(
                    &mut rgba_image,
                    point.position.x.round() as i32,
                    point.position.y.round() as i32,
                    5,
                    overlay_color,
                );
            }
        }
    }

    rgba_image
}

/// Render the original image plus sequence overlays.
///
/// This function clones the base image and paints the selected sequence overlays
/// on top of it, returning a new RGBA image.
pub fn render_image_with_overlay(
    base_image: &image::RgbaImage,
    sequences: &[SequenceItem],
    sequence_indices: &[usize],
    include_points: bool,
) -> image::RgbaImage {
    let mut rgba_image = base_image.clone();

    for &sequence_index in sequence_indices {
        let sequence = &sequences[sequence_index];

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

            draw_thick_line(
                &mut rgba_image,
                start.position.x,
                start.position.y,
                end.position.x,
                end.position.y,
                sequence.line_thickness,
                overlay_color,
            );
        }

        if include_points {
            for point in &sequence.points {
                draw_filled_circle(
                    &mut rgba_image,
                    point.position.x.round() as i32,
                    point.position.y.round() as i32,
                    5,
                    overlay_color,
                );
            }
        }
    }

    rgba_image
}
