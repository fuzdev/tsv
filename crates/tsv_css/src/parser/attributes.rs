use super::CssParser;
use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

/// Parse attribute selector: [attr], [attr="value"], [attr^="prefix"]
pub(crate) fn parse_attribute_selector(
    parser: &mut CssParser,
    start: usize,
) -> Result<SimpleSelector, ParseError> {
    parser.expect(&TokenKind::LeftBracket)?;
    parser.skip_whitespace()?;

    if !parser.check(&TokenKind::Identifier) {
        return Err(ParseError::InvalidSyntax {
            message: "Expected attribute name".to_string(),
            position: parser.base_offset() + parser.current_start,
            context: None,
        });
    }

    // Internal AST: use decoded value (spec-compliant)
    let name = parser
        .current_identifier()
        .ok_or_else(|| ParseError::InvalidSyntax {
            message: "Expected identifier".to_string(),
            position: parser.base_offset() + parser.current_start,
            context: None,
        })?
        .to_string();
    parser.advance()?;
    parser.skip_whitespace()?;

    // Check for matcher and value: =, ~=, |=, ^=, $=, *=
    let (matcher, value) = if parser.check(&TokenKind::RightBracket) {
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
            TokenKind::String { content, .. } => Some(content.clone()),
            _ => {
                return Err(ParseError::InvalidSyntax {
                    message: "Expected attribute value".to_string(),
                    position: parser.base_offset() + parser.current_start,
                    context: None,
                });
            }
        };
        parser.advance()?;
        parser.skip_whitespace()?;

        (Some(matcher), value)
    };

    // TODO: Parse attribute flags (i, s)
    let flags = None;

    // Expect ] and capture its end position
    let end = parser.expect_and_capture(&TokenKind::RightBracket)?;

    Ok(SimpleSelector::Attribute {
        namespace: None, // TODO: Parse namespace
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
    // TODO: Implement compound matchers (^=, $=, *=, ~=, |=)
    // For now, just handle exact match (=)
    let matcher = match &parser.current_kind {
        TokenKind::Equals => AttributeMatcher::Exact,
        _ => {
            return Err(ParseError::InvalidSyntax {
                message: format!("Unsupported attribute matcher: {:?}", parser.current_kind),
                position: parser.base_offset() + parser.current_start,
                context: None,
            });
        }
    };

    parser.advance()?; // consume matcher
    Ok(matcher)
}
