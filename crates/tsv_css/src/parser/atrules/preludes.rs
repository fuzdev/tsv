use super::{CssParser, is_boolean_operator_keyword};
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
    // The connector's verbatim source text, kept so the printer can preserve the
    // author's case (`AND` stays `AND`); set in lockstep with `current_connector`.
    let mut current_connector_raw: Option<&'arena str> = None;
    let mut end_pos = start;

    while !parser.at_prelude_end() {
        parser.skip_whitespace()?;

        // Register comments between condition parts (e.g., `(a) /* comment */ and (b)`)
        while parser.check(TokenKind::Comment) {
            parser.register_current_comment();
            end_pos = parser.base_offset() + parser.current_end;
            parser.advance()?;
            parser.skip_whitespace()?;
        }

        // Check for `and`/`or` connector. CSS grammar keywords are ASCII
        // case-insensitive (CSS Syntax 3), so `AND`/`Or` connect like `and`; the
        // enum normalizes for logic but the source case is kept in `connector_raw`
        // and preserved by the printer (matching prettier).
        if parser.check(TokenKind::Identifier) {
            let ident = parser.current_identifier();

            let connector = if ident.eq_ignore_ascii_case("and") {
                Some(ConditionConnector::And)
            } else if ident.eq_ignore_ascii_case("or") {
                Some(ConditionConnector::Or)
            } else {
                None
            };

            if let Some(conn) = connector {
                // This is a connector between parts
                current_connector = Some(conn);
                current_connector_raw = Some(parser.alloc_str_in(parser.current_value()));
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
        let part_start = parser.span_pos(parser.current_start);
        let mut part_content = Vec::new();
        let mut paren_depth: usize = 0;

        // Check for leading `not` (ASCII case-insensitive). Its source case is kept
        // (pushed verbatim), preserved by the printer like the `and`/`or` connectors.
        if parser.check(TokenKind::Identifier) {
            let ident = parser.current_identifier();
            if ident.eq_ignore_ascii_case("not") {
                part_content.push(parser.current_value().to_string());
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

        // Check for function-style condition like `selector(:has(...))`: an
        // identifier directly followed by `(`. The name is serialized verbatim from
        // source (escapes preserved) — only `and`/`or`/`not` keyword matches decode.
        if parser.check(TokenKind::Identifier)
            && parser.source.get(parser.current_end..=parser.current_end) == Some("(")
        {
            part_content.push(parser.current_value().to_string());
            parser.advance()?;
            // Continue to parse the parenthesized part below
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
                    || matches!(parser.peek_kind(), Ok(TokenKind::RightParen));

                parser.advance()?;

                if skip_whitespace {
                    continue;
                }
                part_content.push(" ".to_string());
                prev_token_kind = Some(TokenKind::Whitespace);
                continue;
            }

            // Get token value. Identifiers serialize verbatim from source so escapes
            // survive (`\@foo` stays `\@foo`); keyword matching below decodes instead.
            let part = match &parser.current_kind {
                TokenKind::Identifier => parser.current_value().to_string(),
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

            // Check if this is a boolean operator (and/or/not) inside nested parens.
            // Match on the decoded value, not the now-verbatim `part`, so an escaped
            // operator still spaces correctly.
            let is_bool_op = matches!(&parser.current_kind, TokenKind::Identifier)
                && is_boolean_operator_keyword(parser.current_identifier());

            // Add space before boolean operators if not preceded by whitespace — but
            // not right after an opening paren (`(not (…))` keeps the paren tight,
            // matching prettier; the space would otherwise stack to `( not …`).
            if is_bool_op
                && !matches!(
                    prev_token_kind,
                    Some(TokenKind::Whitespace | TokenKind::LeftParen)
                )
            {
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
                connector_raw: current_connector_raw.take(),
                content: parser.alloc_str_in(content),
                span: Span {
                    start: part_start,
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
    let start = parser.span_pos(parser.current_start);

    // Check for optional container name: an identifier before the first '(' that
    // isn't a `not`/`and`/`or` keyword or a function call (`style(...)`, no space
    // before `(`). The keyword/function exclusion decodes + a structural `(`
    // lookahead; the stored name is serialized verbatim from source so escapes
    // survive (`\@named` stays `\@named`).
    let container_name = if parser.check(TokenKind::Identifier)
        && parser.source.get(parser.current_end..=parser.current_end) != Some("(")
        && !is_boolean_operator_keyword(parser.current_identifier())
    {
        // Copy into the arena only on the path that stores the name as a node.
        let name = parser.alloc_str_in(parser.current_value());
        parser.advance()?;
        parser.skip_whitespace()?;
        Some(name)
    } else {
        None
    };

    // Now parse the condition (same grammar as @supports).
    let (condition, cond_span) = parse_condition_query(parser)?;

    // The prelude span keeps the pre-name `start` and takes the condition's end,
    // so a named `@container foo (…)` covers the name while an unnamed one matches
    // `parse_condition_query` exactly.
    let span = Span {
        start,
        end: cond_span.end,
    };

    Ok((container_name, condition, span))
}

/// Parse @scope prelude into structured selector lists.
///
/// CSS Syntax (css-cascade-6): `@scope [(<scope-start>)]? [to (<scope-end>)]?` —
/// **both clauses are independently optional**, so all four combinations are valid
/// (parseCss accepts each): a bare `@scope { … }`, root-only, limit-only, and both.
///
/// Examples:
/// - `` (empty) - bare `@scope { … }`, scopes to the enclosing context
/// - `(.card)` - scope root only
/// - `(.card) to (.footer)` - scope root and limit
/// - `to (.footer)` - scope limit only
/// - `(article > header)` - with combinator
///
/// The span covers the authored prelude (first clause start to last `)`); when both
/// clauses are absent it is a zero-width span at the cursor, so the public AST's
/// `prelude` string extracts to `""` (matching parseCss).
pub(super) fn parse_scope_prelude<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<
    (
        Option<SelectorList<'arena>>,
        Option<SelectorList<'arena>>,
        Span,
    ),
    ParseError,
> {
    let start = parser.span_pos(parser.current_start);
    // Widens to each clause's closing `)`; stays at `start` when no clause is present.
    let mut end = start;

    // Optional root clause: `(<scope-start>)`.
    let root = if parser.check(TokenKind::LeftParen) {
        parser.advance()?; // consume '('
        parser.skip_whitespace()?;
        let root_selectors = parse_complex_selector_list(parser)?;
        parser.skip_whitespace()?;
        if !parser.check(TokenKind::RightParen) {
            return Err(parser.error_expected_after("')'", "@scope root selectors"));
        }
        end = parser.span_pos(parser.current_end);
        parser.advance()?; // consume ')'
        parser.skip_whitespace()?;
        Some(root_selectors)
    } else {
        None
    };

    // Optional limit clause: `to (<scope-end>)` — valid with or without a root.
    // `to` is a case-insensitive grammar keyword; canonicalized to lowercase at the
    // printer (the `" to ("` literal in `print_css_atrule`).
    let limit = if parser.check(TokenKind::Identifier)
        && parser.current_identifier().eq_ignore_ascii_case("to")
    {
        parser.advance()?; // consume "to"
        parser.skip_whitespace()?;
        if !parser.check(TokenKind::LeftParen) {
            return Err(parser.error_expected_after("'('", "'to' in @scope prelude"));
        }
        parser.advance()?; // consume '('
        parser.skip_whitespace()?;
        let limit_selectors = parse_complex_selector_list(parser)?;
        parser.skip_whitespace()?;
        if !parser.check(TokenKind::RightParen) {
            return Err(parser.error_expected_after("')'", "@scope limit selectors"));
        }
        end = parser.span_pos(parser.current_end);
        parser.advance()?; // consume ')'
        parser.skip_whitespace()?;
        Some(limit_selectors)
    } else {
        None
    };

    Ok((root, limit, Span { start, end }))
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
) -> Result<PreludeValue<'arena>, ParseError> {
    // Raw offset of the prelude's first token, for the raw fallback below.
    let prelude_start_raw = parser.current_start;
    let start = parser.span_pos(parser.current_start);
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
        let value_start = parser.span_pos(parser.current_start);
        let value_end = parser.span_pos(parser.current_end);
        values.push(CssValue::String {
            content: StringCooked::Verbatim,
            span: Span {
                start: value_start,
                end: value_end,
            },
        });
        parser.advance()?;
    } else if matches!(parser.current_kind, TokenKind::Url) {
        // Unquoted `url(...)` — the lexer consumed it as one opaque `<url-token>`. Mirror
        // `parse_function_value`'s empty-args url shape (name + span): the printer and the
        // public-AST conversion reconstruct the verbatim, inner-ws-trimmed `url(...)` from
        // the function span, so structured `@import url(…) layer/supports/media` wrapping
        // still works (unlike a raw fallback, which would drop that structure).
        let value_start = parser.span_pos(parser.current_start);
        let value_end = parser.span_pos(parser.current_end);
        values.push(CssValue::Function {
            // The printer's empty-args url path only reads `name` as a url-detection key
            // (`eq_ignore_ascii_case("url")`); the emitted text and the public AST both
            // come from `span`, so real casing/content is preserved regardless.
            name: "url",
            args: &[],
            span: Span {
                start: value_start,
                end: value_end,
            },
        });
        parser.advance()?;
    } else {
        // Not a `<url>`/`<string>` first value, so this isn't a structurable @import.
        // Per CSS Syntax 3 the prelude is still consumed as component values (an invalid
        // `@import` is dropped at cascade, not a parse error); parseCss stores it raw and
        // prettier prints it verbatim. Reconsume the whole prelude — including the empty
        // `@import;` case (nothing to consume → a zero-width raw prelude → `""`).
        return super::reconsume_prelude_as_raw(parser, prelude_start_raw);
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
            let ident = parser.current_identifier();

            if ident == "layer" {
                // Bare "layer" keyword (without function call); text recovered from
                // `span` at print time (span-for-verbatim).
                let value_start = parser.span_pos(parser.current_start);
                let value_end = parser.span_pos(parser.current_end);
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
                let media_start = parser.span_pos(media_local_start);
                let mut media_local_end = parser.current_end;

                while !parser.check(TokenKind::Semicolon) && !parser.check(TokenKind::Eof) {
                    if !parser.check(TokenKind::Whitespace) {
                        media_local_end = parser.current_end;
                    }
                    parser.advance()?;
                }

                let media_end = parser.span_pos(media_local_end);

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

    let end = values.last().map_or(start, |v| v.span().end);

    Ok(PreludeValue::Values {
        values: values.into_bump_slice(),
        span: Span { start, end },
    })
}

/// Parse a function value (e.g., url(), layer(), supports())
fn parse_function_value<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<CssValue<'arena>, ParseError> {
    let value_start = parser.span_pos(parser.current_start);

    // Get function name (current token should be identifier)
    let name = if parser.check(TokenKind::Identifier) {
        parser.alloc_str_in(parser.current_identifier())
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
            let arg_start = parser.span_pos(parser.current_start);
            let arg_end = parser.span_pos(parser.current_end);
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
            let arg_start = parser.span_pos(parser.current_start);
            let arg_end = parser.span_pos(parser.current_end);
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

        // The condition text is recovered verbatim from `span` at print time (see the
        // printer), so only its span and whether it holds any content matter here — there
        // is no normalized string to build. `condition_end` tracks the last non-whitespace
        // token, so leading/trailing whitespace is trimmed from the span. Track paren depth
        // so a nested `(…)`/`fn(…)` inside the condition doesn't end the arg at its first
        // inner `)` — the grammar is `supports( <supports-condition> | <declaration> )`
        // (css-cascade-4/5 §import-conditions), so `supports((display: grid))`,
        // `supports(not (a: b))`, and `supports(selector(a > b))` are all valid; only the
        // matching depth-0 `)` (the `supports(` close) ends the arg.
        let condition_start = parser.span_pos(parser.current_start);
        let mut condition_end = condition_start;
        let mut has_content = false;
        let mut depth: u32 = 0;

        while !parser.check(TokenKind::Eof) {
            match parser.current_kind {
                TokenKind::RightParen if depth == 0 => break,
                TokenKind::LeftParen => depth += 1,
                TokenKind::RightParen => depth -= 1,
                _ => {}
            }
            if !parser.check(TokenKind::Whitespace) {
                has_content = true;
                condition_end = parser.span_pos(parser.current_end);
            }
            parser.advance()?;
        }

        if has_content {
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

    let value_end = parser.span_pos(parser.current_end);
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
