//! The `tsv` command-line tool's internals. The binary is the product: this library exists
//! for the workspace's own tools (`tsv_debug` drives the CLI's formatting and `--pretty`
//! routes through it) and makes no stability promise — everything in it is hidden.

// Every byte the CLI emits goes through `cli::out` (a closed reader is not a failure;
// `println!` would abort on it), and the rule is enforced here rather than remembered:
// a print macro anywhere in the crate fails clippy (`deno task lint`, part of `check`). `out.rs` itself writes
// through `io::Write`, so it needs no exemption.
#![deny(clippy::print_stdout, clippy::print_stderr)]

// Export CLI infrastructure for use by other binaries (e.g., tsv_debug)
#[doc(hidden)]
pub mod cli;
#[doc(hidden)]
pub mod json_utils;
