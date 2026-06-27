// Conversion from internal AST to public AST

use super::internal;
use super::public;
use tsv_lang::{LocationTracker, Span};

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

// Submodules
mod control_flow;
mod declarations;
mod expressions;
mod functions;
mod modules;
mod patterns;
mod statements;
mod translate_typed;
mod types;

pub use translate_typed::translate_byte_to_char_offsets_typed;

// Re-export conversion functions (pub(in crate::ast) for internal use)
pub(in crate::ast) use control_flow::*;
pub(in crate::ast) use declarations::*;
pub(in crate::ast) use functions::*;
pub(in crate::ast) use modules::*;
pub(in crate::ast) use patterns::*;
pub(in crate::ast) use statements::*;
pub(in crate::ast) use types::*;

// Public API exports
pub use expressions::convert_expression;
pub use statements::convert_variable_declaration;

/// Translate all byte-based positions in a JSON AST to UTF-16 code-unit positions
///
/// JS (acorn/Svelte) uses UTF-16 code-unit offsets (JS string indices),
/// while Rust strings are byte-indexed. This function post-processes a serialized
/// JSON AST to convert all `start`, `end`, `loc.*.column`, `character`, and
/// `name_loc` positions from byte offsets to UTF-16 code-unit offsets.
///
/// For ASCII-only sources, this is a no-op (byte == code-unit offset).
pub fn translate_byte_to_char_offsets(
    value: &mut serde_json::Value,
    map: &tsv_lang::ByteToCharMap,
    tracker: &LocationTracker,
) {
    if !map.has_multibyte() {
        return;
    }
    translate_positions_recursive(value, map, tracker);
}

/// Translate a column from byte-based to char-based, preserving any prior adjustment (e.g., +1)
///
/// Computes the expected byte-based column from the byte offset, then the char-based column,
/// and preserves the delta between the existing column value and the expected byte column.
/// This ensures adjustments like `adjust_read_pattern_columns` (+1) survive translation.
#[allow(clippy::cast_sign_loss)]
fn translate_column(
    byte_offset: u32,
    existing_column: u64,
    map: &tsv_lang::ByteToCharMap,
    tracker: &LocationTracker,
) -> u64 {
    let line_start = tracker.line_start_byte(byte_offset as usize);
    let expected_byte_col = (byte_offset as usize).saturating_sub(line_start);
    let char_col = map.byte_to_char(byte_offset) - map.byte_to_char(line_start as u32);
    // Preserve any delta (e.g., +1 from adjust_read_pattern_columns)
    let delta = (existing_column as i64) - (expected_byte_col as i64);
    ((char_col as i64) + delta) as u64
}

/// Translate a single loc/name_loc position entry (column + optional character)
fn translate_loc_position(
    pos: &mut serde_json::Map<String, serde_json::Value>,
    byte_offset: u32,
    map: &tsv_lang::ByteToCharMap,
    tracker: &LocationTracker,
) {
    if let Some(existing_col) = pos.get("column").and_then(serde_json::Value::as_u64) {
        let char_col = translate_column(byte_offset, existing_col, map, tracker);
        pos.insert(
            "column".to_string(),
            serde_json::Value::Number(char_col.into()),
        );
    }
    if pos.contains_key("character") {
        pos.insert(
            "character".to_string(),
            serde_json::Value::Number(map.byte_to_char(byte_offset).into()),
        );
    }
}

fn translate_positions_recursive(
    value: &mut serde_json::Value,
    map: &tsv_lang::ByteToCharMap,
    tracker: &LocationTracker,
) {
    match value {
        serde_json::Value::Object(obj) => {
            // Get the original byte-based start/end before we modify them.
            // We need these to compute character-based columns for loc.
            let orig_start = obj
                .get("start")
                .and_then(serde_json::Value::as_u64)
                .map(|v| v as u32);
            let orig_end = obj
                .get("end")
                .and_then(serde_json::Value::as_u64)
                .map(|v| v as u32);

            // Translate start/end byte offsets to character offsets
            if let Some(start_byte) = orig_start {
                obj.insert(
                    "start".to_string(),
                    serde_json::Value::Number(map.byte_to_char(start_byte).into()),
                );
            }
            if let Some(end_byte) = orig_end {
                obj.insert(
                    "end".to_string(),
                    serde_json::Value::Number(map.byte_to_char(end_byte).into()),
                );
            }

            // Translate loc.start.column and loc.end.column
            if let Some(serde_json::Value::Object(loc)) = obj.get_mut("loc") {
                if let (Some(start_byte), Some(serde_json::Value::Object(start_pos))) =
                    (orig_start, loc.get_mut("start"))
                {
                    translate_loc_position(start_pos, start_byte, map, tracker);
                }
                if let (Some(end_byte), Some(serde_json::Value::Object(end_pos))) =
                    (orig_end, loc.get_mut("end"))
                {
                    translate_loc_position(end_pos, end_byte, map, tracker);
                }
            }

            // Translate name_loc (Svelte-specific: start/end with line, column, character)
            // For name_loc, the byte offset comes from the `character` field
            if let Some(serde_json::Value::Object(name_loc)) = obj.get_mut("name_loc") {
                if let Some(serde_json::Value::Object(start_pos)) = name_loc.get_mut("start")
                    && let Some(char_byte) = start_pos
                        .get("character")
                        .and_then(serde_json::Value::as_u64)
                        .map(|v| v as u32)
                {
                    translate_loc_position(start_pos, char_byte, map, tracker);
                }
                if let Some(serde_json::Value::Object(end_pos)) = name_loc.get_mut("end")
                    && let Some(char_byte) = end_pos
                        .get("character")
                        .and_then(serde_json::Value::as_u64)
                        .map(|v| v as u32)
                {
                    translate_loc_position(end_pos, char_byte, map, tracker);
                }
            }

            // Translate extra.trailingComma (TSTypeParameterDeclaration `<T,>`):
            // a byte offset that acorn emits in UTF-16 code units
            if let Some(serde_json::Value::Object(extra)) = obj.get_mut("extra")
                && let Some(trailing_comma) = extra
                    .get("trailingComma")
                    .and_then(serde_json::Value::as_u64)
                    .map(|v| v as u32)
            {
                extra.insert(
                    "trailingComma".to_string(),
                    serde_json::Value::Number(map.byte_to_char(trailing_comma).into()),
                );
            }

            // Recurse into all values (skip loc/name_loc which we already handled)
            for (key, val) in obj.iter_mut() {
                if key != "loc" && key != "name_loc" {
                    translate_positions_recursive(val, map, tracker);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                translate_positions_recursive(item, map, tracker);
            }
        }
        _ => {}
    }
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

/// Convert tsv_lang::SourceLocation to public::SourceLocation
///
/// Converts from the generic location type to the TypeScript-specific public type
/// with serde derives.
#[inline]
pub(super) fn to_public_location(loc: tsv_lang::SourceLocation) -> public::SourceLocation {
    public::SourceLocation {
        start: public::Position {
            line: loc.start.line,
            column: loc.start.column,
            character: None,
        },
        end: public::Position {
            line: loc.end.line,
            column: loc.end.column,
            character: None,
        },
    }
}

/// Create source location, automatically handling offset if needed
///
/// Unified helper that eliminates repetitive if/else checks throughout conversion.
/// When offset is 0, uses fast path directly. When offset is non-zero, adjusts span accordingly.
#[inline]
pub(super) fn create_location(
    span: Span,
    tracker: &LocationTracker,
    offset: usize,
) -> public::SourceLocation {
    let loc = if offset == 0 {
        tracker.span_to_location(span)
    } else {
        tracker.span_to_location_with_offset(span, offset)
    };
    to_public_location(loc)
}

/// Convert an `f64` literal value to a `serde_json::Number`, mapping non-finite
/// values (NaN / ±Inf) to `0` for acorn parity — non-finite number literals
/// don't occur in valid source, and JSON has no representation for them.
pub(super) fn json_number_from_f64(n: f64) -> serde_json::Number {
    serde_json::Number::from_f64(n).unwrap_or_else(|| serde_json::Number::from(0))
}

/// Convert an internal `Program` to the public AST under the given schema.
///
/// Use `Schema::Acorn` for standalone TypeScript and Svelte `lang="ts"` scripts;
/// use `Schema::SvelteScript` for Svelte non-`lang="ts"` `<script>` blocks where
/// the JSON shape must follow Svelte's parser quirks.
pub fn convert_program<'src>(
    program: &internal::Program<'_>,
    source: &'src str,
    loc: &LocationTracker,
    schema: Schema,
) -> public::Program<'src> {
    let interner = program.interner.borrow();

    public::Program {
        node_type: "Program",
        start: program.span.start,
        end: program.span.end,
        loc: create_location(program.span, loc, 0),
        body: program
            .body
            .iter()
            .map(|s| convert_statement(s, source, loc, &interner, 0, schema))
            .collect(),
        source_type: program.goal.source_type(),
    }
}

#[cfg(test)]
mod tests {
    use super::json_number_from_f64;

    #[test]
    fn json_number_from_f64_finite_and_non_finite() {
        // Finite values pass through faithfully (the only case valid source hits).
        assert_eq!(json_number_from_f64(1.5).as_f64(), Some(1.5));
        assert_eq!(json_number_from_f64(0.0).as_f64(), Some(0.0));
        assert_eq!(json_number_from_f64(-42.0).as_f64(), Some(-42.0));
        // Non-finite values never occur in valid source, but the defensive arm
        // must collapse them to integer 0 — JSON has no NaN/Infinity, and acorn
        // emits 0 here too.
        for n in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(json_number_from_f64(n), serde_json::Number::from(0));
        }
    }
}
