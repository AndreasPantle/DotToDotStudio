use crate::app::DotToDotStudioApp;
use crate::resources::DOTTODOTSTUDIO_ICON_BYTES;
use eframe::egui;

// -----------------------------------------------------------------------
// About text
// -----------------------------------------------------------------------
//
// Replace the content of this string with your own About text.
//
// Supported Markdown subset (intentionally small, no external crate):
// - "# ", "## ", "### "   -> headings (3 levels)
// - "**bold**"            -> bold inline text
// - "*italic*"             -> italic inline text
// - "- " or "* " (at line start, followed by a space) -> bullet list item
// - "---" or "***" on its own line -> horizontal separator
// - blank line            -> paragraph spacing
// - anything else         -> plain paragraph text (wraps automatically)
//
// Lines are processed independently, so each list item / paragraph should
// be its own line in the source string.
const ABOUT_TEXT: &str = r#"
**DotToDotStudio** is a small desktop tool for creating dot-to-dot images.

Version 0.1.0

## Features

- Import an image and place numbered points on it
- Organize points into multiple named sequences
- Export points as CSV
- Export images and overlays as PNG or scalable SVG

---

## Warum?

Dieses Projekt war das Resultat meiner ersten Ergo Therapie im Rahmen eines stationären Klinik Aufenthaltes im Universitätsklinikum Tübingen im Jahre 2026.

Herzlichen Dank an alle Damen und Herren die mich während dieser Zeit unterstützt haben.

---

## Why

This project is the result of my first occupational therapy experience during an inpatient stay at Tübingen University Hospital in 2026.

I would like to express my heartfelt gratitude to everyone who supported me during this period.

---

## License

Copyright 2026 Andreas Pantle

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

"#;

// State for the About dialog.
//
// This stays in the app adapter layer, mirroring `ExportDialogState`, since
// it is tied directly to the current GUI implementation.
#[derive(Default)]
pub struct AboutDialogState {
    pub open: bool,

    // Lazily decoded/uploaded on first display and cached here for the
    // lifetime of the app, so the PNG is decoded and sent to the GPU once
    // rather than every frame the dialog is open.
    icon_texture: Option<egui::TextureHandle>,
}

// Decode the bundled app icon and upload it as an egui texture.
//
// The icon is compiled into the binary via `include_bytes!` (see
// `resources::DOTTODOTSTUDIO_ICON_BYTES`), so decoding it can only fail if
// that asset itself is corrupt — treated as a programmer error, not
// something to recover from at runtime.
//
// The bundled PNG is pre-shrunk to `assets/icon-about-192.png` (2x the
// `ICON_SIZE` this dialog displays it at) rather than the full 1024px app
// icon. egui has no mipmapping, so shrinking a 1024px texture down to ~96
// points at draw time aliases badly; downscaling ahead of time keeps the
// runtime ratio small enough to look sharp.
fn load_icon_texture(ctx: &egui::Context) -> egui::TextureHandle {
    let dynamic_image = image::load_from_memory(DOTTODOTSTUDIO_ICON_BYTES)
        .expect("bundled app icon PNG must decode");
    let rgba_image = dynamic_image.to_rgba8();
    let width = rgba_image.width() as usize;
    let height = rgba_image.height() as usize;
    let pixels = rgba_image.into_raw();

    let color_image = egui::ColorImage::from_rgba_unmultiplied([width, height], &pixels);

    ctx.load_texture("about_dialog_icon", color_image, egui::TextureOptions::default())
}

// Draw the About dialog.
//
// The dialog shows a heading followed by a scrollable Markdown-rendered
// text block built from `ABOUT_TEXT` above. Edit that constant to change
// the displayed content; no other code needs to change.
pub fn show_about_dialog(ctx: &egui::Context, app: &mut DotToDotStudioApp) {
    if !app.about_dialog.open {
        return;
    }

    let mut open = app.about_dialog.open;
    let mut close_after_button = false;

    egui::Window::new("About")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(440.0)
        .default_height(420.0)
        .show(ctx, |ui| {
            // Use a fixed height here, not something derived from
            // `ui.available_height()`. Deriving it from the current frame's
            // available space creates a feedback loop: any tiny layout change
            // (even a hover effect changing a button's rendered size by a
            // pixel) changes the "available" height, which changes this
            // height, which changes the window's content size, which changes
            // next frame's "available" height again. The visible symptom is
            // the window constantly resizing/jittering as the mouse moves.
            // A constant value breaks that loop: the content height is the
            // same every frame regardless of hover state or window size.
            const SCROLL_AREA_HEIGHT: f32 = 300.0;
            const ICON_SIZE: f32 = 96.0;

            let icon_texture = app
                .about_dialog
                .icon_texture
                .get_or_insert_with(|| load_icon_texture(ctx))
                .clone();

            ui.vertical_centered(|ui| {
                ui.add(
                    egui::Image::new((icon_texture.id(), icon_texture.size_vec2()))
                        .max_size(egui::vec2(ICON_SIZE, ICON_SIZE)),
                );
            });
            ui.add_space(8.0);

            ui.heading(egui::RichText::new("About DotToDotStudio").strong());
            ui.separator();
            ui.add_space(8.0);

            egui::ScrollArea::vertical()
                .max_height(SCROLL_AREA_HEIGHT)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    render_markdown(ui, ABOUT_TEXT);
                });

            ui.add_space(8.0);
            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("Close").clicked() {
                    close_after_button = true;
                }
            });
        });

    if close_after_button {
        open = false;
    }

    app.about_dialog.open = open;
}

// -----------------------------------------------------------------------
// Minimal Markdown renderer
// -----------------------------------------------------------------------

/// Render a small Markdown subset (see module docs above) into the given
/// `egui::Ui` as a vertically stacked block of widgets.
pub fn render_markdown(ui: &mut egui::Ui, markdown: &str) {
    for raw_line in markdown.lines() {
        let line = raw_line.trim_start();

        if line.is_empty() {
            ui.add_space(6.0);
            continue;
        }

        if line == "---" || line == "***" {
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);
            continue;
        }

        if let Some(heading_text) = line.strip_prefix("### ") {
            ui.add_space(6.0);
            render_inline_markdown(ui, heading_text, 16.0, true);
            ui.add_space(2.0);
            continue;
        }

        if let Some(heading_text) = line.strip_prefix("## ") {
            ui.add_space(8.0);
            render_inline_markdown(ui, heading_text, 19.0, true);
            ui.add_space(2.0);
            continue;
        }

        if let Some(heading_text) = line.strip_prefix("# ") {
            ui.add_space(10.0);
            render_inline_markdown(ui, heading_text, 24.0, true);
            ui.add_space(2.0);
            continue;
        }

        if let Some(item_text) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label("•");
                render_inline_markdown(ui, item_text, 14.0, false);
            });
            continue;
        }

        render_inline_markdown(ui, line, 14.0, false);
    }
}

/// Render one line of inline-formatted text (`**bold**`, `*italic*`) as a
/// wrapped sequence of labels inside a horizontal-wrapped layout.
///
/// `force_bold` is used by headings so the whole line is bold regardless of
/// whether the source text used `**`.
fn render_inline_markdown(ui: &mut egui::Ui, text: &str, font_size: f32, force_bold: bool) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;

        for (segment_text, bold, italic) in parse_inline_segments(text) {
            if segment_text.is_empty() {
                continue;
            }

            let mut rich_text = egui::RichText::new(segment_text).size(font_size);

            if bold || force_bold {
                rich_text = rich_text.strong();
            }

            if italic {
                rich_text = rich_text.italics();
            }

            ui.label(rich_text);
        }
    });
}

/// Split one line of text into `(text, bold, italic)` segments based on
/// `**bold**` and `*italic*` markers.
///
/// This is a deliberately small, line-local parser: it does not handle
/// nested/overlapping emphasis, links, or code spans, since the About
/// dialog only needs lightweight formatting.
fn parse_inline_segments(text: &str) -> Vec<(String, bool, bool)> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut bold = false;
    let mut italic = false;

    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;

    while index < chars.len() {
        if index + 1 < chars.len() && chars[index] == '*' && chars[index + 1] == '*' {
            if !current.is_empty() {
                segments.push((std::mem::take(&mut current), bold, italic));
            }
            bold = !bold;
            index += 2;
            continue;
        }

        if chars[index] == '*' {
            if !current.is_empty() {
                segments.push((std::mem::take(&mut current), bold, italic));
            }
            italic = !italic;
            index += 1;
            continue;
        }

        current.push(chars[index]);
        index += 1;
    }

    if !current.is_empty() {
        segments.push((current, bold, italic));
    }

    segments
}
