use super::CssParser;
use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

/// Check if we're looking at the start of a nested rule (selector) rather than a declaration.
///
/// Nested rules can start with:
/// - `&` (nesting selector)
/// - `.` (class selector)
/// - `#` (ID selector)
/// - `*` (universal selector)
/// - `:` (pseudo-class/element)
/// - `[` (attribute selector)
/// - Identifier (type selector - but could also be a property name)
///
/// For identifiers, we need to look ahead: if next non-whitespace/comment token is `:`, it's a declaration.
pub(crate) fn is_nested_rule_start(parser: &mut CssParser) -> Result<bool, ParseError> {
    match &parser.current_kind {
        // Unambiguous selector start tokens
        TokenKind::Ampersand
        | TokenKind::Dot
        | TokenKind::Hash
        | TokenKind::Asterisk
        | TokenKind::Colon
        | TokenKind::LeftBracket => Ok(true),

        // Ambiguous: identifier could be type selector (nested rule) or property name (declaration)
        // Look ahead to check if next non-whitespace/comment token is `:` (declaration) or not (nested rule)
        TokenKind::Identifier => {
            // Peek past whitespace and comments to find the significant next token
            let next_kind = parser.peek_past_whitespace()?;
            match next_kind {
                // Colon after identifier (possibly with whitespace/comments) = declaration
                TokenKind::Colon => Ok(false),
                // Left brace after identifier = nested rule (e.g., "div {")
                TokenKind::LeftBrace => Ok(true),
                // Selector tokens after identifier = nested rule
                TokenKind::Dot | TokenKind::Hash | TokenKind::LeftBracket => Ok(true),
                // Other tokens - likely nested rule
                _ => Ok(true),
            }
        }

        _ => Ok(false),
    }
}

/// Parse a CSS rule: `selector { property: value; }`
pub(crate) fn parse_rule(parser: &mut CssParser) -> Result<CssRule, ParseError> {
    let start = parser.base_offset() + parser.current_start;

    // Parse selector list (complex selectors for top-level rules)
    // Top-level rules use strict parsing (not forgiving like :is/:where)
    let selector = super::selectors::parse_complex_selector_list(parser)?;

    // Capture any comment after selector (before {)
    let mut declarations = Vec::new();
    parser.skip_whitespace()?;
    if let TokenKind::Comment(content) = &parser.current_kind {
        let comment_start = parser.base_offset() + parser.current_start;
        let comment_end = parser.base_offset() + parser.current_end;
        let content = content.clone();

        parser.advance()?;
        parser.skip_whitespace()?;

        // Store comment as first child (will be formatted before opening brace)
        declarations.push(CssBlockChild::Comment(CssComment {
            content,
            span: Span {
                start: comment_start as u32,
                end: comment_end as u32,
            },
        }));
    }

    // Expect { and capture its start
    let block_start = parser.base_offset() + parser.current_start;
    parser.expect(&TokenKind::LeftBrace)?;
    parser.skip_whitespace()?;

    // Parse declarations, comments, and nested rules
    while !parser.check(&TokenKind::RightBrace) && !parser.check(&TokenKind::Eof) {
        // Capture comments in declaration blocks
        if let TokenKind::Comment(content) = &parser.current_kind {
            let comment_start = parser.base_offset() + parser.current_start;
            let comment_end = parser.base_offset() + parser.current_end;
            let content = content.clone();

            parser.advance()?;
            parser.skip_whitespace()?;

            declarations.push(CssBlockChild::Comment(CssComment {
                content,
                span: Span {
                    start: comment_start as u32,
                    end: comment_end as u32,
                },
            }));
            continue;
        }

        // Check for nested at-rule (CSS Nesting Module)
        if parser.check(&TokenKind::AtSign) {
            // Parse nested at-rule (e.g., @media inside a rule)
            // Pass true for nested_in_rule since we're inside a regular rule's declaration block
            let nested_atrule = super::atrules::parse_atrule(parser, true)?;
            declarations.push(CssBlockChild::Atrule(nested_atrule));
            parser.skip_whitespace()?;
            continue;
        }

        // Check if we're looking at a nested rule (CSS Nesting Module)
        if is_nested_rule_start(parser)? {
            // Parse nested rule recursively
            let nested_rule = parse_rule(parser)?;
            declarations.push(CssBlockChild::Rule(nested_rule));
            parser.skip_whitespace()?;
            continue;
        }

        // Otherwise, parse as declaration
        if parser.check(&TokenKind::Identifier) {
            let decl = parse_declaration(parser)?;
            declarations.push(CssBlockChild::Declaration(decl));
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

    parser.skip_whitespace_and_comments()?;

    // Expect :
    parser.expect(&TokenKind::Colon)?;
    // Only skip whitespace, NOT comments - comments in values need to be preserved
    parser.skip_whitespace()?;

    // Track value start and end for span calculation
    let value_start = parser.base_offset() + parser.current_start;

    // Parse value (collect tokens until ; or })
    // IMPORTANT: Track parenthesis depth to handle semicolons inside functions like url(data:image/png;base64,...)
    let mut value_parts = Vec::new();
    let mut value_comments = Vec::new(); // Collect comments found in the value
    let mut value_end = value_start;
    // Track recent end positions for !important stripping (stores ends for last 2 tokens)
    let mut prev_value_end = value_start;
    let mut prev_prev_value_end = value_start;
    let mut paren_depth: i32 = 0; // Track nesting level of parentheses
    while !parser.check(&TokenKind::Eof)
        && !(paren_depth == 0
            && (parser.check(&TokenKind::Semicolon) || parser.check(&TokenKind::RightBrace)))
    {
        // Convert token to string representation for value
        let value_str = match &parser.current_kind {
            // Internal AST: use decoded value (spec-compliant)
            TokenKind::Identifier => parser
                .current_identifier()
                .unwrap_or_else(|| parser.current_value())
                .to_string(),
            TokenKind::String { content, quote } => format!("{quote}{content}{quote}"),
            TokenKind::Number(n) => n.clone(),
            TokenKind::Percentage(n) => format!("{n}%"),
            TokenKind::Dimension(n, unit) => format!("{n}{unit}"),
            TokenKind::Whitespace => {
                parser.advance()?;
                continue;
            }
            TokenKind::Comment(content) => {
                // Capture comment for value comments side table
                let comment_start = parser.base_offset() + parser.current_start;
                let comment_end = parser.base_offset() + parser.current_end;
                value_comments.push(CssComment {
                    content: content.clone(),
                    span: Span {
                        start: comment_start as u32,
                        end: comment_end as u32,
                    },
                });
                // Update value_end to include the comment in the declaration span
                prev_prev_value_end = prev_value_end;
                prev_value_end = value_end;
                value_end = parser.base_offset() + parser.current_end;
                parser.advance()?;
                continue;
            }
            TokenKind::LeftParen => {
                paren_depth += 1;
                parser.current_value().to_string()
            }
            TokenKind::RightParen => {
                paren_depth = paren_depth.saturating_sub(1);
                parser.current_value().to_string()
            }
            TokenKind::Bang => "!".to_string(),
            _ => {
                // Other tokens - include them as-is from source
                parser.current_value().to_string()
            }
        };

        value_parts.push(value_str);
        prev_prev_value_end = prev_value_end;
        prev_value_end = value_end;
        value_end = parser.base_offset() + parser.current_end;
        parser.advance()?;
    }

    // Check for !important at the end of value
    let important = if value_parts.len() >= 2 {
        let last = value_parts.last().map(String::as_str);
        let second_last = value_parts.get(value_parts.len() - 2).map(String::as_str);
        if second_last == Some("!") && last.is_some_and(|s| s.eq_ignore_ascii_case("important")) {
            // Remove !important from value parts and adjust value_end
            value_parts.pop();
            value_parts.pop();
            value_end = prev_prev_value_end;
            true
        } else {
            false
        }
    } else {
        false
    };

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

    // Allow empty value_str if we have comments (e.g., `color: /* comment */;`)
    // Svelte treats the comment as the value in this case
    if value_str.is_empty() && value_comments.is_empty() {
        return Err(ParseError::InvalidSyntax {
            message: "Empty CSS value".to_string(),
            position: start,
            context: None,
        });
    }

    // Create span for the value (from first token to last token, excluding comments)
    let value_span = Span {
        start: value_start as u32,
        end: value_end as u32,
    };

    // Parse the value directly from source for accurate span tracking
    // Use parse_value_from_source instead of parse_value_string to avoid
    // span drift from whitespace differences between source and reconstructed tokens
    //
    // Convert absolute span to source-relative span (subtract base_offset)
    let base = parser.base_offset() as u32;
    let source_relative_span = Span {
        start: value_span.start - base,
        end: value_span.end - base,
    };

    // Custom properties (--*) with unusual values (e.g., leading comma) preserve raw value
    // Normal custom property values are still parsed for proper formatting
    let raw_value =
        &parser.source()[source_relative_span.start as usize..source_relative_span.end as usize];
    let trimmed_value = raw_value.trim();

    let value = if property.starts_with("--") && trimmed_value.starts_with(',') {
        // Leading comma is unusual syntax - preserve as raw identifier
        CssValue::Identifier {
            name: trimmed_value.to_string(),
            span: value_span,
        }
    } else {
        super::value::parse_value_from_source(parser.source(), source_relative_span, base)
    };

    // Declaration ends after the value, NOT including the semicolon
    let end = value_end;

    // Optionally consume semicolon (but don't include it in the declaration span)
    if parser.check(&TokenKind::Semicolon) {
        parser.advance()?;
    }

    let decl_span = Span {
        start: start as u32,
        end: end as u32,
    };

    // Store value comments in side table if any were found
    if !value_comments.is_empty() {
        parser
            .value_comments
            .insert(decl_span.start, value_comments);
    }

    // Span covers the entire declaration (property + value, not including semicolon)
    // The source value will be extracted on-demand during conversion using this span
    Ok(CssDeclaration {
        property,
        value,
        important,
        span: decl_span,
    })
}
