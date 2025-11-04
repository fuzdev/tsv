use super::CssParser;
use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

/// Parse a CSS rule: `selector { property: value; }`
pub(crate) fn parse_rule(parser: &mut CssParser) -> Result<CssRule, ParseError> {
    let start = parser.base_offset() + parser.current_start;

    // Parse selector list
    let selector = super::selectors::parse_selector_list(parser)?;

    // Expect { and capture its start
    let block_start = parser.base_offset() + parser.current_start;
    parser.expect(&TokenKind::LeftBrace)?;
    parser.skip_whitespace()?;

    // Parse declarations
    let mut declarations = Vec::new();
    while !parser.check(&TokenKind::RightBrace) && !parser.check(&TokenKind::Eof) {
        if parser.check(&TokenKind::Identifier) {
            let decl = parse_declaration(parser)?;
            declarations.push(decl);
        } else {
            // Skip unexpected tokens
            parser.advance()?;
        }
        parser.skip_whitespace()?;
    }

    // Expect } and capture its end position
    if !parser.check(&TokenKind::RightBrace) {
        return Err(ParseError::InvalidSyntax {
            message: "Expected '}'".to_string(),
            position: parser.base_offset() + parser.current_start,
            context: None,
        });
    }
    let block_end = parser.base_offset() + parser.current_end;
    let end = block_end;
    parser.advance()?; // consume }

    Ok(CssRule {
        selector,
        block_span: Span {
            start: block_start as u32,
            end: block_end as u32,
        },
        declarations,
        span: Span {
            start: start as u32,
            end: end as u32,
        },
    })
}

/// Parse a CSS declaration: `property: value;`
pub(crate) fn parse_declaration(parser: &mut CssParser) -> Result<CssDeclaration, ParseError> {
    let start = parser.base_offset() + parser.current_start;

    // Parse property
    if !parser.check(&TokenKind::Identifier) {
        return Err(ParseError::InvalidSyntax {
            message: "Expected property name".to_string(),
            position: start,
            context: None,
        });
    }
    // Internal AST: use decoded value (spec-compliant)
    // Svelte quirk (raw value) will be applied in conversion layer
    let property = parser
        .current_identifier()
        .ok_or_else(|| ParseError::InvalidSyntax {
            message: "Expected identifier".to_string(),
            position: start,
            context: None,
        })?
        .to_string();
    parser.advance()?;

    parser.skip_whitespace()?;

    // Expect :
    parser.expect(&TokenKind::Colon)?;
    parser.skip_whitespace()?;

    // Track value end for span calculation
    // (value_start was removed - no longer needed since we extract from source on-demand)

    // Parse value (collect tokens until ; or })
    let mut value_parts = Vec::new();
    let mut value_end = start;
    while !parser.check(&TokenKind::Semicolon)
        && !parser.check(&TokenKind::RightBrace)
        && !parser.check(&TokenKind::Eof)
    {
        // Convert token to string representation for value
        let value_str = match &parser.current_kind {
            // Internal AST: use decoded value (spec-compliant)
            TokenKind::Identifier => parser
                .current_identifier()
                .unwrap_or_else(|| parser.current_value())
                .to_string(),
            TokenKind::String { content, quote } => format!("{}{}{}", quote, content, quote),
            TokenKind::Number(n) => n.clone(),
            TokenKind::Percentage(n) => format!("{}%", n),
            TokenKind::Dimension(n, unit) => format!("{}{}", n, unit),
            TokenKind::Whitespace => {
                parser.advance()?;
                continue;
            }
            TokenKind::Comment(_) => {
                // Skip comments in values
                parser.advance()?;
                continue;
            }
            _ => {
                // Other tokens - include them as-is from source
                parser.current_value().to_string()
            }
        };

        value_parts.push(value_str);
        value_end = parser.base_offset() + parser.current_end;
        parser.advance()?;
    }

    // Join value parts intelligently - only add spaces when needed
    // Most CSS values don't need spaces (translateX(0), not translateX ( 0 ))
    let value_str = if value_parts.is_empty() {
        String::new()
    } else {
        let mut result = String::new();
        for (i, part) in value_parts.iter().enumerate() {
            if i > 0 {
                // Add space only between certain token combinations
                let prev_part = &value_parts[i - 1];
                let needs_space = super::value::should_add_space_between(prev_part, part);
                if needs_space {
                    result.push(' ');
                }
            }
            result.push_str(part);
        }
        result
    };

    if value_str.is_empty() {
        return Err(ParseError::InvalidSyntax {
            message: "Empty CSS value".to_string(),
            position: start,
            context: None,
        });
    }

    // Parse the value string into structured CssValue AST
    let value = super::value::parse_value_string(&value_str);

    // Declaration ends after the value, NOT including the semicolon
    let end = value_end;

    // Optionally consume semicolon (but don't include it in the declaration span)
    if parser.check(&TokenKind::Semicolon) {
        parser.advance()?;
    }

    // Span covers the entire declaration (property + value, not including semicolon)
    // The source value will be extracted on-demand during conversion using this span
    Ok(CssDeclaration {
        property,
        value,
        span: Span {
            start: start as u32,
            end: end as u32,
        },
    })
}
