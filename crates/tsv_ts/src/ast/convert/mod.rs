// Conversion from the internal AST to the public wire JSON.
//
// The writer (`write/`) emits the compact wire JSON directly from the internal
// AST in one walk, fusing byte→UTF-16 offset translation into the walk (final
// char-space positions emitted directly via `WirePositions`). It is the sole
// emission path; `convert_ast_json_bytes` / `convert_ast_json_string` and
// `convert_ast_json_bytes_with_locations` in `lib.rs` call it.

// The writer — the sole emission mode.
mod write;

pub(crate) use write::write_program_bytes;
pub use write::{
    CommentAttach, CommentMode, EmbedWriter, IslandComments, write_expression_embedded,
    write_identifier_expression_with_character, write_pattern_embedded, write_program_embedded,
    write_variable_declaration_embedded,
};

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
            {
                *slot = (v % 10) as u8;
            }
            carry = v / 10;
        }
        while carry > 0 {
            dec.push((carry % 10) as u8);
            carry /= 10;
        }
    }
    dec.iter().rev().map(|&d| char::from(b'0' + d)).collect()
}
