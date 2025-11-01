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

    let name = parser.current_value().to_string();
    let mut end = (parser.base_offset() + parser.current_end) as u32; // Capture end of name token
    parser.advance()?;

    // Check for arguments: :nth-child(2n+1), :is(), :not(), etc.
    let raw_args = if parser.check(&TokenKind::LeftParen) {
        let (args, args_end) = parse_pseudo_args(parser)?;
        end = args_end; // Use end of closing paren
        Some(args)
    } else {
        None
    };

    if is_pseudo_element {
        Ok(SimpleSelector::PseudoElement {
            name,
            raw_args,
            span: Span {
                start: start as u32,
                end,
            },
        })
    } else {
        Ok(SimpleSelector::PseudoClass {
            name,
            raw_args,
            span: Span {
                start: start as u32,
                end,
            },
        })
    }
}

/// Parse pseudo-class arguments: nth-child(2n+1), is(div, span)
/// Returns (raw_argument_text, end_position_of_closing_paren)
fn parse_pseudo_args(parser: &mut CssParser) -> Result<(String, u32), ParseError> {
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
    let raw_args = parser.source()[args_start..args_end].to_string();

    // Capture end of closing paren before advancing
    let end = parser.expect_and_capture(&TokenKind::RightParen)?;

    Ok((raw_args, end))
}
