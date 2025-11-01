use super::CssParser;
use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

/// Parse a CSS at-rule: `@media (...) { ... }` or `@import "...";`
pub(crate) fn parse_atrule(parser: &mut CssParser) -> Result<CssAtrule, ParseError> {
    let start = parser.base_offset() + parser.current_start;

    // Expect @ symbol
    parser.expect(&TokenKind::AtSign)?;

    // Parse at-rule name (identifier after @)
    if !parser.check(&TokenKind::Identifier) {
        return Err(ParseError::InvalidSyntax {
            message: "Expected at-rule name after @".to_string(),
            position: parser.base_offset() + parser.current_start,
            context: None,
        });
    }

    let name = parser.current_value().to_string();
    parser.advance()?;

    parser.skip_whitespace()?;

    // Parse prelude (raw string until { or ;)
    let mut prelude_parts = Vec::new();

    while !parser.check(&TokenKind::LeftBrace)
        && !parser.check(&TokenKind::Semicolon)
        && !parser.check(&TokenKind::Eof)
    {
        if parser.check(&TokenKind::Whitespace) {
            prelude_parts.push(" ".to_string());
            parser.advance()?;
            continue;
        }

        let part = match &parser.current_kind {
            TokenKind::Identifier => parser.current_value().to_string(),
            TokenKind::String { content, quote } => format!("{}{}{}", quote, content, quote),
            TokenKind::Number(n) => n.to_string(),
            TokenKind::Percentage(n) => format!("{}%", n),
            TokenKind::Dimension(n, unit) => format!("{}{}", n, unit),
            TokenKind::Comment(_) => {
                // Skip comments in prelude
                parser.advance()?;
                continue;
            }
            _ => parser.current_value().to_string(),
        };

        prelude_parts.push(part);
        parser.advance()?;
    }

    let prelude = prelude_parts.join("").trim().to_string();

    // Parse block (if present)
    let block = if parser.check(&TokenKind::LeftBrace) {
        let block = parse_atrule_block(parser, &name)?;
        Some(block)
    } else if parser.check(&TokenKind::Semicolon) {
        // Statement at-rule (no block)
        let end = parser.base_offset() + parser.current_end;
        parser.advance()?;
        return Ok(CssAtrule {
            name,
            prelude,
            block: None,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        });
    } else {
        return Err(ParseError::InvalidSyntax {
            message: "Expected '{' or ';' after at-rule prelude".to_string(),
            position: parser.base_offset() + parser.current_start,
            context: None,
        });
    };

    let end = (parser.base_offset() + parser.current_end) as u32;

    Ok(CssAtrule {
        name,
        prelude,
        block,
        span: Span {
            start: start as u32,
            end,
        },
    })
}

/// Parse an at-rule block: `{ ... }`
/// Block contents depend on at-rule type:
/// - @media, @supports, @layer: contain rules
/// - @keyframes: contains keyframe blocks (rules with percentage/from/to selectors)
/// - @font-face, @page: contain declarations
fn parse_atrule_block(
    parser: &mut CssParser,
    atrule_name: &str,
) -> Result<CssAtruleBlock, ParseError> {
    let start = parser.base_offset() + parser.current_start;

    // Expect {
    parser.expect(&TokenKind::LeftBrace)?;
    parser.skip_whitespace()?;

    let mut children = Vec::new();

    // Determine what content to expect based on at-rule type
    let expect_rules = matches!(atrule_name, "media" | "supports" | "layer" | "keyframes");
    let expect_declarations = matches!(atrule_name, "font-face" | "page");

    while !parser.check(&TokenKind::RightBrace) && !parser.check(&TokenKind::Eof) {
        // Handle comments
        if let TokenKind::Comment(content) = &parser.current_kind {
            let comment_start = parser.base_offset() + parser.current_start;
            let comment_end = parser.base_offset() + parser.current_end;
            let content = content.clone();

            parser.advance()?;
            parser.skip_whitespace()?;

            children.push(CssBlockChild::Comment(CssComment {
                content,
                span: Span {
                    start: comment_start as u32,
                    end: comment_end as u32,
                },
            }));
            continue;
        }

        // Handle nested at-rules
        if parser.check(&TokenKind::AtSign) {
            let atrule = parse_atrule(parser)?;
            children.push(CssBlockChild::Atrule(atrule));
            parser.skip_whitespace()?;
            continue;
        }

        // For @font-face and @page, parse declarations
        if expect_declarations && parser.check(&TokenKind::Identifier) {
            let decl = super::declarations::parse_declaration(parser)?;
            children.push(CssBlockChild::Declaration(decl));
            parser.skip_whitespace()?;
            continue;
        }

        // For @media, @supports, @layer, @keyframes, parse rules
        if expect_rules {
            let rule = super::declarations::parse_rule(parser)?;
            children.push(CssBlockChild::Rule(rule));
            parser.skip_whitespace()?;
            continue;
        }

        // Generic fallback: default to expecting rules for unknown at-rules
        let rule = super::declarations::parse_rule(parser)?;
        children.push(CssBlockChild::Rule(rule));
        parser.skip_whitespace()?;
        continue;
    }

    // Expect }
    if !parser.check(&TokenKind::RightBrace) {
        return Err(ParseError::InvalidSyntax {
            message: "Expected '}'".to_string(),
            position: parser.base_offset() + parser.current_start,
            context: None,
        });
    }
    let end = parser.base_offset() + parser.current_end;
    parser.advance()?; // consume }

    Ok(CssAtruleBlock {
        children,
        span: Span {
            start: start as u32,
            end: end as u32,
        },
    })
}
