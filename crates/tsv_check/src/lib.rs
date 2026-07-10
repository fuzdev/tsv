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
//!   -> merge (single-threaded global-scope fold)        [merge]
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
//! - [`merge`] — the single-threaded global-scope fold (cross-declaration-space
//!   conflicts, `globalThis`/`undefined`, module augmentations).
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
pub mod merge;

pub use binder::{BoundFile, FileFacts, ModuleNess, NodeKind, bind_file, module_ness};
pub use diag::{Category, Diagnostic};
pub use ids::{FileId, NodeId};
pub use merge::{LibBase, LibFile};
pub use program::{
    BoundProgram, CheckResult, FileReport, ParseReport, ParsedFacts, SourceUnit, bind_lib,
    bind_program, check_bound, check_program, check_program_with_lib,
};

// Re-exported so consumers can name the parse goal a `ParsedFacts` reports
// without a separate `tsv_ts` import.
pub use tsv_ts::Goal;
