// Conversion from the internal AST to the public wire JSON.
//
// The writer (`write/`) emits the compact wire JSON directly from the internal
// AST in one walk, fusing byte→UTF-16 offset translation into the walk (final
// char-space positions emitted directly via `LocationMapper`). It is the sole
// emission path; `convert_ast_json_bytes`/`_string` in `lib.rs` call it, and
// `convert_ast_json` parses its bytes back into a `Value`.

use std::borrow::Cow;
use string_interner::{DefaultStringInterner, DefaultSymbol};
use tsv_lang::{ByteToCharMap, InfallibleResolve, LocationTracker, Span};

/// Schema choice for public-AST serialization.
///
/// Svelte's parser (for non-lang="ts" `<script>`) and acorn-typescript differ in
/// which fields they emit on import/export nodes. This enum is threaded through
/// conversion so each call site produces the correct JSON shape.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Schema {
    /// acorn-typescript schema: always emit `importKind`/`exportKind`,
    /// omit empty `attributes`.
    #[default]
    Acorn,
    /// Svelte non-lang="ts" `<script>` schema: omit `importKind`/`exportKind`
    /// when the value is `"value"`, always emit `attributes` on
    /// `ImportDeclaration`/`ExportNamedDeclaration`/`ExportAllDeclaration`.
    SvelteScript,
}

impl Schema {
    #[inline]
    pub(crate) fn is_svelte_script(self) -> bool {
        matches!(self, Schema::SvelteScript)
    }
}

// The writer — the sole emission mode.
mod write;

pub use write::{
    WriterComments, write_expression_embedded, write_expression_embedded_with_comments,
    write_identifier_expression_with_character,
    write_identifier_expression_with_character_and_comments, write_pattern_embedded,
    write_program_embedded, write_program_json, write_variable_declaration_embedded,
    write_variable_declaration_embedded_with_comments,
};

/// Translate a column from byte-based to char-based, preserving any prior adjustment (e.g., +1)
///
/// Computes the expected byte-based column from the byte offset, then the char-based column,
/// and preserves the delta between the existing column value and the expected byte column.
/// This ensures adjustments like Svelte's read-pattern `+1` survive translation.
///
/// `pub` so the `tsv_svelte` writer reuses it for the `<script>` `Program`'s
/// tag-line column positions (which it emits in char space directly).
#[allow(clippy::cast_sign_loss)]
pub fn translate_column(
    byte_offset: u32,
    existing_column: u64,
    map: &ByteToCharMap,
    tracker: &LocationTracker,
) -> u64 {
    let line_start = tracker.line_start_byte(byte_offset as usize);
    let expected_byte_col = (byte_offset as usize).saturating_sub(line_start);
    let char_col = map.byte_to_char(byte_offset) - map.byte_to_char(line_start as u32);
    // Preserve any delta (e.g., +1 from Svelte's read-pattern column shift)
    let delta = (existing_column as i64) - (expected_byte_col as i64);
    ((char_col as i64) + delta) as u64
}

/// Convert non-decimal BigInt values to decimal string (matching acorn behavior).
/// Strips numeric separators (`_`) and converts radix prefixes:
/// `0xff` → `255`, `0o77` → `63`, `0b1010` → `10`, `1_000` → `1000`
pub(super) fn bigint_to_decimal(val: &str) -> String {
    // Strip numeric separators first (acorn normalizes them away)
    let stripped: String;
    let val = if val.contains('_') {
        stripped = val.replace('_', "");
        &stripped
    } else {
        val
    };
    if let Some(hex) = val.strip_prefix("0x").or_else(|| val.strip_prefix("0X")) {
        u128::from_str_radix(hex, 16).map_or_else(|_| val.to_string(), |n| n.to_string())
    } else if let Some(oct) = val.strip_prefix("0o").or_else(|| val.strip_prefix("0O")) {
        u128::from_str_radix(oct, 8).map_or_else(|_| val.to_string(), |n| n.to_string())
    } else if let Some(bin) = val.strip_prefix("0b").or_else(|| val.strip_prefix("0B")) {
        u128::from_str_radix(bin, 2).map_or_else(|_| val.to_string(), |n| n.to_string())
    } else {
        val.to_string()
    }
}

/// Borrow an interned name from `source` when the span's source slice is exactly
/// the name (the overwhelming case — a plain identifier reference); own the
/// resolved name otherwise.
///
/// The slice can't be borrowed blindly: a *binding* identifier's `span` covers
/// the whole binding, type annotation included (`x: T`), not just the name, and
/// an escaped identifier (`\u{78}`) decodes to a name distinct from its raw
/// source. Resolving the symbol is a cheap interner lookup (no allocation); the
/// `String` allocation — the thing this avoids — only happens on the owned
/// branch, where the slice and the name genuinely differ.
///
/// `pub` so hosts with interned names in their own wire JSON apply the same
/// guard (`tsv_svelte`'s writer uses it for element/attribute names). The TS
/// writer's own `write_name` inlines the same test, emitting either branch
/// directly without a `Cow`.
pub fn name_cow<'src>(
    span: Span,
    source: &'src str,
    sym: DefaultSymbol,
    interner: &DefaultStringInterner,
) -> Cow<'src, str> {
    let resolved = interner.resolve_infallible(sym);
    let raw = span.extract(source);
    if raw == resolved {
        Cow::Borrowed(raw)
    } else {
        Cow::Owned(resolved.to_string())
    }
}
