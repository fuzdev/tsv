//! The `tsv` command-line tool — `tsv format`, `tsv parse` and `tsv strip` for
//! TypeScript/JS, CSS and Svelte; part of [tsv](https://github.com/fuzdev/tsv).
//!
//! Install with `cargo install --locked tsv_cli` (the binary is `tsv`), or take the prebuilt
//! binary from the `@fuzdev/tsv` npm package or a
//! [GitHub Release](https://github.com/fuzdev/tsv/releases) — `cargo binstall tsv_cli` fetches
//! the Release's binary for the installing target, building from source where none ships. A
//! build from crates.io does not get the profile the prebuilt binaries ship with (a
//! workspace's profiles do not travel with a published crate), so it has no LTO and unwinds
//! on a panic: it runs slower, and an internal panic while formatting files is reported as
//! that file's error rather than aborting the run. Usage: `tsv --help`, and the
//! [README](https://github.com/fuzdev/tsv#readme).
//!
//! **The library target is the CLI's internals**, shared with tsv's dev tooling (its input
//! handling, its sized-stack threads and the `--pretty` re-indent). It is hidden here and
//! makes no stability promise; for formatting and parsing from Rust use [`tsv_ts`],
//! [`tsv_css`] and [`tsv_svelte`].

// Every byte the CLI emits goes through `cli::out` (a closed reader is not a failure;
// `println!` would abort on it), and the rule is enforced here rather than remembered:
// a print macro anywhere in the crate fails clippy (`deno task lint`, part of `check`). `out.rs` itself writes
// through `io::Write`, so it needs no exemption.
#![deny(clippy::print_stdout, clippy::print_stderr)]
#![warn(missing_docs)]

// Export CLI infrastructure for use by other binaries (e.g., tsv_debug)
#[doc(hidden)]
// A hidden module renders nowhere public, so its links to private items are maintainer
// navigation (`docs:audit` documents private items); only the public-only docs.rs build
// fires the lint, which `docs:audit:docsrs` denies everywhere else.
#[expect(rustdoc::private_intra_doc_links)]
pub mod cli;
#[doc(hidden)]
pub mod json_utils;
