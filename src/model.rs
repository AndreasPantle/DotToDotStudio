// The domain model describes the application's data in a UI-independent way.
//
// This file contains the core data structures of DotToDotStudio.
// These structs describe the project itself, not the UI state.
//
// We derive:
//
// - Debug:
//   Allows printing values with `{:?}` for debugging.
//
// - Clone:
//   Allows creating explicit copies with `.clone()`.
//
// Both are perfectly fine in normal and production builds.
// `Clone` should simply be used consciously for larger data structures.

#![allow(dead_code)]

#[derive(Debug, Clone)]
pub struct Project {
    /// Display name of the project.
    pub name: String,

    /// Optional URL that describes where the image or idea came from.
    pub origin_url: Option<String>,

    /// Free-text comment or description for the whole project.
    pub comment: String,

    /// The image that belongs to this project.
    ///
    /// For now, we assume one image per project.
    pub image: Option<ImageAsset>,

    /// All sequences defined in the project.
    pub sequences: Vec<Sequence>,
}

#[derive(Debug, Clone)]
pub struct ImageAsset {
    /// Original imported file name, e.g. "p11.jpg".
    pub filename: String,

    /// Optional MIME type such as "image/png" or "image/jpeg".
    pub mime_type: Option<String>,

    /// Width of the image in pixels.
    pub width: u32,

    /// Height of the image in pixels.
    pub height: u32,

    /// Raw original file bytes.
    ///
    /// This is suitable for later storage in SQLite as a BLOB.
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Sequence {
    /// Stable identifier of the sequence.
    pub id: String,

    /// Human-readable display name.
    pub name: String,

    /// Main color of the sequence.
    pub color: ColorStyle,

    /// Rendering settings for the sequence.
    pub appearance: SequenceAppearance,

    /// Points belonging to the sequence.
    pub points: Vec<Point>,
}

#[derive(Debug, Clone)]
pub struct Point {
    /// Point label, e.g. "1", "2", "A", or "Start".
    pub label: String,

    /// Horizontal image coordinate.
    pub x: f32,

    /// Vertical image coordinate.
    pub y: f32,
}

#[derive(Debug, Clone)]
pub struct ColorStyle {
    /// Red channel (0-255).
    pub r: u8,

    /// Green channel (0-255).
    pub g: u8,

    /// Blue channel (0-255).
    pub b: u8,

    /// Alpha channel (0-255).
    pub a: u8,
}

#[derive(Debug, Clone)]
pub struct SequenceAppearance {
    /// Thickness of the rendered line.
    pub line_width: f32,

    /// Radius of the rendered point marker.
    pub point_radius: f32,

    /// Whether line segments between points are visible.
    pub show_lines: bool,

    /// Whether point markers are visible.
    pub show_points: bool,
}
