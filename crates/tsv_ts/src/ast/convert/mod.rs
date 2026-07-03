// Conversion from the internal AST to the public wire JSON.
//
// The writer (`write/`) emits the compact wire JSON directly from the internal
// AST in one walk, fusing byte→UTF-16 offset translation into the walk (final
// char-space positions emitted directly via `LocationMapper`). It is the sole
// emission path; `convert_ast_json_bytes`/`_string` in `lib.rs` call it, and
// `convert_ast_json` parses its bytes back into a `Value`.

use tsv_lang::{ByteToCharMap, LocationTracker};

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
    write_pattern_embedded_with_comments, write_program_embedded, write_program_json,
    write_variable_declaration_embedded, write_variable_declaration_embedded_with_comments,
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
        u128::from_str_radix(hex, 16)
            .map_or_else(|_| radix_digits_to_decimal(hex, 16), |n| n.to_string())
    } else if let Some(oct) = val.strip_prefix("0o").or_else(|| val.strip_prefix("0O")) {
        u128::from_str_radix(oct, 8)
            .map_or_else(|_| radix_digits_to_decimal(oct, 8), |n| n.to_string())
    } else if let Some(bin) = val.strip_prefix("0b").or_else(|| val.strip_prefix("0B")) {
        u128::from_str_radix(bin, 2)
            .map_or_else(|_| radix_digits_to_decimal(bin, 2), |n| n.to_string())
    } else {
        val.to_string()
    }
}

/// Decimal digits of an arbitrarily long radix-2/8/16 digit string — the
/// beyond-`u128` fallback (rare: >32 hex digits). Schoolbook multiply-add
/// over a little-endian decimal-digit accumulator; the digits were already
/// validated by the lexer.
fn radix_digits_to_decimal(digits: &str, radix: u32) -> String {
    let mut dec: Vec<u8> = vec![0];
    for ch in digits.chars() {
        let Some(d) = ch.to_digit(radix) else {
            continue; // unreachable: the lexer validated every digit
        };
        let mut carry = d;
        for slot in &mut dec {
            let v = u32::from(*slot) * radix + carry;
            #[allow(clippy::cast_possible_truncation)]
            {
                *slot = (v % 10) as u8;
            }
            carry = v / 10;
        }
        while carry > 0 {
            #[allow(clippy::cast_possible_truncation)]
            dec.push((carry % 10) as u8);
            carry /= 10;
        }
    }
    dec.iter().rev().map(|&d| char::from(b'0' + d)).collect()
}
