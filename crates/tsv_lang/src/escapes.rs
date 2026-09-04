//! Shared escape sequence utilities for string literals
//!
//! Provides utilities for manipulating escape sequences in raw string content,
//! used by printers when changing quote styles while preserving all other
//! escape sequences exactly as they appear in the source.

/// Swap quote escaping when changing quote styles in raw string content
///
/// When a printer changes the quote style of a string literal, it needs to:
/// 1. Unescape the old quote character (no longer needs escaping)
/// 2. Escape the new quote character (now needs escaping)
/// 3. Preserve all other escape sequences exactly as-is (\n, \t, \\, \u{...}, etc.)
///
/// This function operates on **raw content** (the string content between quotes,
/// with escape sequences still encoded as backslash sequences from the source).
///
/// Byte runs, not chars: only three bytes can move the state — `\`, the new quote (to be
/// escaped) and, behind a `\`, the old quote (to be unescaped) — and all three are ASCII,
/// which no UTF-8 continuation byte equals, so the walk hops run to run on
/// [`crate::swar::next_byte_of`] and copies each run with one `push_str`. The byte after a
/// `\` is skipped, never scanned, so an already-escaped new quote (`\"` staying `\"`) is not
/// escaped twice; a multi-byte char behind a `\` stays whole, its lead byte the skipped one
/// and its continuation bytes matching no needle. The char loop this replaces pushed every
/// char through `String::push`'s reserve-and-encode, and real CSS strings are icon-font
/// escapes and data URIs — long runs, few hits.
///
/// # Arguments
/// * `content` - Raw string content without surrounding quotes (with escape sequences)
/// * `old_quote` - The quote character being changed from (`'` or `"`)
/// * `new_quote` - The quote character being changed to (`'` or `"`)
/// * `out` - The buffer the adjusted content is appended to — the literal being
///   assembled, so the swap is written once rather than into a `String` of its own
///   that is then copied in
///
/// # Examples
/// ```ignore
/// use tsv_lang::escapes::swap_quote_escaping_into;
///
/// // Single-quoted string with escaped single quote → double quotes
/// // Source: 'it\'s great' → raw content: it\'s great
/// // Result: "it's great" → raw content: it's great
/// let input = r"it\'s great";
/// let mut result = String::new();
/// swap_quote_escaping_into(input, '\'', '"', &mut result);
/// assert_eq!(result, r"it's great");  // \' → ' (unescaped)
///
/// // Single-quoted string with unescaped double quote → double quotes
/// // Source: 'has "double" quotes' → raw content: has "double" quotes
/// // Result: "has \"double\" quotes" → raw content: has \"double\" quotes
/// let input = r#"has "double" quotes"#;
/// let mut result = String::new();
/// swap_quote_escaping_into(input, '\'', '"', &mut result);
/// assert_eq!(result, r#"has \"double\" quotes"#);  // " → \" (escaped for new quotes)
/// ```
pub fn swap_quote_escaping_into(content: &str, old_quote: char, new_quote: char, out: &mut String) {
    if old_quote == new_quote {
        out.push_str(content);
        return;
    }
    debug_assert!(
        old_quote.is_ascii() && new_quote.is_ascii(),
        "string quotes are ASCII, so a byte compare cannot match inside a multi-byte char"
    );
    let (old_q, new_q) = (old_quote as u8, new_quote as u8);
    let bytes = content.as_bytes();
    // `run_start`..: the verbatim run not yet copied. Every position it and `hit` take is
    // an ASCII byte's, or 0 / `len`, so each slice below is on a char boundary.
    let mut run_start = 0;
    let mut i = 0;
    while i < bytes.len() {
        let hit = crate::swar::next_byte_of(bytes, i, [b'\\', new_q]);
        if hit == bytes.len() {
            break;
        }
        if bytes[hit] == b'\\' {
            if bytes.get(hit + 1) == Some(&old_q) {
                // Unescape old quote: \' → ' or \" → "
                out.push_str(&content[run_start..hit]);
                out.push(old_quote);
                run_start = hit + 2;
            }
            // Every other escape stays as-is, escaped char included — \n, \t, \\, \u{...},
            // an already-escaped new quote — and so does a trailing backslash (malformed).
            i = (hit + 2).min(bytes.len());
        } else {
            // Escape unescaped new quote character: " → \" or ' → \' — the quote itself
            // heads the next run.
            out.push_str(&content[run_start..hit]);
            out.push('\\');
            run_start = hit;
            i = hit + 1;
        }
    }
    out.push_str(&content[run_start..]);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [`swap_quote_escaping_into`] into a fresh `String` — the spelling the tests read.
    fn swap_quote_escaping(content: &str, old_quote: char, new_quote: char) -> String {
        let mut out = String::new();
        swap_quote_escaping_into(content, old_quote, new_quote, &mut out);
        out
    }

    /// The char-at-a-time spelling [`swap_quote_escaping_into`] replaced, kept as the oracle.
    fn swap_quote_escaping_reference(content: &str, old_quote: char, new_quote: char) -> String {
        if old_quote == new_quote {
            return content.to_string();
        }
        let mut result = String::with_capacity(content.len());
        let mut chars = content.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                if let Some(&next) = chars.peek() {
                    chars.next();
                    if next == old_quote {
                        result.push(old_quote);
                    } else {
                        result.push('\\');
                        result.push(next);
                    }
                } else {
                    result.push('\\');
                }
            } else if ch == new_quote {
                result.push('\\');
                result.push(new_quote);
            } else {
                result.push(ch);
            }
        }
        result
    }

    /// The byte-run walk against the char-at-a-time reference: every escape and quote
    /// shape at every position within and across word boundaries, multi-byte chars on
    /// either side of a `\`, a trailing backslash, and both swap directions.
    #[test]
    fn swap_quote_escaping_matches_the_char_reference() {
        let pieces = [
            "",
            "a",
            "abc",
            "\\'",
            "\\\"",
            "\\\\",
            "'",
            "\"",
            "\\",
            "\\n",
            "\\u{e9}",
            "é",
            "\\é",
            "日本",
            "\\日",
            "0123456789abcdef",
            "\\'\\'\\'",
            "\"\"\"",
            "data:image/png;base64,iVBOR",
        ];
        for a in pieces {
            for b in pieces {
                for c in pieces {
                    let content = format!("{a}{b}{c}");
                    for (old_quote, new_quote) in [('\'', '"'), ('"', '\''), ('\'', '\'')] {
                        assert_eq!(
                            swap_quote_escaping(&content, old_quote, new_quote),
                            swap_quote_escaping_reference(&content, old_quote, new_quote),
                            "{content:?} {old_quote} -> {new_quote}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_swap_quote_escaping_same_quote() {
        // No change if quote stays the same
        assert_eq!(swap_quote_escaping("test", '\'', '\''), "test");
        assert_eq!(swap_quote_escaping(r"it\'s", '\'', '\''), r"it\'s");
    }

    #[test]
    fn test_swap_quote_escaping_single_to_double() {
        // Scenario: "has 'single' quotes" in single quotes with escapes
        // Input: 'has \'single\' quotes' → raw content: has \'single\' quotes
        // After swap to double quotes: "has 'single' quotes" → raw content: has 'single' quotes
        let input = "has \\'single\\' quotes";
        let expected = "has 'single' quotes";
        assert_eq!(swap_quote_escaping(input, '\'', '"'), expected);
    }

    #[test]
    fn test_swap_quote_escaping_double_to_single() {
        // Scenario: 'has "double" quotes' (single quotes, unescaped " inside)
        // Raw content: has "double" quotes (no escapes needed for " in single quotes)
        // Swap to double quotes: "has \"double\" quotes"
        // Raw content becomes: has \"double\" quotes (escape " for double quotes)
        let input = "has \"double\" quotes";
        let expected = "has \\\"double\\\" quotes";
        assert_eq!(swap_quote_escaping(input, '\'', '"'), expected);
    }

    #[test]
    fn test_swap_quote_escaping_with_both_quotes() {
        // Scenario: "has 'both' \"types\"" in single quotes
        // Input: 'has \'both\' "types"' → raw content: has \'both\' "types"
        // After swap to double quotes: "has 'both' "types"" → raw content: has 'both' \"types\"
        let input = "has \\'both\\' \"types\"";
        let expected = "has 'both' \\\"types\\\"";
        assert_eq!(swap_quote_escaping(input, '\'', '"'), expected);
    }

    #[test]
    fn test_swap_quote_escaping_preserves_other_escapes() {
        // Other escapes are preserved exactly (including unicode, hex, special chars)
        assert_eq!(swap_quote_escaping(r"test\n\t\\", '\'', '"'), r"test\n\t\\");
        assert_eq!(swap_quote_escaping(r"\u0041\x42", '\'', '"'), r"\u0041\x42");
        assert_eq!(
            swap_quote_escaping(r"line\nbreak", '"', '\''),
            r"line\nbreak"
        );
    }

    #[test]
    fn test_swap_quote_escaping_already_escaped_new_quote() {
        // Edge case: Input has escaped new quote even though using old quote
        // Example: 'has \"double\" quotes' (using single quotes but \" is escaped)
        // After swap to double quotes: should keep \" as-is
        let input = "has \\\"double\\\" quotes";
        let expected = "has \\\"double\\\" quotes";
        assert_eq!(swap_quote_escaping(input, '\'', '"'), expected);
    }

    #[test]
    fn test_swap_quote_escaping_mixed_with_other_escapes() {
        // Combination of quote escapes and other escapes
        let input = "it\\'s\\ngreat";
        let expected = "it's\\ngreat";
        assert_eq!(swap_quote_escaping(input, '\'', '"'), expected);
    }
}
