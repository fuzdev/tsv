use super::CssParser;
use super::selectors::{
    parse_complex_selector_list, parse_forgiving_selector_list, parse_relative_selector_list,
};
use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

/// Parse pseudo-class or pseudo-element: :hover, ::before, :nth-child(2n+1)
pub(crate) fn parse_pseudo_selector<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    start: usize,
) -> Result<SimpleSelector<'arena>, ParseError> {
    parser.advance()?; // consume first :

    // Check for :: (pseudo-element)
    let is_pseudo_element = parser.check(TokenKind::Colon);
    if is_pseudo_element {
        parser.advance()?; // consume second :
    }

    if !parser.check(TokenKind::Identifier) {
        return Err(parser.error_expected("pseudo-class or pseudo-element name"));
    }

    // Decode the name only for parse-time argument dispatch (`:nth-child` → Nth,
    // `:is`/`:not` → SelectorList). It is NOT stored on the node: the name is recovered
    // from `span` at convert/print time (half-decoded to match Svelte — see convert/mod.rs),
    // so storing a fully-decoded copy would be redundant and, for identity escapes, wrong.
    // The arena copy is needed because `current_identifier()` borrows a buffer that
    // `advance()` overwrites, and dispatch happens after the name token is consumed.
    let name_ident = parser
        .current_identifier()
        .unwrap_or_else(|| parser.current_value());
    let name = parser.alloc_str_in(name_ident);
    let mut end = (parser.base_offset() + parser.current_end) as u32; // Capture end of name token
    parser.advance()?;

    // Check for arguments: :nth-child(2n+1), :is(), :not(), etc.
    let args = if parser.check(TokenKind::LeftParen) {
        let (args_opt, args_end) = parse_pseudo_args(parser, name)?;
        end = args_end; // Use end of closing paren
        args_opt
    } else {
        None
    };

    if is_pseudo_element {
        Ok(SimpleSelector::PseudoElement {
            args,
            span: Span {
                start: start as u32,
                end,
            },
        })
    } else {
        Ok(SimpleSelector::PseudoClass {
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
/// - :is(), :not(), :where(), :has(), :global() → PseudoClassArgs::SelectorList
/// - Others: returns None (unknown pseudo-classes)
fn parse_pseudo_args<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    pseudo_name: &str,
) -> Result<(Option<PseudoClassArgs<'arena>>, u32), ParseError> {
    parser.expect(TokenKind::LeftParen)?;

    let args_start = parser.current_start;

    // Parse arguments for ::slotted() pseudo-element
    //
    // Per CSS Scoping Module Level 1: `::slotted( <compound-selector> )`
    // A compound selector is a sequence of simple selectors without combinators.
    if pseudo_name == "slotted" {
        parser.skip_whitespace_and_comments()?;

        // Parse compound selector (sequence of simple selectors, no combinators)
        let compound_start = parser.current_start;
        let mut compound_selectors = parser.bvec();

        // Parse simple selectors until we hit a combinator or closing paren
        while !parser.check(TokenKind::RightParen) && !parser.check(TokenKind::Eof) {
            parser.skip_whitespace_and_comments()?;

            if parser.check(TokenKind::RightParen) {
                break;
            }

            // Check for combinators (not allowed in compound selectors)
            if parser.check(TokenKind::GreaterThan)
                || parser.check(TokenKind::Plus)
                || parser.check(TokenKind::Tilde)
                || parser.check(TokenKind::ColumnCombinator)
            {
                return Err(
                    parser.error_msg("Combinators not allowed in ::slotted() compound selector")
                );
            }

            // Try to parse one simple selector
            // This will fail if we hit something invalid (like a descendant combinator)
            let selector = super::selectors::parse_simple_selector(parser)?;
            compound_selectors.push(selector);

            parser.skip_whitespace_and_comments()?;
        }

        if compound_selectors.is_empty() {
            return Err(parser.error_msg_at(
                "::slotted() requires a compound selector argument",
                parser.base_offset() + compound_start,
            ));
        }

        let end = parser.expect_and_capture(TokenKind::RightParen)?;

        return Ok((
            Some(PseudoClassArgs::Slotted {
                selectors: compound_selectors.into_bump_slice(),
                span: Span {
                    start: (parser.base_offset() + args_start) as u32,
                    end,
                },
            }),
            end,
        ));
    }

    // Parse arguments for ::part() pseudo-element
    //
    // Per CSS Shadow Parts Specification: `::part( <ident>+ )`
    // One or more space-separated identifiers (NOT selectors).
    if pseudo_name == "part" {
        parser.skip_whitespace_and_comments()?;

        let mut idents = parser.bvec();

        // Parse space-separated identifiers
        while !parser.check(TokenKind::RightParen) && !parser.check(TokenKind::Eof) {
            if !parser.check(TokenKind::Identifier) {
                return Err(parser.error_msg("::part() requires identifier arguments"));
            }

            let ident_str = parser
                .current_identifier()
                .unwrap_or_else(|| parser.current_value());
            let ident = parser.alloc_str_in(ident_str);
            idents.push(ident);
            parser.advance()?;

            parser.skip_whitespace_and_comments()?;
        }

        if idents.is_empty() {
            return Err(parser.error_msg_at(
                "::part() requires at least one identifier",
                parser.base_offset() + args_start,
            ));
        }

        let end = parser.expect_and_capture(TokenKind::RightParen)?;

        return Ok((
            Some(PseudoClassArgs::Part {
                idents: idents.into_bump_slice(),
                span: Span {
                    start: (parser.base_offset() + args_start) as u32,
                    end,
                },
            }),
            end,
        ));
    }

    // Parse identifier arguments for spec-compliant pseudo-classes/elements
    //
    // These take single identifiers per CSS spec:
    // - :dir(ltr | rtl) → direction identifier
    // - :lang(en-US) → language code
    // - ::highlight(search-results) → custom highlight name
    //
    // Note: Svelte's parser quirk treats these as selectors in public AST (handled at conversion)
    if matches!(pseudo_name, "dir" | "lang" | "highlight") {
        parser.skip_whitespace_and_comments()?;

        // Parse identifier (or consume tokens until closing paren)
        let ident_start = parser.current_start;
        let mut ident_parts = Vec::new();

        // Collect tokens that form the identifier (may include hyphens, etc.)
        while !parser.check(TokenKind::RightParen) && !parser.check(TokenKind::Eof) {
            if parser.check(TokenKind::Whitespace) {
                parser.advance()?;
                continue;
            }
            ident_parts.push(parser.current_value().to_string());
            parser.advance()?;
        }

        let ident_value = ident_parts.join("");
        let ident_end = parser.current_start;

        let paren_end = parser.expect_and_capture(TokenKind::RightParen)?;

        return Ok((
            Some(PseudoClassArgs::Identifier {
                value: parser.alloc_str_in(&ident_value),
                span: Span {
                    start: (parser.base_offset() + ident_start) as u32,
                    end: (parser.base_offset() + ident_end) as u32,
                },
            }),
            paren_end,
        ));
    }

    // Parse selector list for logical pseudo-classes
    //
    // Per CSS Selectors Level 4, each pseudo-class accepts different selector types:
    // - :has() → <<relative-selector-list>> (can start with combinators: `:has(> img)`)
    // - :is(), :where() → <<forgiving-selector-list>> (invalid selectors wrapped as Invalid, not failed)
    // - :not() → <<complex-real-selector-list>> (complex selectors, strict parsing)
    // - :global() → Svelte-specific, uses complex selectors
    if matches!(pseudo_name, "is" | "not" | "where" | "has" | "global") {
        parser.skip_whitespace_and_comments()?;

        let selector_list = match pseudo_name {
            "has" => {
                // :has() uses relative selectors (can start with combinators)
                parse_relative_selector_list(parser)?
            }
            "is" | "where" => {
                // :is() and :where() use forgiving selector lists
                // Invalid selectors wrapped as SimpleSelector::Invalid (never fails)
                parse_forgiving_selector_list(parser)?
            }
            "not" | "global" => {
                // :not() and :global() use complex selectors (strict parsing)
                parse_complex_selector_list(parser)?
            }
            _ => unreachable!(),
        };

        parser.skip_whitespace_and_comments()?;

        let end = parser.expect_and_capture(TokenKind::RightParen)?;

        return Ok((
            Some(PseudoClassArgs::SelectorList {
                selectors: selector_list,
                span: Span {
                    start: (parser.base_offset() + args_start) as u32,
                    end,
                },
            }),
            end,
        ));
    }

    // For nth-* pseudo-classes, parse An+B notation and optional "of <selector-list>"
    // Per CSS Selectors Level 4: :nth-child(An+B [of S]?)
    let args = match pseudo_name {
        "nth-child" | "nth-of-type" | "nth-last-child" | "nth-last-of-type" | "nth-col"
        | "nth-last-col" => {
            // Parse An+B part (collect tokens until "of" keyword or closing paren)
            parser.skip_whitespace_and_comments()?;
            let anb_start = parser.current_start;

            // Scan tokens until we hit "of" keyword or closing paren
            let mut anb_end = anb_start;
            let mut found_of = false;
            let mut depth = 0;

            while !parser.check(TokenKind::Eof) {
                if parser.check(TokenKind::RightParen) && depth == 0 {
                    // End of nth args
                    break;
                } else if parser.check(TokenKind::LeftParen) {
                    depth += 1;
                    anb_end = parser.current_end;
                    parser.advance()?;
                } else if parser.check(TokenKind::RightParen) {
                    depth -= 1;
                    anb_end = parser.current_end;
                    parser.advance()?;
                } else if parser.check(TokenKind::Identifier) && depth == 0 {
                    let ident = parser
                        .current_identifier()
                        .unwrap_or_else(|| parser.current_value());
                    if ident == "of" {
                        // Found "of" keyword - An+B part ends here
                        found_of = true;
                        parser.advance()?; // consume "of"
                        break;
                    }
                    // Part of An+B (e.g., "odd", "even", "n")
                    anb_end = parser.current_end;
                    parser.advance()?;
                } else {
                    // Other tokens (numbers, +, -, whitespace, etc.)
                    anb_end = parser.current_end;
                    parser.advance()?;
                }
            }

            // Extract An+B value
            let anb_value = parser.alloc_str_in(parser.source()[anb_start..anb_end].trim());

            // Parse optional selector list after "of"
            let of_selector = if found_of {
                parser.skip_whitespace_and_comments()?;
                Some(parse_complex_selector_list(parser)?)
            } else {
                None
            };

            parser.skip_whitespace_and_comments()?;
            let span_end = (parser.base_offset() + parser.current_start) as u32; // End before closing paren
            let paren_end = parser.expect_and_capture(TokenKind::RightParen)?; // End after closing paren

            (
                Some(PseudoClassArgs::Nth {
                    value: anb_value,
                    of_selector,
                    span: Span {
                        start: (parser.base_offset() + args_start) as u32,
                        end: span_end,
                    },
                }),
                paren_end,
            )
        }
        _ => {
            // Unknown pseudo-class/pseudo-element - try parsing as selector list
            // This handles generic pseudo-classes like :current(), :state(), etc.
            // and pseudo-elements like ::cue(), ::highlight(), etc.
            //
            // Approach: Try to parse as selector list first (most common case)
            // If that fails, skip the arguments (for pseudo-classes with non-selector args)
            parser.skip_whitespace_and_comments()?;

            // Try to parse as a selector list (complex selectors, strict parsing)
            let selector_result = parse_complex_selector_list(parser);

            match selector_result {
                Ok(selector_list) => {
                    parser.skip_whitespace_and_comments()?;
                    let end = parser.expect_and_capture(TokenKind::RightParen)?;

                    (
                        Some(PseudoClassArgs::SelectorList {
                            selectors: selector_list,
                            span: Span {
                                start: (parser.base_offset() + args_start) as u32,
                                end,
                            },
                        }),
                        end,
                    )
                }
                Err(_) => {
                    // Parsing as selector list failed - skip arguments
                    // This handles pseudo-classes with non-selector arguments
                    let mut depth = 1;
                    while depth > 0 && !parser.check(TokenKind::Eof) {
                        if parser.check(TokenKind::LeftParen) {
                            depth += 1;
                        } else if parser.check(TokenKind::RightParen) {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        if depth > 0 {
                            parser.advance()?;
                        }
                    }
                    let end = parser.expect_and_capture(TokenKind::RightParen)?;
                    (None, end)
                }
            }
        }
    };

    Ok(args)
}
