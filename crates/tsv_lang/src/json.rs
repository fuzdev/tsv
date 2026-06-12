// Sizing heuristic for public-AST JSON wire output.

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
