use super::CssParser;
use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

/// Parse pseudo-class or pseudo-element: :hover, ::before, :nth-child(2n+1)
pub(crate) fn parse_pseudo_selector(
    parser: &mut CssParser,
    start: usize,
) -> Result<SimpleSelector, ParseError> {
    parser.advance()?; // consume first :

    // Check for :: (pseudo-element)
    let is_pseudo_element = parser.check(&TokenKind::Colon);
    if is_pseudo_element {
        parser.advance()?; // consume second :
    }

    if !parser.check(&TokenKind::Identifier) {
        return Err(ParseError::InvalidSyntax {
            message: "Expected pseudo-class or pseudo-element name".to_string(),
            position: parser.base_offset() + parser.current_start,
            context: None,
        });
    }

    // Internal AST: use decoded value (spec-compliant)
    let name = parser
        .current_identifier()
        .unwrap_or_else(|| parser.current_value())
        .to_string();
    let mut end = (parser.base_offset() + parser.current_end) as u32; // Capture end of name token
    parser.advance()?;

    // Check for arguments: :nth-child(2n+1), :is(), :not(), etc.
    let args = if parser.check(&TokenKind::LeftParen) {
        let (args_opt, args_end) = parse_pseudo_args(parser, &name)?;
        end = args_end; // Use end of closing paren
        args_opt
    } else {
        None
    };

    if is_pseudo_element {
        Ok(SimpleSelector::PseudoElement {
            name,
            args,
            span: Span {
                start: start as u32,
                end,
            },
        })
    } else {
        Ok(SimpleSelector::PseudoClass {
            name,
            args,
            span: Span {
                start: start as u32,
                end,
            },
        })
    }
}

/// Parse pseudo-class arguments: nth-child(2n+1), is(div, span)
/// Returns (Option<PseudoClassArgs>, end_position_of_closing_paren)
///
/// Creates semantic args for recognized pseudo-classes:
/// - :nth-child(), :nth-of-type(), :nth-last-child(), :nth-last-of-type() → PseudoClassArgs::Nth
/// - Others: returns None (deferred for future implementation)
fn parse_pseudo_args(
    parser: &mut CssParser,
    pseudo_name: &str,
) -> Result<(Option<PseudoClassArgs>, u32), ParseError> {
    parser.expect(&TokenKind::LeftParen)?;

    let args_start = parser.current_start;

    // Skip to matching right paren, tracking depth
    let mut depth = 1;
    while depth > 0 && !parser.check(&TokenKind::Eof) {
        if parser.check(&TokenKind::LeftParen) {
            depth += 1;
        } else if parser.check(&TokenKind::RightParen) {
            depth -= 1;
        }
        if depth > 0 {
            parser.advance()?;
        }
    }

    let args_end = parser.current_start;
    let raw_text = parser.source()[args_start..args_end].to_string();

    // Capture end of closing paren before advancing
    let end = parser.expect_and_capture(&TokenKind::RightParen)?;

    // Create semantic args based on pseudo-class type
    let args = match pseudo_name {
        "nth-child" | "nth-of-type" | "nth-last-child" | "nth-last-of-type" => {
            Some(PseudoClassArgs::Nth {
                value: raw_text.trim().to_string(),
                span: Span {
                    start: (parser.base_offset() + args_start) as u32,
                    end: (parser.base_offset() + args_end) as u32,
                },
            })
        }
        _ => {
            // For other pseudo-classes (:is, :not, :where, :has, etc.), return None for now
            // Future: parse these into structured args
            None
        }
    };

    Ok((args, end))
}
