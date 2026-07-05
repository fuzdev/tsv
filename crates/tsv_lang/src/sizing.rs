// Sizing heuristics for allocation pre-sizing — the wire-JSON output buffer
// and the parse-time bump arena.

/// Estimated compact-JSON bytes per source byte for the wire-JSON output.
///
/// Per-file means measured across corpora cluster tightly: TypeScript ~18.5x
/// (zzz, 90 files), Svelte ~17.6x (zzz, 123 files), CSS ~19.8x (prettier css
/// tests, 205 files) — node objects with `start`/`end`/`loc` dominate the
/// wire size regardless of language. 20 slightly over-allocates the typical
/// file so serialization finishes without reallocating; high-ratio outliers
/// (TS max ~30x) pay one doubling.
const JSON_BYTES_PER_SOURCE_BYTE: usize = 20;

/// Pre-size estimate for a document's compact wire-JSON output.
///
/// Used by each language's wire-JSON writer (`convert_ast_json_bytes`) to
/// allocate the `JsonWriter` buffer up front instead of growing it through
/// `Vec`'s default doubling (the JSON wire form runs ~20x the source length,
/// so default growth pays many large reallocs). The floor covers tiny sources
/// whose output is mostly fixed envelope.
pub fn estimated_json_capacity(source_len: usize) -> usize {
    source_len
        .saturating_mul(JSON_BYTES_PER_SOURCE_BYTE)
        .max(128)
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
