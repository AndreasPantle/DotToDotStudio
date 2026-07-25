// This file is the module entry point for the `ui` folder.
//
// In Rust, a folder can become a module if it contains a `mod.rs` file.
// That allows us to organize related source files into a submodule tree.
//
// Here, `mod.rs` tells Rust that the `ui` module contains:
// - `project_panel` from `project_panel.rs`
// - `sequences_panel` from `sequences_panel.rs`
//
// Because of these `pub mod ...` lines, other parts of the program can use:
//
// - crate::ui::project_panel
// - crate::ui::sequences_panel
// - crate::ui::export
//
// In short:
// `mod.rs` wires the files in this folder together into one named module.
pub mod about;
pub mod export;
pub mod project_panel;
pub mod sequences_panel;
