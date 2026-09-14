//! Integration tests for CLI commands — each test spawns the `tsv` binary.
//!
//! The crate root of the split suite; every test lives in one of the aspect modules.
//!
//! - `common` — the shared helpers: the binary builder, the spawn wrappers, the temp-tree
//!   guard, the file-mode probe and guard, and the source constants.
//! - `top_level` — argv and dispatch: unknown and missing commands, `--version`, the help text.
//! - `parse` — `tsv parse` on every input arm, its flags and its refusals.
//! - `format_single` — `tsv format --content` / `--stdin`: the single-input routes.
//! - `format_paths` — `tsv format <path>`: discovery, `--check`, `--list`, `--jobs`, per-file
//!   failures, and the filesystem shapes the walk survives.
//! - `ignore_files` — ignore-file scoping, precedence, shadowing and the warnings they emit.
//! - `pipes` — a reader that leaves, drains late, or never blocks (unix only).

mod common;
mod format_paths;
mod format_single;
mod ignore_files;
mod parse;
#[cfg(unix)]
mod pipes;
mod top_level;
