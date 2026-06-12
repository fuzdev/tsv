use super::CssParser;
use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

/// Parse attribute selector: [attr], [attr="value"], [attr^="prefix"]
/// Supports namespace prefixes: [ns|attr], [*|attr], [|attr]
pub(crate) fn parse_attribute_selector(
    parser: &mut CssParser,
    start: usize,
) -> Result<SimpleSelector, ParseError> {
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
        Some("*".to_string())
    } else if parser.check(TokenKind::Pipe) {
        // Explicit no namespace: |attr
        parser.advance()?; // consume |
        parser.skip_whitespace()?;
        Some(String::new())
    } else if parser.check(TokenKind::Identifier) {
        // Could be: ns|attr or just attr (or lang with |= operator)
        let maybe_namespace = parser
            .current_identifier()
            .ok_or_else(|| parser.error_expected("identifier"))?
            .to_string();
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
                // and return early
                let name = maybe_namespace;
                parser.advance()?; // consume =
                parser.skip_whitespace()?;

                // Parse value (identifier or string)
                let value = match &parser.current_kind {
                    TokenKind::Identifier => Some(
                        parser
                            .current_identifier()
                            .unwrap_or_else(|| parser.current_value())
                            .to_string(),
                    ),
                    TokenKind::String { .. } => {
                        // Extract content without quotes
                        Some(
                            parser.source()[parser.current_start + 1..parser.current_end - 1]
                                .to_string(),
                        )
                    }
                    _ => {
                        return Err(parser.error_expected("attribute value"));
                    }
                };
                parser.advance()?;
                parser.skip_whitespace()?;

                // Parse attribute flags (i/I=case-insensitive, s/S=case-sensitive) - optional
                let flags = if parser.check(TokenKind::Identifier) {
                    let flag = parser.current_value().to_string();
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
                    name,
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
            // It's the attribute name, not a namespace
            // Restore the identifier as the name
            let name = maybe_namespace;

            // Check for matcher and value: =, ~=, |=, ^=, $=, *=
            let (matcher, value) = if parser.check(TokenKind::RightBracket) {
                (None, None) // Just [attr]
            } else {
                let matcher = parse_attribute_matcher(parser)?;
                parser.skip_whitespace()?;

                // Parse value (identifier or string)
                let value = match &parser.current_kind {
                    // Internal AST: use decoded value (spec-compliant)
                    TokenKind::Identifier => Some(
                        parser
                            .current_identifier()
                            .unwrap_or_else(|| parser.current_value())
                            .to_string(),
                    ),
                    TokenKind::String { .. } => {
                        // Extract content without quotes
                        Some(
                            parser.source()[parser.current_start + 1..parser.current_end - 1]
                                .to_string(),
                        )
                    }
                    _ => {
                        return Err(parser.error_expected("attribute value"));
                    }
                };
                parser.advance()?;
                parser.skip_whitespace()?;

                (Some(matcher), value)
            };

            // Parse attribute flags (i/I=case-insensitive, s/S=case-sensitive) - optional
            let flags = if parser.check(TokenKind::Identifier) {
                let flag = parser.current_value().to_string();
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
                name,
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

    // Internal AST: use decoded value (spec-compliant)
    let name = parser
        .current_identifier()
        .ok_or_else(|| parser.error_expected("identifier"))?
        .to_string();
    parser.advance()?;
    parser.skip_whitespace()?;

    // Check for matcher and value: =, ~=, |=, ^=, $=, *=
    let (matcher, value) = if parser.check(TokenKind::RightBracket) {
        (None, None) // Just [attr]
    } else {
        let matcher = parse_attribute_matcher(parser)?;
        parser.skip_whitespace()?;

        // Parse value (identifier or string)
        let value = match &parser.current_kind {
            // Internal AST: use decoded value (spec-compliant)
            TokenKind::Identifier => Some(
                parser
                    .current_identifier()
                    .unwrap_or_else(|| parser.current_value())
                    .to_string(),
            ),
            TokenKind::String { .. } => {
                // Extract content without quotes
                Some(parser.source()[parser.current_start + 1..parser.current_end - 1].to_string())
            }
            _ => {
                return Err(parser.error_expected("attribute value"));
            }
        };
        parser.advance()?;
        parser.skip_whitespace()?;

        (Some(matcher), value)
    };

    // Parse attribute flags (i/I=case-insensitive, s/S=case-sensitive) - optional
    let flags = if parser.check(TokenKind::Identifier) {
        let flag = parser.current_value().to_string();
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
        name,
        matcher,
        value,
        flags,
        span: Span {
            start: start as u32,
            end,
        },
    })
}

/// Parse attribute matcher: =, ~=, |=, ^=, $=, *=
fn parse_attribute_matcher(parser: &mut CssParser) -> Result<AttributeMatcher, ParseError> {
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
