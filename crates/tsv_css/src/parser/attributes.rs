use super::CssParser;
use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

/// Parse attribute selector: [attr], [attr="value"], [attr^="prefix"]
/// Supports namespace prefixes: [ns|attr], [*|attr], [|attr]
pub(crate) fn parse_attribute_selector<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    start: usize,
) -> Result<SimpleSelector<'arena>, ParseError> {
    parser.expect(TokenKind::LeftBracket)?;
    parser.skip_whitespace()?;

    // Parse namespace prefix (optional):
    // - [ns|attr] - namespace prefix "ns"
    // - [*|attr] - universal namespace "*"
    // - [|attr] - explicit no namespace ""
    // - [attr] - implicit no namespace (None)
    let namespace = if parser.check(TokenKind::Asterisk) {
        // Universal namespace: *|attr
        parser.advance()?;
        if !parser.check(TokenKind::Pipe) {
            return Err(parser.error_expected_after("'|'", "'*' in attribute selector"));
        }
        parser.advance()?; // consume |
        parser.skip_whitespace()?;
        Some("*")
    } else if parser.check(TokenKind::Pipe) {
        // Explicit no namespace: |attr
        parser.advance()?; // consume |
        parser.skip_whitespace()?;
        Some("")
    } else if parser.check(TokenKind::Identifier) {
        // Could be: ns|attr or just attr (or lang with |= operator). Capture the identifier's
        // span before consuming it: if it turns out to be the attribute *name* (not a namespace
        // prefix), this is the verbatim name span — the decoded string below is only needed when
        // it's actually a namespace.
        let maybe_namespace_span = Span {
            start: (parser.base_offset() + parser.current_start) as u32,
            end: (parser.base_offset() + parser.current_end) as u32,
        };
        let maybe_namespace = parser.alloc_str_in(
            parser
                .current_identifier()
                .ok_or_else(|| parser.error_expected("identifier"))?,
        );
        parser.advance()?;
        parser.skip_whitespace()?;

        // Check if this is a namespace prefix (identifier|identifier)
        // vs dash-match operator (identifier|=value)
        if parser.check(TokenKind::Pipe) {
            // Peek ahead to distinguish namespace from |= operator
            parser.advance()?; // consume |
            parser.skip_whitespace()?;

            // If next token is =, this was the |= operator, not a namespace
            if parser.check(TokenKind::Equals) {
                // Backtrack: restore the identifier as attribute name
                // and let matcher parsing handle |=
                // We need to restore state...
                // Actually, we can't easily backtrack here. Let me restructure.

                // Since we've already consumed the |, we need to handle |= here
                // and return early. The identifier before `|=` is the attribute name —
                // use its captured span (the decoded `maybe_namespace` was a false start).
                parser.advance()?; // consume =
                parser.skip_whitespace()?;

                // Parse value (identifier or string)
                let value = parse_attribute_value(parser)?;

                // Parse attribute flags (i/I=case-insensitive, s/S=case-sensitive) - optional
                let flags = if parser.check(TokenKind::Identifier) {
                    let flag = parser.alloc_str_in(parser.current_value());
                    let flag_lower = flag.to_lowercase();
                    if flag_lower == "i" || flag_lower == "s" {
                        parser.advance()?;
                        parser.skip_whitespace()?;
                        Some(flag) // Preserve original case
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Expect ] and capture its end position
                let end = parser.expect_and_capture(TokenKind::RightBracket)?;

                return Ok(SimpleSelector::Attribute {
                    namespace: None,
                    name_span: maybe_namespace_span,
                    matcher: Some(AttributeMatcher::DashMatch),
                    value,
                    flags,
                    span: Span {
                        start: start as u32,
                        end,
                    },
                });
            }

            // Otherwise it's a namespace prefix: ns|attr
            Some(maybe_namespace)
        } else {
            // It's the attribute name (no namespace) — use its captured span; the decoded
            // `maybe_namespace` is unused here (the name is recovered from source).

            // Check for matcher and value: =, ~=, |=, ^=, $=, *=
            let (matcher, value) = if parser.check(TokenKind::RightBracket) {
                (None, None) // Just [attr]
            } else {
                let matcher = parse_attribute_matcher(parser)?;
                parser.skip_whitespace()?;

                // Parse value (identifier or string)
                let value = parse_attribute_value(parser)?;

                (Some(matcher), value)
            };

            // Parse attribute flags (i/I=case-insensitive, s/S=case-sensitive) - optional
            let flags = if parser.check(TokenKind::Identifier) {
                let flag = parser.alloc_str_in(parser.current_value());
                let flag_lower = flag.to_lowercase();
                // Accept both lowercase and uppercase flag letters
                if flag_lower == "i" || flag_lower == "s" {
                    parser.advance()?;
                    parser.skip_whitespace()?;
                    Some(flag) // Preserve original case
                } else {
                    None
                }
            } else {
                None
            };

            // Expect ] and capture its end position
            let end = parser.expect_and_capture(TokenKind::RightBracket)?;

            return Ok(SimpleSelector::Attribute {
                namespace: None, // No namespace prefix (implicit)
                name_span: maybe_namespace_span,
                matcher,
                value,
                flags,
                span: Span {
                    start: start as u32,
                    end,
                },
            });
        }
    } else {
        return Err(parser.error_expected("attribute name, '*', or '|'"));
    };

    // Now parse the attribute name (after namespace|)
    if !parser.check(TokenKind::Identifier) {
        return Err(parser.error_expected_after("attribute name", "namespace"));
    }

    // Name recovered verbatim from its span (printer emits raw; convert half-decodes — see
    // ast/convert/mod.rs). The `Identifier` check above guarantees a name token, so no decoded
    // copy is stored.
    let name_span = Span {
        start: (parser.base_offset() + parser.current_start) as u32,
        end: (parser.base_offset() + parser.current_end) as u32,
    };
    parser.advance()?;
    parser.skip_whitespace()?;

    // Check for matcher and value: =, ~=, |=, ^=, $=, *=
    let (matcher, value) = if parser.check(TokenKind::RightBracket) {
        (None, None) // Just [attr]
    } else {
        let matcher = parse_attribute_matcher(parser)?;
        parser.skip_whitespace()?;

        // Parse value (identifier or string)
        let value = parse_attribute_value(parser)?;

        (Some(matcher), value)
    };

    // Parse attribute flags (i/I=case-insensitive, s/S=case-sensitive) - optional
    let flags = if parser.check(TokenKind::Identifier) {
        let flag = parser.alloc_str_in(parser.current_value());
        let flag_lower = flag.to_lowercase();
        // Accept both lowercase and uppercase flag letters
        if flag_lower == "i" || flag_lower == "s" {
            parser.advance()?;
            parser.skip_whitespace()?;
            Some(flag) // Preserve original case
        } else {
            None
        }
    } else {
        None
    };

    // Expect ] and capture its end position
    let end = parser.expect_and_capture(TokenKind::RightBracket)?;

    Ok(SimpleSelector::Attribute {
        namespace,
        name_span,
        matcher,
        value,
        flags,
        span: Span {
            start: start as u32,
            end,
        },
    })
}

/// Parse an attribute selector's value (identifier or string), copying the
/// content (quotes stripped for strings) into the arena. Advances past the
/// value token and trailing whitespace.
fn parse_attribute_value<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<Option<&'arena str>, ParseError> {
    let value =
        match &parser.current_kind {
            // Internal AST: use decoded value (spec-compliant)
            TokenKind::Identifier => {
                let v = parser
                    .current_identifier()
                    .unwrap_or_else(|| parser.current_value());
                Some(parser.alloc_str_in(v))
            }
            TokenKind::String { .. } => {
                // Extract content without quotes
                Some(parser.alloc_str_in(
                    &parser.source()[parser.current_start + 1..parser.current_end - 1],
                ))
            }
            _ => {
                return Err(parser.error_expected("attribute value"));
            }
        };
    parser.advance()?;
    parser.skip_whitespace()?;
    Ok(value)
}

/// Parse attribute matcher: =, ~=, |=, ^=, $=, *=
fn parse_attribute_matcher(parser: &mut CssParser<'_, '_>) -> Result<AttributeMatcher, ParseError> {
    let matcher = match &parser.current_kind {
        TokenKind::Equals => AttributeMatcher::Exact, // =
        TokenKind::Tilde => {
            // ~= (contains in whitespace-separated list)
            parser.advance()?;
            if !parser.check(TokenKind::Equals) {
                return Err(parser.error_expected_after("'='", "'~'"));
            }
            parser.advance()?; // consume =
            return Ok(AttributeMatcher::Contains);
        }
        TokenKind::Pipe => {
            // |= (dash-match)
            parser.advance()?;
            if !parser.check(TokenKind::Equals) {
                return Err(parser.error_expected_after("'='", "'|'"));
            }
            parser.advance()?; // consume =
            return Ok(AttributeMatcher::DashMatch);
        }
        TokenKind::Caret => {
            // ^= (prefix match)
            parser.advance()?;
            if !parser.check(TokenKind::Equals) {
                return Err(parser.error_expected_after("'='", "'^'"));
            }
            parser.advance()?; // consume =
            return Ok(AttributeMatcher::Prefix);
        }
        TokenKind::Dollar => {
            // $= (suffix match)
            parser.advance()?;
            if !parser.check(TokenKind::Equals) {
                return Err(parser.error_expected_after("'='", "'$'"));
            }
            parser.advance()?; // consume =
            return Ok(AttributeMatcher::Suffix);
        }
        TokenKind::Asterisk => {
            // *= (substring match)
            parser.advance()?;
            if !parser.check(TokenKind::Equals) {
                return Err(parser.error_expected_after("'='", "'*'"));
            }
            parser.advance()?; // consume =
            return Ok(AttributeMatcher::Substring);
        }
        _ => {
            return Err(parser.error_msg(&format!(
                "Unsupported attribute matcher: {:?}",
                parser.current_kind
            )));
        }
    };

    parser.advance()?; // consume = for Exact match
    Ok(matcher)
}
