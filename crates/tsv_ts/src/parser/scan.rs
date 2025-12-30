// Low-level byte scanning utilities for parser lookahead
//
// These are generic helpers for scanning raw bytes, used by both expression
// parsing (arrow function detection) and type parsing (index signature detection).

/// Skip ASCII whitespace characters in a byte slice, returning new position
#[inline]
pub(super) fn skip_whitespace(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() && matches!(bytes[pos], b' ' | b'\t' | b'\n' | b'\r') {
        pos += 1;
    }
    pos
}

/// Skip a line comment (// ...), returning position after the newline
/// Assumes `pos` is at the first `/`
#[inline]
pub(super) fn skip_line_comment(bytes: &[u8], mut pos: usize) -> usize {
    // Skip //
    pos += 2;
    // Read until newline or EOF
    while pos < bytes.len() && bytes[pos] != b'\n' && bytes[pos] != b'\r' {
        pos += 1;
    }
    pos
}

/// Skip a block comment (/* ... */), returning position after the closing */
/// Assumes `pos` is at the first `/`
#[inline]
pub(super) fn skip_block_comment(bytes: &[u8], mut pos: usize) -> usize {
    // Skip /*
    pos += 2;
    while pos + 1 < bytes.len() {
        if bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
            return pos + 2;
        }
        pos += 1;
    }
    pos
}

/// Skip whitespace and comments, returning new position
#[inline]
pub(super) fn skip_whitespace_and_comments(bytes: &[u8], mut pos: usize) -> usize {
    loop {
        let start = pos;
        pos = skip_whitespace(bytes, pos);
        // Check for comments
        if pos + 1 < bytes.len() && bytes[pos] == b'/' {
            if bytes[pos + 1] == b'/' {
                pos = skip_line_comment(bytes, pos);
            } else if bytes[pos + 1] == b'*' {
                pos = skip_block_comment(bytes, pos);
            } else {
                break;
            }
        } else {
            break;
        }
        // Continue loop to handle whitespace after comment
        if pos == start {
            break;
        }
    }
    pos
}

/// Check if a byte can start an identifier (letter, underscore, dollar sign, or non-ASCII)
///
/// Non-ASCII bytes (> 127) are included for lookahead purposes - they're part of multi-byte
/// UTF-8 sequences that are likely unicode identifier chars. The actual lexer uses proper
/// `is_xid_start` from `unicode_ident` crate for validation.
#[inline]
pub(super) fn is_identifier_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b == b'$' || b > 127
}

/// Check if a byte can continue an identifier (alphanumeric, underscore, dollar sign, or non-ASCII)
///
/// Non-ASCII bytes (> 127) are included for lookahead purposes - they're part of multi-byte
/// UTF-8 sequences that are likely unicode identifier chars. The actual lexer uses proper
/// `is_xid_continue` from `unicode_ident` crate for validation.
#[inline]
pub(super) fn is_identifier_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b > 127
}

/// Skip an identifier, returning position after the identifier
/// Assumes `pos` is at the start of an identifier
#[inline]
pub(super) fn skip_identifier(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() && is_identifier_continue(bytes[pos]) {
        pos += 1;
    }
    pos
}

/// Skip a string literal (single or double quoted), returning position after closing quote
/// Assumes `pos` is at the opening quote character
#[inline]
pub(super) fn skip_string_literal(bytes: &[u8], mut pos: usize) -> usize {
    let quote = bytes[pos];
    pos += 1;
    while pos < bytes.len() && bytes[pos] != quote {
        if bytes[pos] == b'\\' && pos + 1 < bytes.len() {
            pos += 1; // skip escaped char
        }
        pos += 1;
    }
    // Return position AFTER the closing quote (if found)
    if pos < bytes.len() { pos + 1 } else { pos }
}

/// Parse a JS number literal (hex, binary, octal, scientific, BigInt)
/// Returns f64 (BigInt suffix 'n' is ignored for value, preserved in raw)
///
/// Note: Precision loss for large integers (>2^52) matches JS behavior.
#[allow(clippy::cast_precision_loss)]
pub(crate) fn parse_number_literal(raw: &str) -> Result<f64, std::num::ParseFloatError> {
    // Remove numeric separators
    let clean: String = raw.chars().filter(|&c| c != '_').collect();

    // Strip BigInt suffix
    let clean = clean.strip_suffix('n').unwrap_or(&clean);

    if clean.len() >= 2 {
        let prefix = &clean[..2];
        let digits = &clean[2..];
        match prefix {
            "0x" | "0X" => {
                // Hex: 0xff
                return Ok(i64::from_str_radix(digits, 16).unwrap_or(0) as f64);
            }
            "0b" | "0B" => {
                // Binary: 0b1010
                return Ok(i64::from_str_radix(digits, 2).unwrap_or(0) as f64);
            }
            "0o" | "0O" => {
                // Octal: 0o77
                return Ok(i64::from_str_radix(digits, 8).unwrap_or(0) as f64);
            }
            _ => {}
        }
    }

    // Regular decimal (including scientific notation)
    clean.parse::<f64>()
}
