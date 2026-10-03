// Sizing heuristics for allocation pre-sizing — the wire-JSON output buffer,
// the parse-time bump arena, and the wire writer's line-start table.

use crate::location::Wire;

/// Estimated compact-JSON bytes per source byte for the span-only wire — the one
/// every binding emits.
///
/// Per-file ratios across the `../corpora` snapshot cluster by wire, not by
/// language: span-only, TypeScript and Svelte both have a median near 9x and a p90
/// near 12x (CSS lower by bytes, noisier on tiny files). 12 sizes ~90% of files to
/// finish without reallocating, where the loc wire's constant would reserve over
/// twice what the median file writes — memory a WASM instance's linear memory keeps
/// as its high-water mark. The outliers pay one doubling.
const SPAN_JSON_BYTES_PER_SOURCE_BYTE: usize = 12;

/// Estimated compact-JSON bytes per source byte for the loc-bearing wire
/// (`tsv parse --locations`): the per-node `loc` object roughly doubles the wire
/// (median ~17x TypeScript, ~19x Svelte over the same snapshot), so 20 covers the
/// typical file and the high-ratio outliers pay one doubling.
#[cfg(feature = "locations")]
const LOC_JSON_BYTES_PER_SOURCE_BYTE: usize = 20;

/// Pre-size estimate for a document's compact wire-JSON output on `wire`.
///
/// Used by each language's wire-JSON writer to allocate the `JsonWriter` buffer
/// up front instead of growing it through `Vec`'s default doubling (the wire runs
/// several times the source length, so default growth pays many large reallocs).
/// The floor covers tiny sources whose output is mostly fixed envelope.
pub fn estimated_json_capacity(source_len: usize, wire: Wire) -> usize {
    let per_byte = match wire {
        Wire::Span => SPAN_JSON_BYTES_PER_SOURCE_BYTE,
        #[cfg(feature = "locations")]
        Wire::Loc => LOC_JSON_BYTES_PER_SOURCE_BYTE,
    };
    source_len.saturating_mul(per_byte).max(128)
}

/// Bump-arena pre-size floor, in bytes per source byte, for the internal AST.
///
/// This is a deliberate *partial* pre-size, not the AST's true footprint — the
/// bump-allocated AST (nodes inline-by-value, child slices, arena-copied
/// strings) runs to roughly 30–50 bytes per source byte in practice. Sizing the
/// `Bump` to 16x up front folds the first several chunk-doubling `malloc`s a
/// fresh `Bump::new()` would pay (512 B first chunk, then doubling) into one
/// allocation — a small win on the WASM-format wall (dlmalloc) and native.
/// Provisioning all the way to true demand buys nothing measurable: it does not
/// change the allocation *count*, only trims the chunk-grow tail, and the batch
/// drivers reuse one arena across files (`Bump::reset()`), so cold-start sizing
/// is moot after the first file.
const AST_ARENA_BYTES_PER_SOURCE_BYTE: usize = 16;

/// Pre-size estimate (in bytes) for the parse-time bump arena that owns the
/// internal AST, given the source length.
///
/// Feed to `bumpalo::Bump::with_capacity(...)` at each parse entry point
/// (caller-owns-`Bump`). The floor covers tiny sources whose AST is mostly
/// fixed-size envelope, so a one-line input still gets one chunk, not several.
pub fn estimated_ast_arena_capacity(source_len: usize) -> usize {
    source_len
        .saturating_mul(AST_ARENA_BYTES_PER_SOURCE_BYTE)
        .max(512)
}

/// Source bytes per line assumed when pre-sizing a line-start table — a
/// deliberate *under*-estimate of the line length, so the table rarely grows.
///
/// Real source runs ~30–35 bytes per line (TypeScript ~35, Svelte ~31 across
/// the benchmark corpora), so at 16 only a file of unusually short lines
/// outgrows the reservation and pays one doubling. Growing from the seeded
/// single entry instead cost ~5 reallocations a file, each a copy of the
/// table so far. The reservation is one 4-byte entry per 16 source bytes — a
/// quarter of the source length in bytes, of which a typical file fills about
/// half — short-lived (the table lives for one wire emission), and small
/// beside the ~20x wire buffer it sits next to.
const SOURCE_BYTES_PER_LINE: usize = 16;

/// Pre-size estimate (in entries) for a document's line-start table, given
/// the source length. The `+ 1` is line 1's start, which every table holds.
///
/// Used by `location`'s line-start builders — every constructor that scans
/// a source for its lines seeds its table at this capacity rather than at
/// the single entry it begins with.
pub(crate) fn estimated_line_starts_capacity(source_len: usize) -> usize {
    source_len / SOURCE_BYTES_PER_LINE + 1
}
