use super::CssParser;
use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

/// Parse a selector list: `div, span, .class`
pub(crate) fn parse_selector_list(parser: &mut CssParser) -> Result<SelectorList, ParseError> {
    let start = parser.base_offset() + parser.current_start();
    let mut selectors = Vec::new();

    // Parse first complex selector
    selectors.push(parse_complex_selector(parser)?);

    // Parse additional selectors separated by commas
    while parser.check(&TokenKind::Comma) {
        parser.advance()?; // consume comma
        parser.skip_whitespace()?;
        selectors.push(parse_complex_selector(parser)?);
    }

    // End position should be the end of the last selector, not the next token
    let end = selectors.last().unwrap().span.end;

    Ok(SelectorList {
        selectors,
        span: Span {
            start: start as u32,
            end,
        },
    })
}

/// Parse a complex selector: `div > span + .class`
pub(crate) fn parse_complex_selector(
    parser: &mut CssParser,
) -> Result<ComplexSelector, ParseError> {
    let start = parser.base_offset() + parser.current_start();
    let mut children = Vec::new();

    // First relative selector has no combinator
    children.push(parse_relative_selector(parser, None)?);

    // Parse additional relative selectors with combinators
    while !parser.check(&TokenKind::LeftBrace)
        && !parser.check(&TokenKind::Comma)
        && !parser.check(&TokenKind::Eof)
    {
        // Check for combinator
        let combinator = parse_combinator(parser)?;
        if combinator.is_none() {
            break; // No more combinators, we're done
        }

        children.push(parse_relative_selector(parser, combinator)?);
    }

    // End position should be the end of the last child, not the next token
    let end = children.last().unwrap().span.end;

    Ok(ComplexSelector {
        children,
        span: Span {
            start: start as u32,
            end,
        },
    })
}

/// Parse a combinator: `>`, `+`, `~`, `||`, or whitespace (descendant)
pub(crate) fn parse_combinator(parser: &mut CssParser) -> Result<Option<Combinator>, ParseError> {
    parser.skip_whitespace()?;

    let combinator = match &parser.current_kind {
        TokenKind::GreaterThan => Some(Combinator::Child),
        TokenKind::Plus => Some(Combinator::NextSibling),
        TokenKind::Tilde => Some(Combinator::SubsequentSibling),
        // TODO: Column combinator (||) - requires two-token lookahead
        _ => {
            // Check if we had whitespace before (descendant combinator)
            // For now, peek ahead to see if there's another selector coming
            if is_selector_start(parser) {
                Some(Combinator::Descendant)
            } else {
                None
            }
        }
    };

    if combinator.is_some() && combinator != Some(Combinator::Descendant) {
        parser.advance()?; // consume combinator token
        parser.skip_whitespace()?;
    }

    Ok(combinator)
}

/// Check if current token could start a selector
fn is_selector_start(parser: &CssParser) -> bool {
    matches!(
        parser.current_kind,
        TokenKind::Identifier
            | TokenKind::Dot
            | TokenKind::Hash
            | TokenKind::Asterisk
            | TokenKind::Colon
            | TokenKind::LeftBracket
            | TokenKind::Ampersand
    )
}

/// Parse a relative selector: combinator + simple selectors
fn parse_relative_selector(
    parser: &mut CssParser,
    combinator: Option<Combinator>,
) -> Result<RelativeSelector, ParseError> {
    let start = parser.base_offset() + parser.current_start();
    let mut selectors = Vec::new();

    // Parse one or more simple selectors
    loop {
        let simple = parse_simple_selector(parser)?;
        selectors.push(simple);

        // Check if another simple selector follows (no whitespace, no combinator)
        if !is_simple_selector_chain(parser) {
            break;
        }
    }

    let end = parser.base_offset() + parser.current_start();

    if selectors.is_empty() {
        return Err(ParseError::InvalidSyntax {
            message: "Expected selector".to_string(),
            position: start,
            context: None,
        });
    }

    Ok(RelativeSelector {
        combinator,
        selectors,
        span: Span {
            start: start as u32,
            end: end as u32,
        },
    })
}

/// Check if another simple selector follows in the chain (e.g., `div.class#id`)
fn is_simple_selector_chain(parser: &CssParser) -> bool {
    matches!(
        parser.current_kind,
        TokenKind::Dot | TokenKind::Hash | TokenKind::Colon | TokenKind::LeftBracket
    )
}

/// Parse a simple selector: type, class, id, attribute, pseudo-class, pseudo-element
pub(crate) fn parse_simple_selector(parser: &mut CssParser) -> Result<SimpleSelector, ParseError> {
    let start = parser.base_offset() + parser.current_start();

    match &parser.current_kind {
        TokenKind::Identifier => {
            // Type selector: div, span, etc.
            let name = parser.current_value().to_string();
            let end = parser.base_offset() + parser.current_end;
            parser.advance()?;
            Ok(SimpleSelector::Type {
                namespace: None, // TODO: Parse namespace (svg|rect)
                name,
                span: Span {
                    start: start as u32,
                    end: end as u32,
                },
            })
        }
        TokenKind::Dot => {
            // Class selector: .class
            parser.advance()?; // consume .
            if !parser.check(&TokenKind::Identifier) {
                return Err(ParseError::InvalidSyntax {
                    message: "Expected class name after '.'".to_string(),
                    position: parser.base_offset() + parser.current_start(),
                    context: None,
                });
            }
            let name = parser.current_value().to_string();
            let end = parser.base_offset() + parser.current_end;
            parser.advance()?;
            Ok(SimpleSelector::Class {
                name,
                span: Span {
                    start: start as u32,
                    end: end as u32,
                },
            })
        }
        TokenKind::Hash => {
            // ID selector: #id
            parser.advance()?; // consume #
            if !parser.check(&TokenKind::Identifier) {
                return Err(ParseError::InvalidSyntax {
                    message: "Expected ID name after '#'".to_string(),
                    position: parser.base_offset() + parser.current_start(),
                    context: None,
                });
            }
            let name = parser.current_value().to_string();
            let end = parser.base_offset() + parser.current_end;
            parser.advance()?;
            Ok(SimpleSelector::Id {
                name,
                span: Span {
                    start: start as u32,
                    end: end as u32,
                },
            })
        }
        TokenKind::Asterisk => {
            // Universal selector: *
            let end = parser.base_offset() + parser.current_end;
            parser.advance()?;
            Ok(SimpleSelector::Universal {
                namespace: None, // TODO: Parse namespace (*|div)
                span: Span {
                    start: start as u32,
                    end: end as u32,
                },
            })
        }
        TokenKind::Colon => {
            // Pseudo-class or pseudo-element
            super::pseudo::parse_pseudo_selector(parser, start)
        }
        TokenKind::LeftBracket => {
            // Attribute selector: [attr], [attr="value"]
            super::attributes::parse_attribute_selector(parser, start)
        }
        TokenKind::Ampersand => {
            // Nesting selector: &
            let end = parser.base_offset() + parser.current_end;
            parser.advance()?;
            Ok(SimpleSelector::Nesting {
                span: Span {
                    start: start as u32,
                    end: end as u32,
                },
            })
        }
        TokenKind::Percentage(value_str) => {
            // Percentage selector: 0%, 50%, 100% (used in @keyframes)
            let value = value_str.parse::<f64>().map_err(|_| ParseError::InvalidSyntax {
                message: format!("Invalid percentage value: {}", value_str),
                position: start,
                context: None,
            })?;
            let end = parser.base_offset() + parser.current_end;
            parser.advance()?;
            Ok(SimpleSelector::Percentage {
                value,
                span: Span {
                    start: start as u32,
                    end: end as u32,
                },
            })
        }
        _ => Err(ParseError::InvalidSyntax {
            message: format!("Unexpected token in selector: {:?}", parser.current_kind),
            position: start,
            context: None,
        }),
    }
}
