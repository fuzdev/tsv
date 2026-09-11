// Every byte the CLI emits goes through `cli::out` (a closed reader is not a failure;
// `println!` would abort on it), and the rule is enforced here rather than remembered:
// a print macro anywhere in the crate is a compile error. `out.rs` itself writes
// through `io::Write`, so it needs no exemption.
#![deny(clippy::print_stdout, clippy::print_stderr)]

// Export CLI infrastructure for use by other binaries (e.g., tsv_debug)
pub mod cli;
pub mod json_utils;
