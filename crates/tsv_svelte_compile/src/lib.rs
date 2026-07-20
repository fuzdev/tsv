//! Svelte-to-JS compiler and JavaScript canonicalizer.
//!
//! This crate compiles Svelte components to JavaScript, pinned to Svelte's own
//! `compile()` as the correctness oracle. Parity is judged not on raw output
//! bytes but on the *canonical reprint* of both sides: [`canonicalize_js`] parses
//! JavaScript and reprints it with newline-derived authoring intent erased, so a
//! diff between two canonical forms reflects only a real code difference, never
//! incidental whitespace.
//!
//! [`compile`] generates server (SSR) output by constructing a synthetic
//! `tsv_ts` AST over the hybrid appendix buffer (`build`) and printing it
//! through `tsv_ts::format_canonical` — generated JS is canonical-form by
//! construction, so the parity comparison verifies rather than transforms it.
//! The server transform (`transform_server`) covers a deliberately small
//! language subset today; unhandled shapes surface as
//! [`CompileError::Unsupported`] rather than guessed output.
//!
//! # The walks and their oracle phases
//!
//! The compile path makes several separate passes over the same document, and the
//! organizing principle is **one walk per oracle phase**. They are not separate
//! because separation is tidy, and not merely because their descent threads
//! per-node scope state — they are separate because *the oracle's phase boundaries
//! carry ordering semantics that are load-bearing for output parity*. A walk that
//! spans two oracle phases can no longer be checked against either.
//!
//! The counterexample is concrete and was hit rather than theorized: the two
//! `{#each}` generated names are minted in different oracle phases and therefore in
//! different orders — `each_array` in the transform (pre-order), `$$index` in the
//! scope-creation pass (post-order, over SSR-dropped regions too). Sharing one walk
//! and one counter mis-numbers every document where one `{#each}` contains another
//! or one sits in a `{:catch}`. See `blocks::assign_each_index_names`.
//!
//! | Walk | Oracle phase | Descent |
//! | --- | --- | --- |
//! | `validate::validate_document` | phase 1 parse-time rules + whole-component phase-2 validations | `attr_refs::each_child_fragment` |
//! | `script_ts_gate::refuse_template_typescript` | phase 1 (the TypeScript grammar gate) | `attr_refs::each_template_item` |
//! | `element_census::build_census` | phase 2, CSS pass (`css-prune.js`) | own exhaustive match |
//! | `snippet::analyze_snippets` | phase 2 (`can_hoist_snippet`) | seam + own scope-threaded match |
//! | `needs_context::analyze_component` | phase 2 accumulation + `validate_assignment` | own scope-threaded match |
//! | `blocks::assign_each_index_names` | the scope-creation pass (`phases/scope.js`) | `attr_refs::each_child_fragment` |
//!
//! Two consequences worth stating, because each has been a bug:
//!
//! - **Two walks being mechanically mergeable is not a reason to merge them.** Some
//!   pairs share a seam, run unconditionally, and thread no conflicting state — and
//!   still port different phases, so merging them trades a checkable correspondence
//!   for one traversal. Every fragment walk together is ~1% of compile wall; the
//!   traversal was never the cost.
//! - **Riding the shared seam is the default, and opting out needs a reason in the
//!   code.** `each_child_fragment` exists so a new `FragmentNode` variant fails
//!   compilation rather than silently going undescended. A walk that hand-rolls its
//!   descent must carry an exhaustive match instead (`element_census` does, and says
//!   why); a catch-all is admissible only where an unhandled variant fails in the
//!   *safe* direction, and must say which direction that is.

mod analyze;
mod attr_refs;
mod attribute;
mod attribute_bind;
mod attribute_class_style;
mod blocks;
mod body_builder;
mod build;
mod census;
mod component;
mod css_scope;
mod dropped;
mod element;
mod element_census;
mod erase;

pub use census::{census, census_detected_buckets};

/// The forward half of an erased TypeScript region's comment-refusal window (see
/// `erase`). Exported so the `erase_comment_census` diagnostic sizes the rule the
/// compiler actually enforces, rather than a hand-rolled copy of it.
pub use erase::next_token_pos;
mod fragment;
mod html_tree;
mod namespace;
mod needs_context;
mod parity;
mod refusal;
mod rune_guard;
mod script_bindings;
mod script_collision;
mod script_comments;
mod script_decls;
mod script_props;
mod script_rewrite;
mod script_ts_gate;
mod snippet;
mod snippet_emit;
mod special_element_kind;
mod store_rewrite;
mod template_value;
mod text_class;
mod transform_server;
mod validate;

pub use parity::{Parity, compare_canonical};
pub use refusal::Refusal;

use tsv_ts::Goal;

/// Which runtime the compiler targets.
///
/// Mirrors Svelte's `generate` option. Defaults to [`Generate::Server`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Generate {
    /// Server-side rendering output (the default).
    #[default]
    Server,
    /// Client-side output.
    Client,
}

/// Options controlling a [`compile`] run.
///
/// Defaults to server generation, non-development output — matching the
/// deterministic oracle configuration used for parity comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CompileOptions {
    /// Target runtime.
    pub generate: Generate,
    /// Development-mode output (extra runtime checks / metadata).
    pub dev: bool,
}

/// A diagnostic emitted during compilation.
///
/// Minimal for this slice — a stable code and a human-readable message. It grows
/// as the compiler produces real warnings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileWarning {
    /// Stable warning code (e.g. an `a11y-*` identifier).
    pub code: String,
    /// Human-readable description.
    pub message: String,
}

/// The product of a successful [`compile`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileOutput {
    /// The generated JavaScript module.
    pub js: String,
    /// The extracted, scoped CSS, if the component had a `<style>`.
    pub css: Option<String>,
    /// Warnings produced during compilation.
    pub warnings: Vec<CompileWarning>,
}

/// An error from [`compile`].
#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    /// The component failed to parse (a real syntax error in the `.svelte`
    /// source, its `<script>`, or its `<style>`).
    #[error("failed to parse Svelte component: {0}")]
    Parse(#[from] tsv_lang::ParseError),
    /// The component parsed, but uses a shape the compiler does not cover yet.
    /// Always a clear refusal — never guessed output. The [`Refusal`] carries
    /// both the human-readable message and a stable corpus bucket key.
    #[error("not yet supported by the Svelte compiler: {0}")]
    Unsupported(Refusal),
    /// The generated JS failed to reparse — a divergent shape slipped every
    /// guard and the transform emitted invalid JavaScript. Always a compiler
    /// bug; surfaced loudly instead of returning the corrupt module (the same
    /// contract as [`CanonicalizeError::CorruptOutput`]).
    #[error("generated JS failed to reparse (compiler bug): {0}")]
    CorruptOutput(tsv_lang::ParseError),
    /// A TypeScript-only node survived type erasure into the emitted program —
    /// the erase pass missed a case. Always a compiler bug, and one
    /// [`Self::CorruptOutput`] **cannot** catch: a surviving annotation still
    /// parses (tsv's parser is TypeScript-permissive), so the reparse check
    /// passes while the output carries TypeScript verbatim. Surfaced loudly
    /// instead of returning the mis-compiled module.
    #[error("TypeScript survived erasure into the generated JS (compiler bug) at {0:?}")]
    TypeErasureLeak(tsv_lang::Span),
    /// A generated name the transform assigns upfront was missing from its table
    /// at emission — the upfront walk lost a fragment the emission path reached.
    /// Always a compiler bug. Surfaced loudly rather than falling back to the
    /// unsuffixed name: a guessed generated name is *silently correct* whenever
    /// the document happens to need only the first one, so the fallback would
    /// hide the very table-population bug it is standing in for.
    #[error("a generated name was not assigned upfront (compiler bug) at {0:?}")]
    GeneratedNameMissing(tsv_lang::Span),
}

/// An error from [`canonicalize_js`].
#[derive(Debug, thiserror::Error)]
pub enum CanonicalizeError {
    /// The input did not parse as a JavaScript/TypeScript module.
    #[error("failed to parse JavaScript for canonicalization: {0}")]
    Parse(#[from] tsv_lang::ParseError),
    /// The canonical reprint itself failed to reparse — the canonicalizer
    /// corrupted the program (e.g. content trailed onto a `//` comment's line).
    /// Always a canonicalizer bug; surfaced loudly instead of returning the
    /// corrupt string.
    #[error("canonical output failed to reparse (canonicalizer bug): {0}")]
    CorruptOutput(tsv_lang::ParseError),
}

/// Compile a Svelte component to JavaScript.
///
/// Parses `source` (surfacing any real parse error as [`CompileError::Parse`])
/// and runs the server transform. The generated JS is already in canonical form
/// (it prints through `tsv_ts::format_canonical`), so
/// `canonicalize_js(output.js)` is a fixed point. Client generation and dev
/// mode are not implemented yet ([`CompileError::Unsupported`]).
///
/// The output is self-validated by reparse before it is returned — generated JS
/// the parser rejects surfaces as [`CompileError::CorruptOutput`] instead of a
/// silently invalid module. Always on: the reparse costs ~13% of the compile
/// itself (microseconds per component), cheap insurance for a dev-stage
/// compiler whose refusal contract depends on never shipping guessed output.
pub fn compile(source: &str, options: &CompileOptions) -> Result<CompileOutput, CompileError> {
    if options.generate == Generate::Client {
        return Err(CompileError::Unsupported(Refusal::ClientGeneration));
    }
    if options.dev {
        return Err(CompileError::Unsupported(Refusal::DevMode));
    }
    let arena = bumpalo::Bump::new();
    let root = tsv_svelte::parse(source, &arena)?;
    let output = transform_server::compile_server(&root, source, &arena)?;
    validate_output_js(&output.js)?;
    Ok(output)
}

/// The self-validation seam: assert `js` reparses as a strict module.
///
/// Split from [`compile`] so the corrupt-output path is unit-testable without
/// weakening the public API (no test-only hooks on `compile` itself).
fn validate_output_js(js: &str) -> Result<(), CompileError> {
    let arena = bumpalo::Bump::new();
    match tsv_ts::parse_with_goal(js, Goal::Module, &arena) {
        Ok(_) => Ok(()),
        Err(err) => Err(CompileError::CorruptOutput(err)),
    }
}

/// Reprint JavaScript with newline-derived authoring intent erased — the
/// canonical form used for parity comparison.
///
/// Parses `source` as a strict module ([`Goal::Module`]) and reprints it via
/// `tsv_ts`'s canonical formatter, which:
///
/// - **drops blank lines** between statements,
/// - **turns off expansion heuristics** — a construct that fits the print width
///   collapses to one line regardless of whether the source had a newline after
///   its opening delimiter; it breaks only when width forces it,
/// - **preserves comments** (content and relative order) — placement is
///   normalized deterministically (an own-line comment may become a trailing
///   comment of the preceding node), never dropped or merged.
///
/// The result is idempotent: canonicalizing an already-canonical string
/// reproduces it. Because both an oracle's output and the compiler's output pass
/// through the same normalization, a byte difference between their canonical
/// forms is a genuine code difference.
///
/// One caveat on that last claim, for callers outside this crate: `format_canonical`
/// does **not** erase a mapped type's source multi-line-ness (a deliberate residual —
/// see its docs), so two sources differing only in how a mapped type was authored do
/// canonicalize differently. It cannot bite the compiler-parity use this exists for —
/// compiled JS carries no TypeScript types — but it does mean "canonical form" is not
/// unconditionally authoring-independent over arbitrary TS.
///
/// The output is self-validated by reparse before it is returned — a reprint the
/// parser rejects (canonicalizer corruption) surfaces as
/// [`CanonicalizeError::CorruptOutput`] instead of a silently broken string.
/// This is a comparison harness, so the extra parse is cheap insurance.
pub fn canonicalize_js(source: &str) -> Result<String, CanonicalizeError> {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse_with_goal(source, Goal::Module, &arena)?;
    let output = tsv_ts::format_canonical(&program, source);
    let check_arena = bumpalo::Bump::new();
    if let Err(err) = tsv_ts::parse_with_goal(&output, Goal::Module, &check_arena) {
        return Err(CanonicalizeError::CorruptOutput(err));
    }
    Ok(output)
}

#[cfg(test)]
mod tests;
