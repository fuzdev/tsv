use super::CssParser;
use crate::ast::internal::*;
use crate::lexer::TokenKind;
use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;
use tsv_lang::{ParseError, Span};

/// Parse a complex selector list: `div, span > a, .class#id`
///
/// This parses a comma-separated list of complex selectors.
/// Complex selectors can contain combinators (>, +, ~, descendant) but CANNOT start with them.
///
/// Used in:
/// - Top-level CSS rules: `div, span { }`
/// - :is(), :where(), :not() pseudo-classes (they accept complex selectors but not relative selectors)
///
/// For selectors that CAN start with combinators, use `parse_relative_selector_list()` instead.
/// See: CSS Selectors Level 4 - <<complex-selector-list>> vs <<relative-selector-list>>
/// Register comment(s) sitting between a complex selector and its `,` separator
/// (comments are inter-token whitespace per css-syntax-3) — but only when a comma
/// actually follows, so a trailing comment before `{` is left for `parse_rule` (it
/// sits outside the list span and is inline-printed as a pre-brace comment). The
/// lookahead is non-destructive.
///
/// The comments are **registered** (not dropped) so the printer can interleave them
/// at the comma boundary via `comments_in_range` — the principled replacement for the
/// old raw-source selector seam. Not needed by `parse_forgiving_selector_list`, whose
/// terminator is `)` (not `{`); it registers comments unconditionally before its comma check.
fn skip_comments_before_comma(parser: &mut CssParser<'_, '_>) -> Result<(), ParseError> {
    if matches!(&parser.current_kind, TokenKind::Comment)
        && parser.peek_past_whitespace()? == TokenKind::Comma
    {
        parser.skip_whitespace_registering_comments()?;
    }
    Ok(())
}

pub(crate) fn parse_complex_selector_list<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<SelectorList<'arena>, ParseError> {
    let start = parser.base_offset() + parser.current_start();
    let mut selectors = parser.bvec();

    // Parse first complex selector
    let first = parse_complex_selector(parser)?;
    let mut end = first.span.end;
    selectors.push(first);

    // Parse additional selectors separated by commas
    loop {
        skip_comments_before_comma(parser)?;
        if !parser.check(TokenKind::Comma) {
            break;
        }
        parser.advance()?; // consume comma
        parser.skip_whitespace_registering_comments()?; // register after-comma comments
        let sel = parse_complex_selector(parser)?;
        end = sel.span.end;
        selectors.push(sel);
    }

    Ok(SelectorList {
        selectors: selectors.into_bump_slice(),
        span: Span {
            start: start as u32,
            end,
        },
    })
}

/// Parse a forgiving complex selector list: `div, >>>invalid, span`
///
/// This is used for :is() and :where() pseudo-classes, which have "forgiving" parsing.
/// Per CSS Selectors Level 4, section 9.1:
///
/// "Any items in a forgiving selector list that are invalid
/// (whether explicitly, by using unknown selectors or syntax,
/// or merely contextually, using known syntax but in an invalid context)
/// must be treated as having zero specificity."
///
/// Algorithm (CSS Selectors Level 4 - "parse as a forgiving selector list"):
/// 1. Parse a list of <<complex-real-selector>>s from input
/// 2. For items that fail to parse: wrap as Invalid selector (preserves source)
/// 3. Keep all selectors (valid and invalid) in AST
/// 4. Return a selector list representing all items
///
/// Examples:
/// - `:is(.a, ., .b)` → [ClassSelector(.a), Invalid("."), ClassSelector(.b)]
/// - `:is(.a, ::before, .b)` → [ClassSelector(.a), PseudoElement(::before), ClassSelector(.b)]
/// - `:is(., [)` → [Invalid("."), Invalid("[")]
///
/// Note: Invalid selectors preserved for formatter output (not deleted).
/// Public AST conversion filters them out for Svelte compatibility.
///
/// See: CSS Selectors Level 4 - <<forgiving-selector-list>>
pub(crate) fn parse_forgiving_selector_list<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<SelectorList<'arena>, ParseError> {
    let start = parser.base_offset() + parser.current_start();
    let mut selectors = parser.bvec();

    loop {
        let selector_start = parser.base_offset() + parser.current_start();
        let source_start = parser.current_start; // Raw source position for extraction

        // Try to parse a selector
        match parse_complex_selector(parser) {
            Ok(selector) => {
                selectors.push(selector);
            }
            Err(_) => {
                // Parse error - advance past the invalid selector and wrap as Invalid
                // (its raw text is recovered from the span at print time).
                extract_selector_until_comma_or_end(parser, source_start)?;
                let selector_end = parser.base_offset() + parser.current_start();

                let invalid = create_invalid_selector(parser.arena, selector_start, selector_end);
                selectors.push(invalid);
            }
        }

        // Check for comma (more selectors) or end of list. Register comments so the
        // printer can interleave them (forgiving lists carry leading/comma/trailing
        // comments inside `:is()`/`:where()`).
        parser.skip_whitespace_registering_comments()?;
        if parser.check(TokenKind::Comma) {
            parser.advance()?; // consume comma
            parser.skip_whitespace_registering_comments()?;
        } else {
            // End of list (hit right paren or other terminator)
            break;
        }
    }

    // Calculate end position from last selector, or use start if empty
    let end = selectors.last().map_or(start as u32, |s| s.span.end);

    Ok(SelectorList {
        selectors: selectors.into_bump_slice(),
        span: Span {
            start: start as u32,
            end,
        },
    })
}

/// Create an Invalid ComplexSelector from span positions (the raw text is
/// recovered verbatim from `span` at print time).
fn create_invalid_selector<'arena>(
    arena: &'arena Bump,
    start: usize,
    end: usize,
) -> ComplexSelector<'arena> {
    let span = Span {
        start: start as u32,
        end: end as u32,
    };

    let mut simple = BumpVec::new_in(arena);
    simple.push(SimpleSelector::Invalid { span });
    let mut children = BumpVec::new_in(arena);
    children.push(RelativeSelector {
        combinator: None,
        combinator_span: None,
        selectors: simple.into_bump_slice(),
        span,
    });

    ComplexSelector {
        children: children.into_bump_slice(),
        span,
    }
}

/// Extract raw text until we reach the next comma or end of selector list.
///
/// This is used for error recovery in forgiving selector lists.
/// We skip invalid tokens while tracking nesting depth (parens, brackets)
/// to avoid stopping at commas inside nested contexts.
///
/// Returns the raw text from `start_pos` to current position.
///
/// Stops at:
/// - Comma at depth 0 (next selector in list)
/// - Right paren at depth 0 (end of pseudo-class args)
/// - EOF (unexpected but handled)
fn extract_selector_until_comma_or_end<'a>(
    parser: &mut CssParser<'a, '_>,
    start_pos: usize,
) -> Result<&'a str, ParseError> {
    let mut depth = 0; // Track nesting depth for parens/brackets

    loop {
        match &parser.current_kind {
            TokenKind::RightParen if depth == 0 => {
                // End of selector list - don't consume the closing paren
                break;
            }
            TokenKind::Comma if depth == 0 => {
                // Next selector - don't consume the comma
                break;
            }
            TokenKind::LeftParen | TokenKind::LeftBracket | TokenKind::LeftBrace => {
                depth += 1;
                parser.advance()?;
            }
            TokenKind::RightParen | TokenKind::RightBracket | TokenKind::RightBrace => {
                if depth > 0 {
                    depth -= 1;
                }
                parser.advance()?;
            }
            TokenKind::Eof => {
                // Unexpected EOF - stop here
                return Err(ParseError::UnexpectedEof {
                    position: parser.base_offset() + parser.current_start(),
                    context: None,
                });
            }
            _ => {
                parser.advance()?;
            }
        }
    }

    // Extract raw text from source (from start_pos to current position)
    let end_pos = parser.current_start;
    let raw = &parser.source()[start_pos..end_pos];
    Ok(raw)
}

/// Parse a relative selector list: `:has(> img, + li, div)`
///
/// Relative selectors can start with combinators (>, +, ~) or have implied descendant.
/// Used in :has() pseudo-class arguments.
///
/// Per CSS Selectors Level 4: "Relative selectors begin with a combinator,
/// with a selector representing the anchor element implied at the start of the selector.
/// If no combinator is present, the descendant combinator is implied."
///
/// Note: :is(), :where(), :not() do NOT use relative selectors - they use complex selectors.
/// Only :has() uses relative selectors because it needs to express relationships from the element.
///
/// See: CSS Selectors Level 4 - <<relative-selector-list>>
pub(crate) fn parse_relative_selector_list<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<SelectorList<'arena>, ParseError> {
    let start = parser.base_offset() + parser.current_start();
    let mut selectors = parser.bvec();

    // Parse first relative complex selector
    let first = parse_relative_complex_selector(parser)?;
    let mut end = first.span.end;
    selectors.push(first);

    // Parse additional selectors separated by commas
    loop {
        skip_comments_before_comma(parser)?;
        if !parser.check(TokenKind::Comma) {
            break;
        }
        parser.advance()?; // consume comma
        parser.skip_whitespace_registering_comments()?; // register after-comma comments
        let sel = parse_relative_complex_selector(parser)?;
        end = sel.span.end;
        selectors.push(sel);
    }

    Ok(SelectorList {
        selectors: selectors.into_bump_slice(),
        span: Span {
            start: start as u32,
            end,
        },
    })
}

/// Parse a relative complex selector: `> div > span` or `+ li` or just `div`
///
/// This handles the core difference between relative and regular complex selectors:
/// - Relative: CAN start with a combinator (>, +, ~) - used in :has()
/// - Regular: CANNOT start with a combinator - used in :is(), :not(), :where()
///
/// Examples:
/// - `> img` - starts with > combinator (relative)
/// - `+ li` - starts with + combinator (relative)
/// - `span` - no leading combinator, combinator field is null (NOT descendant!)
/// - `div > span` - combinator in middle (relative or regular)
///
/// See: CSS Selectors Level 4 - <<relative-selector>> vs <<complex-selector>>
fn parse_relative_complex_selector<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<ComplexSelector<'arena>, ParseError> {
    let start = parser.base_offset() + parser.current_start();
    let mut children = parser.bvec();

    // Check if we start with an EXPLICIT combinator (>, +, ~, ||)
    // Note: We use parse_explicit_combinator here, NOT parse_combinator,
    // because selectors without a leading combinator should have combinator: null,
    // not an implicit Descendant combinator.
    let first_combinator_info = parse_explicit_combinator(parser)?;

    let first = if let Some((combinator, combinator_span)) = first_combinator_info {
        // Starts with explicit combinator: :has(> img)
        parse_relative_selector(parser, Some(combinator), Some(combinator_span))?
    } else {
        // No leading combinator: :has(img) - combinator field will be null
        parse_relative_selector(parser, None, None)?
    };
    let mut end = first.span.end;
    children.push(first);

    // Parse additional relative selectors with combinators
    loop {
        // Stop at ), ,, or EOF (used in pseudo-class argument contexts)
        if parser.check(TokenKind::LeftBrace)
            || parser.check(TokenKind::Comma)
            || parser.check(TokenKind::RightParen)
            || parser.check(TokenKind::Eof)
            || matches!(&parser.current_kind, TokenKind::Comment)
        {
            break;
        }

        // Check for combinator
        let Some((combinator, combinator_span)) = parse_combinator(parser)? else {
            break; // No more combinators, we're done
        };

        let child = parse_relative_selector(parser, Some(combinator), Some(combinator_span))?;
        end = child.span.end;
        children.push(child);
    }

    Ok(ComplexSelector {
        children: children.into_bump_slice(),
        span: Span {
            start: start as u32,
            end,
        },
    })
}

/// Parse a complex selector: `div > span + .class`
pub(crate) fn parse_complex_selector<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<ComplexSelector<'arena>, ParseError> {
    let start = parser.base_offset() + parser.current_start();
    let mut children = parser.bvec();

    // First relative selector has no combinator
    let first = parse_relative_selector(parser, None, None)?;
    let mut end = first.span.end;
    children.push(first);

    // Parse additional relative selectors with combinators
    loop {
        // Don't skip comments here - let parse_combinator handle them
        // Comments before {, ,, or EOF will cause parse_combinator to return None
        // Also check for ) to support selector lists inside pseudo-class arguments
        if parser.check(TokenKind::LeftBrace)
            || parser.check(TokenKind::Comma)
            || parser.check(TokenKind::RightParen)
            || parser.check(TokenKind::Eof)
            || matches!(&parser.current_kind, TokenKind::Comment)
        {
            break;
        }

        // Check for combinator (this will skip whitespace internally)
        let Some((combinator, combinator_span)) = parse_combinator(parser)? else {
            break; // No more combinators, we're done
        };

        let child = parse_relative_selector(parser, Some(combinator), Some(combinator_span))?;
        end = child.span.end;
        children.push(child);
    }

    Ok(ComplexSelector {
        children: children.into_bump_slice(),
        span: Span {
            start: start as u32,
            end,
        },
    })
}

/// Parse an EXPLICIT combinator only: `>`, `+`, `~`, `||` (but NOT whitespace/descendant)
/// Returns (combinator type, combinator span) only for explicit combinator symbols.
/// Used for checking leading combinators in relative selectors where descendant is NOT implied.
///
/// This is different from `parse_combinator()` which also returns Descendant for whitespace.
pub(crate) fn parse_explicit_combinator(
    parser: &mut CssParser<'_, '_>,
) -> Result<Option<(Combinator, Span)>, ParseError> {
    parser.skip_whitespace()?;

    let combinator_start = parser.base_offset() + parser.current_start();

    let combinator = match &parser.current_kind {
        TokenKind::GreaterThan => Some(Combinator::Child),
        TokenKind::Plus => Some(Combinator::NextSibling),
        TokenKind::Tilde => Some(Combinator::SubsequentSibling),
        TokenKind::ColumnCombinator => Some(Combinator::Column),
        _ => None, // No explicit combinator found
    };

    if let Some(comb) = combinator {
        let end = parser.base_offset() + parser.current_end;
        parser.advance()?; // consume combinator token
        parser.skip_whitespace()?;

        Ok(Some((
            comb,
            Span {
                start: combinator_start as u32,
                end: end as u32,
            },
        )))
    } else {
        Ok(None)
    }
}

/// Parse a combinator: `>`, `+`, `~`, `||`, or whitespace (descendant)
/// Returns (combinator type, combinator span)
pub(crate) fn parse_combinator(
    parser: &mut CssParser<'_, '_>,
) -> Result<Option<(Combinator, Span)>, ParseError> {
    // Capture position before skipping whitespace for descendant combinator
    let whitespace_start = parser.base_offset() + parser.current_start();
    parser.skip_whitespace()?;

    // Don't skip comments - parse_complex_selector checks for them before calling this
    // If we're here and there's a comment, it means it's terminal (before {, ,, or EOF)
    // and we should return None to stop parsing selectors

    let combinator_start = parser.base_offset() + parser.current_start();

    let combinator = match &parser.current_kind {
        TokenKind::GreaterThan => Some(Combinator::Child),
        TokenKind::Plus => Some(Combinator::NextSibling),
        TokenKind::Tilde => Some(Combinator::SubsequentSibling),
        TokenKind::ColumnCombinator => Some(Combinator::Column),
        _ => {
            // Descendant requires actual whitespace between the selectors — an adjacent
            // selector token is part of the same compound (handled by the
            // is_simple_selector_chain loop) and must never fabricate a zero-width combinator
            if combinator_start > whitespace_start && is_selector_start(parser) {
                Some(Combinator::Descendant)
            } else {
                None
            }
        }
    };

    let result = if let Some(comb) = combinator {
        let (start, end) = if comb == Combinator::Descendant {
            // Descendant is whitespace - span from end of previous to start of next
            (whitespace_start, combinator_start)
        } else {
            (combinator_start, parser.base_offset() + parser.current_end)
        };

        if comb != Combinator::Descendant {
            parser.advance()?; // consume combinator token
            parser.skip_whitespace()?;
        }

        Some((
            comb,
            Span {
                start: start as u32,
                end: end as u32,
            },
        ))
    } else {
        None
    };

    Ok(result)
}

/// Check if current token could start a selector
fn is_selector_start(parser: &CssParser<'_, '_>) -> bool {
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
fn parse_relative_selector<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    combinator: Option<Combinator>,
    combinator_span: Option<Span>,
) -> Result<RelativeSelector<'arena>, ParseError> {
    // Start position is either the combinator start (if present) or the current selector start
    let start = combinator_span.map_or_else(
        || parser.base_offset() + parser.current_start(),
        |s| s.start_usize(),
    );
    let mut selectors = parser.bvec();

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
        return Err(parser.error_expected_at("selector", start));
    }

    Ok(RelativeSelector {
        combinator,
        combinator_span,
        selectors: selectors.into_bump_slice(),
        span: Span {
            start: start as u32,
            end: end as u32,
        },
    })
}

/// Check if another simple selector follows in the chain (e.g., `div.class#id`, `&__a`, `div&`)
///
/// Whitespace is tokenized, so a directly-adjacent `Identifier`/`Asterisk`/`Ampersand` can only
/// appear mid-compound (`&__a`, `div&`, `&&`, `*&`) — a space yields a `Whitespace` token and ends
/// the chain. Type-not-first compounds (`&div`, `a&b`) are grammar-invalid per Selectors 4 but
/// parsed for parity with Svelte's `parseCss` (validity is the future diagnostics layer's job).
fn is_simple_selector_chain(parser: &CssParser<'_, '_>) -> bool {
    matches!(
        parser.current_kind,
        TokenKind::Dot
            | TokenKind::Hash
            | TokenKind::Colon
            | TokenKind::LeftBracket
            | TokenKind::Identifier
            | TokenKind::Asterisk
            | TokenKind::Ampersand
    )
}

/// Parse a simple selector: type, class, id, attribute, pseudo-class, pseudo-element
pub(crate) fn parse_simple_selector<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<SimpleSelector<'arena>, ParseError> {
    let start = parser.base_offset() + parser.current_start();

    match &parser.current_kind {
        TokenKind::Identifier => {
            // Type selector: div, span, etc. Could also be a namespace prefix:
            // svg|rect, *|div. Peek for the `|` before allocating — only the rare
            // namespaced form copies the prefix into the arena; a bare type selector
            // recovers its text verbatim from `span` at print time.
            if matches!(parser.peek()?, TokenKind::Pipe) {
                // Namespace prefix: identifier|element
                let namespace = Some(
                    parser.alloc_str_in(
                        parser
                            .current_identifier()
                            .ok_or_else(|| parser.error_expected("identifier"))?,
                    ),
                );
                parser.advance()?; // consume the namespace identifier
                parser.advance()?; // consume the pipe

                // Must be followed by an identifier (element name)
                if !parser.check(TokenKind::Identifier) {
                    return Err(parser.error_expected_after("element name", "namespace prefix"));
                }
                let end = parser.base_offset() + parser.current_end;
                parser.advance()?;

                Ok(SimpleSelector::Type {
                    namespace,
                    span: Span {
                        start: start as u32,
                        end: end as u32,
                    },
                })
            } else {
                // No namespace, just a regular type selector. Validate the identifier
                // (matches the prior path); its text is recovered from `span` at print
                // time, so nothing is copied into the arena.
                parser
                    .current_identifier()
                    .ok_or_else(|| parser.error_expected("identifier"))?;
                parser.advance()?;
                let end = parser.base_offset() + parser.current_start();
                Ok(SimpleSelector::Type {
                    namespace: None,
                    span: Span {
                        start: start as u32,
                        end: end as u32,
                    },
                })
            }
        }
        TokenKind::Dot => {
            // Class selector: .class
            parser.advance()?; // consume .
            if !parser.check(TokenKind::Identifier) {
                return Err(parser.error_expected_after("class name", "."));
            }
            // Validate an identifier follows; its text is recovered from `span` at
            // print time, so nothing is copied into the arena.
            parser
                .current_identifier()
                .ok_or_else(|| parser.error_expected("identifier"))?;
            let end = parser.base_offset() + parser.current_end;
            parser.advance()?;
            Ok(SimpleSelector::Class {
                span: Span {
                    start: start as u32,
                    end: end as u32,
                },
            })
        }
        TokenKind::Hash => {
            // ID selector: #id
            parser.advance()?; // consume #
            if !parser.check(TokenKind::Identifier) {
                return Err(parser.error_expected_after("ID name", "#"));
            }
            // Validate an identifier follows; its text is recovered from `span` at
            // print time, so nothing is copied into the arena.
            parser
                .current_identifier()
                .ok_or_else(|| parser.error_expected("identifier"))?;
            let end = parser.base_offset() + parser.current_end;
            parser.advance()?;
            Ok(SimpleSelector::Id {
                span: Span {
                    start: start as u32,
                    end: end as u32,
                },
            })
        }
        TokenKind::Asterisk => {
            // Universal selector: *
            // Could also be universal namespace prefix: *|div
            parser.advance()?;

            // Check for namespace: *|element
            if parser.check(TokenKind::Pipe) {
                parser.advance()?; // consume pipe

                // Must be followed by an identifier (element name)
                if !parser.check(TokenKind::Identifier) {
                    return Err(
                        parser.error_expected_after("element name", "universal namespace prefix")
                    );
                }

                // Validate an element identifier follows; its text is recovered from
                // `span` at print time, so nothing is copied into the arena.
                parser
                    .current_identifier()
                    .ok_or_else(|| parser.error_expected("identifier"))?;
                let end = parser.base_offset() + parser.current_end;
                parser.advance()?;

                Ok(SimpleSelector::Type {
                    namespace: Some("*"), // Universal namespace
                    span: Span {
                        start: start as u32,
                        end: end as u32,
                    },
                })
            } else {
                // Just a universal selector (no namespace)
                let end = parser.base_offset() + parser.current_start();
                Ok(SimpleSelector::Universal {
                    namespace: None,
                    span: Span {
                        start: start as u32,
                        end: end as u32,
                    },
                })
            }
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
        TokenKind::Percentage => {
            // Percentage selector: 0%, 50%, 100% (used in @keyframes)
            // Extract value without the % suffix
            let value_str = &parser.source()[parser.current_start..parser.current_end - 1];
            let value = value_str.parse::<f64>().map_err(|_| {
                parser.error_msg_at(&format!("Invalid percentage value: {value_str}"), start)
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
        TokenKind::Pipe => {
            // Explicit no-namespace selector: |div
            // This selects elements with no namespace (in contrast to *|div for any namespace)
            parser.advance()?; // consume pipe

            // Must be followed by an identifier (element name) or asterisk (universal)
            if parser.check(TokenKind::Identifier) {
                // Validate an element identifier follows; its text is recovered from
                // `span` at print time, so nothing is copied into the arena.
                parser
                    .current_identifier()
                    .ok_or_else(|| parser.error_expected("identifier"))?;
                let end = parser.base_offset() + parser.current_end;
                parser.advance()?;

                Ok(SimpleSelector::Type {
                    namespace: Some(""), // Empty string = explicit no namespace
                    span: Span {
                        start: start as u32,
                        end: end as u32,
                    },
                })
            } else if parser.check(TokenKind::Asterisk) {
                // |* - universal selector with explicit no namespace
                let end = parser.base_offset() + parser.current_end;
                parser.advance()?;

                Ok(SimpleSelector::Universal {
                    namespace: Some(""), // Empty string = explicit no namespace
                    span: Span {
                        start: start as u32,
                        end: end as u32,
                    },
                })
            } else {
                Err(parser.error_expected_after("element name or '*'", "no-namespace prefix '|'"))
            }
        }
        _ => Err(parser.error_msg_at(
            &format!("Unexpected token in selector: {:?}", parser.current_kind),
            start,
        )),
    }
}
