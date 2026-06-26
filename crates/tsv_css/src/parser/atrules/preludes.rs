use super::{CssParser, is_boolean_operator};
use crate::ast::internal::*;
use crate::lexer::TokenKind;
use crate::parser::selectors::parse_complex_selector_list;
use tsv_lang::{ParseError, Span};

/// Parse a condition query — `(prop: val)` parts connected by `and`/`or`, with
/// an optional leading `not` and function-style `selector(...)` conditions.
///
/// This *is* the entire `@supports` prelude (`<supports-condition>`), and
/// `@container` reuses it verbatim for its `<container-query>` — the two grammars
/// are identical; `@container` only adds an optional `<container-name>` preamble
/// before calling this. The returned span starts at the parser's current
/// position (the first condition token); `parse_container_prelude` widens the
/// start to cover the name.
///
/// Examples:
/// - `(display: grid)` - single condition
/// - `(display: grid) and (flex: 1)` - conjunction
/// - `not (color: red)` - negation
/// - `(a) and (b) or (c)` - mixed (parsed left-to-right)
pub(super) fn parse_condition_query<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<(ConditionQuery<'arena>, Span), ParseError> {
    let start = parser.base_offset() + parser.current_start;
    let mut parts = parser.bvec();
    let mut current_connector: Option<ConditionConnector> = None;
    let mut end_pos = start;

    while !parser.check(TokenKind::LeftBrace)
        && !parser.check(TokenKind::Semicolon)
        && !parser.check(TokenKind::Eof)
    {
        parser.skip_whitespace()?;

        // Register comments between condition parts (e.g., `(a) /* comment */ and (b)`)
        while parser.check(TokenKind::Comment) {
            parser.register_current_comment();
            end_pos = parser.base_offset() + parser.current_end;
            parser.advance()?;
            parser.skip_whitespace()?;
        }

        // Check for `and`/`or` connector
        if parser.check(TokenKind::Identifier) {
            let ident = parser
                .current_identifier()
                .unwrap_or_else(|| parser.current_value());

            if ident == "and" || ident == "or" {
                // This is a connector between parts
                current_connector = Some(if ident == "and" {
                    ConditionConnector::And
                } else {
                    ConditionConnector::Or
                });
                parser.advance()?;
                // Register comments after connector (e.g., `and /* comment */ (b)`)
                parser.skip_whitespace()?;
                while parser.check(TokenKind::Comment) {
                    parser.register_current_comment();
                    end_pos = parser.base_offset() + parser.current_end;
                    parser.advance()?;
                    parser.skip_whitespace()?;
                }
                continue;
            }
        }

        // Parse a condition part (may start with `not`, then parenthesized content)
        let part_start = parser.base_offset() + parser.current_start;
        let mut part_content = Vec::new();
        let mut paren_depth: usize = 0;

        // Check for leading `not`
        if parser.check(TokenKind::Identifier) {
            let ident = parser
                .current_identifier()
                .unwrap_or_else(|| parser.current_value());
            if ident == "not" {
                part_content.push("not".to_string());
                parser.advance()?;
                parser.skip_whitespace()?;
                // Include comments after `not` in content (e.g., `not /* comment */ (...)`)
                // These go in part_content rather than being registered, since they're
                // inside the condition part's span
                while parser.check(TokenKind::Comment) {
                    part_content.push(" ".to_string());
                    part_content.push(parser.current_value().to_string());
                    end_pos = parser.base_offset() + parser.current_end;
                    parser.advance()?;
                    parser.skip_whitespace()?;
                }
                part_content.push(" ".to_string());
            }
        }

        // Check for function-style condition like `selector(:has(...))`
        if parser.check(TokenKind::Identifier) {
            let ident = parser
                .current_identifier()
                .unwrap_or_else(|| parser.current_value());
            // Check if this is a function call (identifier followed by '(')
            let is_function =
                parser.source.get(parser.current_end..=parser.current_end) == Some("(");
            if is_function {
                // Include the function name
                part_content.push(ident.to_string());
                parser.advance()?;
                // Continue to parse the parenthesized part below
            }
        }

        // Now parse the parenthesized condition
        if !parser.check(TokenKind::LeftParen) {
            // Not a valid @supports part - break out
            break;
        }

        // Parse until we close all parens and hit whitespace/and/or/brace
        // Track state for whitespace normalization
        let mut prev_token_kind: Option<TokenKind> = None;
        let mut last_non_whitespace_kind: Option<TokenKind> = None;

        while !parser.check(TokenKind::Eof) {
            // Track paren depth
            if parser.check(TokenKind::LeftParen) {
                paren_depth += 1;
            } else if parser.check(TokenKind::RightParen) {
                if paren_depth == 0 {
                    break;
                }
                paren_depth -= 1;
            }

            // Check for end of part (at top level)
            if paren_depth == 0 && parser.check(TokenKind::RightParen) {
                // Include the closing paren
                part_content.push(")".to_string());
                end_pos = parser.base_offset() + parser.current_end;
                parser.advance()?;
                break;
            }

            // Handle whitespace normalization
            if parser.check(TokenKind::Whitespace) {
                let skip_whitespace = matches!(prev_token_kind, Some(TokenKind::LeftParen))
                    || matches!(parser.peek(), Ok(TokenKind::RightParen));

                parser.advance()?;

                if skip_whitespace {
                    continue;
                }
                part_content.push(" ".to_string());
                prev_token_kind = Some(TokenKind::Whitespace);
                continue;
            }

            // Get token value
            let part = match &parser.current_kind {
                TokenKind::Identifier => parser
                    .current_identifier()
                    .unwrap_or_else(|| parser.current_value())
                    .to_string(),
                TokenKind::String { quote } => {
                    let content =
                        &parser.source()[parser.current_start + 1..parser.current_end - 1];
                    format!("{quote}{content}{quote}")
                }
                TokenKind::Number | TokenKind::Percentage | TokenKind::Dimension { .. } => {
                    parser.current_value().to_string()
                }
                TokenKind::Comment => {
                    // Include comment and preserve trailing space before next token
                    parser.current_value().to_string()
                }
                _ => parser.current_value().to_string(),
            };

            // Add space after comment if followed by non-whitespace
            // (Comments need space before the next token)
            let is_comment = matches!(parser.current_kind, TokenKind::Comment);

            // Check if this is a boolean operator (and/or/not) inside nested parens
            let is_bool_op = matches!(&parser.current_kind, TokenKind::Identifier)
                && matches!(part.as_str(), "and" | "or" | "not");

            // Add space before boolean operators if not preceded by whitespace
            if is_bool_op && !matches!(prev_token_kind, Some(TokenKind::Whitespace)) {
                part_content.push(" ".to_string());
            }

            // Remove trailing whitespace before ':'
            if matches!(parser.current_kind, TokenKind::Colon) {
                while part_content.last().is_some_and(|s| s == " ") {
                    part_content.pop();
                }
            }

            part_content.push(part);
            let current_kind = parser.current_kind;
            end_pos = parser.base_offset() + parser.current_end;
            parser.advance()?;

            // Add space after boolean operators
            if is_bool_op && !parser.check(TokenKind::Whitespace) {
                part_content.push(" ".to_string());
            }

            // Add space after comment if followed by non-whitespace
            // (e.g., `/* comment */ grid` needs space before `grid`)
            if is_comment
                && !parser.check(TokenKind::Whitespace)
                && !parser.check(TokenKind::RightParen)
            {
                part_content.push(" ".to_string());
            }

            // Add space after ':' for property:value pairs
            if !parser.check(TokenKind::Whitespace)
                && matches!(current_kind, TokenKind::Colon)
                && matches!(
                    last_non_whitespace_kind,
                    Some(TokenKind::Identifier)
                        | Some(TokenKind::Number)
                        | Some(TokenKind::Dimension { .. })
                        | Some(TokenKind::Percentage)
                )
            {
                part_content.push(" ".to_string());
            }

            prev_token_kind = Some(current_kind);
            if !matches!(current_kind, TokenKind::Whitespace) {
                last_non_whitespace_kind = Some(current_kind);
            }
        }

        // Build the part
        let joined = part_content.join("");
        let content = joined.trim();
        if !content.is_empty() {
            parts.push(ConditionPart {
                connector: current_connector.take(),
                content: parser.alloc_str_in(content),
                span: Span {
                    start: part_start as u32,
                    end: end_pos as u32,
                },
            });
        }
    }

    let span = Span {
        start: start as u32,
        end: end_pos as u32,
    };

    Ok((
        ConditionQuery {
            parts: parts.into_bump_slice(),
        },
        span,
    ))
}

/// Parse @container prelude into structured condition parts with optional name
///
/// CSS Syntax: `@container [<container-name>]? <container-query>`
/// where container-query is similar to @supports: `(prop: val)` parts connected by `and`/`or`
///
/// Examples:
/// - `(min-width: 100px)` - no name, single condition
/// - `(min-width: 100px) and (max-width: 200px)` - no name, conjunction
/// - `sidebar (min-width: 100px)` - named container
/// - `sidebar (min-width: 100px) and (max-width: 200px)` - named container with conjunction
pub(super) fn parse_container_prelude<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<(Option<&'arena str>, ConditionQuery<'arena>, Span), ParseError> {
    let start = parser.base_offset() + parser.current_start;

    // Check for optional container name (identifier before first '(')
    // Container name is an identifier followed by whitespace then '('
    // NOT a function call like style(...) where there's no whitespace
    let container_name = if parser.check(TokenKind::Identifier) {
        let ident = parser
            .current_identifier()
            .unwrap_or_else(|| parser.current_value());
        // Check if this is actually a name (not 'not' or 'and' or 'or')
        // Also check it's not a function call (identifier directly followed by '(')
        let is_function_call =
            parser.source.get(parser.current_end..=parser.current_end) == Some("(");
        if !matches!(ident, "not" | "and" | "or") && !is_function_call {
            // Copy into the arena only on the path that stores the name as a node.
            let name = parser.alloc_str_in(ident);
            parser.advance()?;
            parser.skip_whitespace()?;
            Some(name)
        } else {
            None
        }
    } else {
        None
    };

    // Now parse the condition (same grammar as @supports).
    let (condition, cond_span) = parse_condition_query(parser)?;

    // The prelude span keeps the pre-name `start` and takes the condition's end,
    // so a named `@container foo (…)` covers the name while an unnamed one matches
    // `parse_condition_query` exactly.
    let span = Span {
        start: start as u32,
        end: cond_span.end,
    };

    Ok((container_name, condition, span))
}

/// Parse @scope prelude into structured selector lists
///
/// CSS Syntax: `@scope (<scope-start>) [to (<scope-end>)]`
///
/// Examples:
/// - `(.card)` - scope root only
/// - `(.card) to (.footer)` - scope root and limit
/// - `(article > header)` - with combinator
pub(super) fn parse_scope_prelude<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<(SelectorList<'arena>, Option<SelectorList<'arena>>, Span), ParseError> {
    let start = parser.base_offset() + parser.current_start;

    // Expect opening paren
    if !parser.check(TokenKind::LeftParen) {
        return Err(parser.error_expected("'(' in @scope prelude"));
    }
    parser.advance()?; // consume '('
    parser.skip_whitespace()?;

    // Parse root selector list
    let root = parse_complex_selector_list(parser)?;

    parser.skip_whitespace()?;

    // Expect closing paren
    if !parser.check(TokenKind::RightParen) {
        return Err(parser.error_expected_after("')'", "@scope root selectors"));
    }
    let end_after_root_paren = parser.base_offset() + parser.current_end;
    parser.advance()?; // consume ')'

    // Note: Don't skip whitespace yet - we need to check for "to" keyword
    // Check for optional "to" clause
    parser.skip_whitespace()?;
    let (limit, end_pos) = if parser.check(TokenKind::Identifier) {
        let identifier = parser
            .current_identifier()
            .unwrap_or_else(|| parser.current_value());
        if identifier == "to" {
            parser.advance()?; // consume "to"
            parser.skip_whitespace()?;

            // Expect opening paren
            if !parser.check(TokenKind::LeftParen) {
                return Err(parser.error_expected_after("'('", "'to' in @scope prelude"));
            }
            parser.advance()?; // consume '('
            parser.skip_whitespace()?;

            // Parse limit selector list
            let limit_selectors = parse_complex_selector_list(parser)?;

            parser.skip_whitespace()?;

            // Expect closing paren
            if !parser.check(TokenKind::RightParen) {
                return Err(parser.error_expected_after("')'", "@scope limit selectors"));
            }
            let end_after_limit_paren = parser.base_offset() + parser.current_end;
            parser.advance()?; // consume ')'
            parser.skip_whitespace()?;

            (Some(limit_selectors), end_after_limit_paren)
        } else {
            (None, end_after_root_paren)
        }
    } else {
        (None, end_after_root_paren)
    };

    // Span covers the entire prelude (up to and including the last ')')
    let span = Span {
        start: start as u32,
        end: end_pos as u32,
    };

    Ok((root, limit, span))
}

/// Parse @import prelude into structured values
///
/// CSS Syntax: `@import [ <url> | <string> ] [ layer | layer(<layer-name>) ]? <import-conditions> ;`
///
/// Examples:
/// - `url('styles.css')`
/// - `'styles.css'`
/// - `url('tabs.css') layer(framework)`
/// - `url('override.css') layer`
/// - `url('narrow.css') supports(display: flex) screen`
pub(super) fn parse_import_prelude<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<(&'arena [CssValue<'arena>], Span), ParseError> {
    let start = parser.base_offset() + parser.current_start;
    let mut values = parser.bvec();

    // Register a leading comment between `@import` and the url()/string (e.g.
    // `@import /* c */ url(...)`). Svelte strips it from the prelude; the printer
    // reconstructs it from `self.comments`.
    parser.skip_whitespace_registering_comments()?;

    // Parse first value: url() function or bare string
    let is_function = parser.check(TokenKind::Identifier) && {
        // Check if next char in source is '(' (function call)
        let end_pos = parser.current_end;
        parser.source.get(end_pos..=end_pos) == Some("(")
    };

    if is_function {
        // url() function
        values.push(parse_function_value(parser)?);
    } else if let TokenKind::String { .. } = &parser.current_kind {
        // Bare string — the inner text is recovered verbatim from `span` at print time
        // (span-for-verbatim, zero alloc); the quote char from `source[span.start]`.
        let value_start = (parser.base_offset() + parser.current_start) as u32;
        let value_end = (parser.base_offset() + parser.current_end) as u32;
        values.push(CssValue::String {
            content: StringCooked::Verbatim,
            span: Span {
                start: value_start,
                end: value_end,
            },
        });
        parser.advance()?;
    } else {
        return Err(parser.error_msg("@import expects url() or string"));
    }

    parser.skip_whitespace_registering_comments()?;

    // Parse optional layer(), supports() functions and other conditions
    while !parser.check(TokenKind::Semicolon) && !parser.check(TokenKind::Eof) {
        let is_function = parser.check(TokenKind::Identifier) && {
            let end_pos = parser.current_end;
            parser.source.get(end_pos..=end_pos) == Some("(")
        };

        if is_function {
            // layer() or supports() function
            values.push(parse_function_value(parser)?);
            parser.skip_whitespace_registering_comments()?;
        } else if parser.check(TokenKind::Identifier) {
            // Check for bare "layer" keyword or media query
            let ident = parser
                .current_identifier()
                .unwrap_or_else(|| parser.current_value());

            if ident == "layer" {
                // Bare "layer" keyword (without function call); text recovered from
                // `span` at print time (span-for-verbatim).
                let value_start = (parser.base_offset() + parser.current_start) as u32;
                let value_end = (parser.base_offset() + parser.current_end) as u32;
                values.push(CssValue::Identifier {
                    span: Span {
                        start: value_start,
                        end: value_end,
                    },
                });
                parser.advance()?;
                parser.skip_whitespace_registering_comments()?;
            } else {
                // Media query - preserve original whitespace from source
                let media_local_start = parser.current_start;
                let media_start = (parser.base_offset() + media_local_start) as u32;
                let mut media_local_end = parser.current_end;

                while !parser.check(TokenKind::Semicolon) && !parser.check(TokenKind::Eof) {
                    if !parser.check(TokenKind::Whitespace) {
                        media_local_end = parser.current_end;
                    }
                    parser.advance()?;
                }

                let media_end = (parser.base_offset() + media_local_end) as u32;

                // Media-query text recovered verbatim from `span` at print time.
                if media_local_end > media_local_start {
                    values.push(CssValue::Identifier {
                        span: Span {
                            start: media_start,
                            end: media_end,
                        },
                    });
                }
                break;
            }
        } else {
            break;
        }
    }

    let end = values.last().map_or(start as u32, |v| v.span().end);

    Ok((
        values.into_bump_slice(),
        Span {
            start: start as u32,
            end,
        },
    ))
}

/// Parse a function value (e.g., url(), layer(), supports())
fn parse_function_value<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<CssValue<'arena>, ParseError> {
    let value_start = (parser.base_offset() + parser.current_start) as u32;

    // Get function name (current token should be identifier)
    let name = if parser.check(TokenKind::Identifier) {
        parser.alloc_str_in(
            parser
                .current_identifier()
                .unwrap_or_else(|| parser.current_value()),
        )
    } else {
        return Err(parser.error_expected("function name"));
    };

    parser.advance()?; // consume function name

    // Expect '('
    if !parser.check(TokenKind::LeftParen) {
        return Err(parser.error_expected_after("'('", "function name"));
    }

    parser.advance()?; // consume '('

    // For @import functions (url, layer, supports), parse arguments based on function type
    let mut args = parser.bvec();

    if name == "url" {
        // url() - parse the URL argument (string or bare URL)
        parser.skip_whitespace()?;
        if let TokenKind::String { .. } = &parser.current_kind {
            let arg_start = (parser.base_offset() + parser.current_start) as u32;
            let arg_end = (parser.base_offset() + parser.current_end) as u32;
            // Bare string arg — inner text recovered verbatim from `span` at print
            // time (span-for-verbatim, zero alloc); quote char from `source[span.start]`.
            args.push(CssValue::String {
                content: StringCooked::Verbatim,
                span: Span {
                    start: arg_start,
                    end: arg_end,
                },
            });
            parser.advance()?;
            parser.skip_whitespace()?;
        } else {
            // Unquoted bare URL (`url(a.css)`, `url(a.css?x=1)`): an opaque token run
            // up to ')'. Leave args empty — both the public-AST conversion and the
            // printer reconstruct the `url(...)` verbatim from the function span.
            while !parser.check(TokenKind::RightParen) && !parser.check(TokenKind::Eof) {
                parser.advance()?;
            }
        }
    } else if name == "layer" {
        // layer(name) - parse the layer name as identifier
        parser.skip_whitespace()?;
        if parser.check(TokenKind::Identifier) {
            let arg_start = (parser.base_offset() + parser.current_start) as u32;
            let arg_end = (parser.base_offset() + parser.current_end) as u32;
            // Identifier text recovered verbatim from `span` at print time.
            args.push(CssValue::Identifier {
                span: Span {
                    start: arg_start,
                    end: arg_end,
                },
            });
            parser.advance()?;
        }
        parser.skip_whitespace()?;
    } else if name == "supports" {
        // supports(condition) - normalize whitespace like @supports at-rule prelude
        // This ensures `supports(  display:  grid  )` → `supports(display: grid)`
        parser.skip_whitespace()?;

        let condition_start = (parser.base_offset() + parser.current_start) as u32;
        let mut condition_parts = Vec::new();
        let mut prev_token_kind: Option<TokenKind> = None;
        let mut last_non_whitespace_kind: Option<TokenKind> = None;
        let mut condition_end = condition_start;

        while !parser.check(TokenKind::RightParen) && !parser.check(TokenKind::Eof) {
            // Skip whitespace after '(' or before ')'
            if parser.check(TokenKind::Whitespace) {
                let skip_whitespace = matches!(prev_token_kind, Some(TokenKind::LeftParen))
                    || matches!(parser.peek(), Ok(TokenKind::RightParen));

                parser.advance()?;

                if skip_whitespace {
                    continue;
                }
                condition_parts.push(" ".to_string());
                prev_token_kind = Some(TokenKind::Whitespace);
                continue;
            }

            let part = match &parser.current_kind {
                TokenKind::Identifier => parser
                    .current_identifier()
                    .unwrap_or_else(|| parser.current_value())
                    .to_string(),
                TokenKind::String { quote } => {
                    let content =
                        &parser.source()[parser.current_start + 1..parser.current_end - 1];
                    format!("{quote}{content}{quote}")
                }
                TokenKind::Number | TokenKind::Percentage | TokenKind::Dimension { .. } => {
                    parser.current_value().to_string()
                }
                _ => parser.current_value().to_string(),
            };

            // Check for boolean operators (for complex supports conditions)
            let is_bool_op = is_boolean_operator(parser);
            if is_bool_op && !matches!(prev_token_kind, Some(TokenKind::Whitespace)) {
                condition_parts.push(" ".to_string());
            }

            // Remove trailing whitespace before ':'
            if matches!(parser.current_kind, TokenKind::Colon) {
                while condition_parts.last().is_some_and(|s| s == " ") {
                    condition_parts.pop();
                }
            }

            condition_parts.push(part);

            let current_kind = parser.current_kind;
            condition_end = (parser.base_offset() + parser.current_end) as u32;
            parser.advance()?;

            // Add space after boolean operators or ':'
            if !parser.check(TokenKind::Whitespace) {
                if is_bool_op {
                    condition_parts.push(" ".to_string());
                } else if matches!(current_kind, TokenKind::Colon) {
                    // Add space after ':' in property:value pairs
                    if matches!(
                        last_non_whitespace_kind,
                        Some(TokenKind::Identifier)
                            | Some(TokenKind::Number)
                            | Some(TokenKind::Dimension { .. })
                            | Some(TokenKind::Percentage)
                    ) {
                        condition_parts.push(" ".to_string());
                    }
                }
            }

            prev_token_kind = Some(current_kind);
            if !matches!(current_kind, TokenKind::Whitespace) {
                last_non_whitespace_kind = Some(current_kind);
            }
        }

        // Condition text recovered verbatim from `span` at print time; the join is
        // only used to decide whether there's any content to push.
        let joined = condition_parts.join("");
        let condition_text = joined.trim();
        if !condition_text.is_empty() {
            args.push(CssValue::Identifier {
                span: Span {
                    start: condition_start,
                    end: condition_end,
                },
            });
        }

        parser.skip_whitespace()?;
    } else {
        // Other unknown functions - consume everything until )
        while !parser.check(TokenKind::RightParen) && !parser.check(TokenKind::Eof) {
            parser.advance()?;
        }
    }

    if !parser.check(TokenKind::RightParen) {
        return Err(parser.error_expected("')' to close function"));
    }

    let value_end = (parser.base_offset() + parser.current_end) as u32;
    parser.advance()?; // consume ')'

    Ok(CssValue::Function {
        name,
        args: args.into_bump_slice(),
        span: Span {
            start: value_start,
            end: value_end,
        },
    })
}
