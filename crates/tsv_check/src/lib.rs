//! `tsv_check` — experimental TypeScript type-checking for tsv.
//!
//! The checker consumes the `tsv_ts` internal AST and produces TypeScript
//! diagnostics. It is a **consumer crate** of `tsv_ts`'s concrete types (the
//! `tsv_svelte` precedent) — no `Language` trait, no registry, no dyn dispatch.
//!
//! ## Zero-cost invariant
//!
//! `tsv_check` is referenced only by the dev harness (`tsv_debug`); no
//! `tsv_ffi`/`tsv_wasm`/`tsv_napi`/`tsv_cli` format-or-parse artifact links it.
//! That exclusion is a crate boundary, not a cfg — stronger than a feature gate.
//!
//! ## Pipeline
//!
//! ```text
//! source units (+ arena)
//!   -> parse (goal rule: Module first, Script retry)   [program]
//!   -> lower + bind (one fused pre-order walk per file) [binder]
//!   -> check (no-op skeleton)                           [program]
//!   -> sort + dedup (tsgo's comparer)                   [diag]
//!   -> owned diagnostics
//! ```
//!
//! [`check_program`] is the single entry point. The caller owns the parse arena
//! (`&bumpalo::Bump`, the tsv_ts caller-owns-arena contract) and the returned
//! [`CheckResult`] borrows nothing from it.
//!
//! ## Modules
//!
//! - [`ids`] — `NodeId` / `FileId` dense-integer identities.
//! - [`diag`] — the `Diagnostic` shape and the canonical sort/dedup kernel.
//! - `hash` (private) — the crate's Fx-style hasher and `FxHashMap`/`FxHashSet`.
//! - `binder` (private) — the fused lower+bind pre-order walk.
//! - `program` (private) — pipeline assembly and the parse-error short-circuit.
//!
//! ## Reference-anchor convention
//!
//! Semantic-core functions carry a `// tsgo: <file> <fn>` pointer to their
//! typescript-go counterpart (the lexer's ECMA-262-citation convention applied
//! to the checker), so the port stays diffable against the oracle.

mod binder;
mod hash;
mod program;

pub mod diag;
pub mod ids;

pub use binder::{
    bind_file, module_ness, BoundFile, FileFacts, ModuleNess, NodeKind,
};
pub use diag::{Category, Diagnostic};
pub use ids::{FileId, NodeId};
pub use program::{
    check_program, CheckResult, FileReport, ParseReport, ParsedFacts, SourceUnit,
};

// Re-exported so consumers can name the parse goal a `ParsedFacts` reports
// without a separate `tsv_ts` import.
pub use tsv_ts::Goal;
