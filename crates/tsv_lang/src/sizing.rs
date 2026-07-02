// Sizing heuristics for allocation pre-sizing — the public-AST JSON wire buffer
// and the parse-time bump arena.

/// Estimated compact-JSON bytes per source byte for public-AST output.
///
/// Per-file means measured across corpora cluster tightly: TypeScript ~18.5x
/// (zzz, 90 files), Svelte ~17.6x (zzz, 123 files), CSS ~19.8x (prettier css
/// tests, 205 files) — node objects with `start`/`end`/`loc` dominate the
/// wire size regardless of language. 20 slightly over-allocates the typical
/// file so serialization finishes without reallocating; high-ratio outliers
/// (TS max ~30x) pay one doubling.
const JSON_BYTES_PER_SOURCE_BYTE: usize = 20;

/// Pre-size estimate for serializing a public AST to compact JSON.
///
/// Used by each language's `convert_ast_json_string` to allocate the output
/// buffer up front instead of growing it through `serde_json`'s default
/// doubling (the JSON wire form runs ~20x the source length, so default
/// growth pays many large reallocs). The floor covers tiny sources whose
/// output is mostly fixed envelope.
pub fn estimated_json_capacity(source_len: usize) -> usize {
    source_len
        .saturating_mul(JSON_BYTES_PER_SOURCE_BYTE)
        .max(128)
}

/// Estimated bump-arena bytes per source byte for the internal AST.
///
/// The bump-allocated AST (nodes inline-by-value, child slices, arena-copied
/// strings) runs on the order of this many bytes per source byte. Sizing the
/// `Bump` up front turns the handful of chunk-doubling `malloc`s a fresh
/// `Bump::new()` would pay (512 B first chunk, then doubling) into one
/// allocation — a small win on the WASM-format wall (dlmalloc) and native.
/// This does not change the allocation *count* materially; it trims the
/// chunk-grow tail.
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

/// Estimated source bytes per distinct interned symbol.
///
/// Every parse interns each distinct identifier (and property/type name) once
/// into a per-file `string-interner`. Measured across corpora (2108 files,
/// zzz/fuz_app/svelte/kit/svelte-docinfo), the per-file distribution of source
/// bytes per distinct symbol has median ~84 (25th percentile ~53); dividing by
/// 32 sizes the interner's dedup map + span vec a little above the actual symbol
/// count for ~95% of files, so the from-empty doubling reallocs (the map, the
/// span vec, and the backend's `cap * 5`-byte string buffer) collapse to one
/// up-front allocation each. Over-provisioning is ~2.6x the map on the median
/// file — modest — and the interner is per-file (no cross-file reuse: it has no
/// `reset()`), so it is freed promptly.
const SOURCE_BYTES_PER_INTERNED_SYMBOL: usize = 32;

/// Pre-size estimate (in distinct symbols) for the per-file string interner.
///
/// Feed to `DefaultStringInterner::with_capacity(...)` at each parse entry point
/// that creates its own interner (`tsv_ts` standalone, `tsv_svelte`; embedded TS
/// shares the host document's interner and constructs none). No floor: a tiny
/// source maps to a capacity of 0, and `with_capacity(0)` allocates nothing, so
/// a one-line input keeps its zero-allocation start.
pub fn estimated_interner_capacity(source_len: usize) -> usize {
    source_len / SOURCE_BYTES_PER_INTERNED_SYMBOL
}
