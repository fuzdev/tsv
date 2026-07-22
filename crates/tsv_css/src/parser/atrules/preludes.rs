use super::{CssParser, is_boolean_operator_keyword};
use crate::ast::internal::*;
use crate::lexer::TokenKind;
use crate::parser::selectors::parse_forgiving_selector_list;
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
        // One growable buffer instead of a `Vec<String>` of per-token / per-space pieces
        // joined at the end (mirrors `parse_raw_prelude_content` and
        // `parse_declaration`): tokens `push_str` straight in, separators are a single
        // `push(' ')`. `trailing_spaces` counts the trailing programmatic spaces (the
        // collapse unit) so `truncate` strips exactly those, never a token's own
        // escape-terminator space — the old `Vec` "last part is `\" \"`" test, exactly.
        let mut part_buf = String::new();
        let mut trailing_spaces: usize = 0;
        let mut paren_depth: usize = 0;

        // Check for leading `not` (ASCII case-insensitive). Its source case is kept
        // (pushed verbatim), preserved by the printer like the `and`/`or` connectors.
        if parser.check(TokenKind::Identifier) {
            let ident = parser.current_identifier();
            if ident.eq_ignore_ascii_case("not") {
                part_buf.push_str(parser.current_value());
                trailing_spaces = 0;
                parser.advance()?;
                parser.skip_whitespace()?;
                // Include comments after `not` in content (e.g., `not /* comment */ (...)`)
                // These go in the part buffer rather than being registered, since they're
                // inside the condition part's span
                while parser.check(TokenKind::Comment) {
                    part_buf.push(' ');
                    part_buf.push_str(parser.current_value());
                    trailing_spaces = 0;
                    end_pos = parser.base_offset() + parser.current_end;
                    parser.advance()?;
                    parser.skip_whitespace()?;
                }
                part_buf.push(' ');
                trailing_spaces += 1;
            }
        }

        // Check for function-style condition like `selector(:has(...))`: an
        // identifier directly followed by `(`. The name is serialized verbatim from
        // source (escapes preserved) — only `and`/`or`/`not` keyword matches decode.
        if parser.check(TokenKind::Identifier)
            && parser.source.get(parser.current_end..=parser.current_end) == Some("(")
        {
            part_buf.push_str(parser.current_value());
            trailing_spaces = 0;
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
                // Include the closing paren (loop ends here, so no counter reset needed)
                part_buf.push(')');
                end_pos = parser.base_offset() + parser.current_end;
                parser.advance()?;
                break;
            }

            // Handle whitespace normalization
            if parser.check(TokenKind::Whitespace) {
                // A whitespace run right after a value colon (`(a: )`, empty value) is
                // the prettier-mandated single space after `:` — keep it before `)`
                // rather than dropping it, or `(a: )` would collapse to `(a:)` while
                // `(a:)` gains the space (the colon-space rule below), an F1 oscillation.
                // In `@supports`/`@container` a `:` is always a value colon.
                let after_value_colon = matches!(prev_token_kind, Some(TokenKind::Colon));
                let skip_whitespace = matches!(prev_token_kind, Some(TokenKind::LeftParen))
                    || (matches!(parser.peek_kind(), Ok(TokenKind::RightParen))
                        && !after_value_colon);

                parser.advance()?;

                if skip_whitespace {
                    continue;
                }
                part_buf.push(' ');
                trailing_spaces += 1;
                prev_token_kind = Some(TokenKind::Whitespace);
                continue;
            }

            // Add space after comment if followed by non-whitespace
            // (Comments need space before the next token)
            let is_comment = matches!(parser.current_kind, TokenKind::Comment);

            // Check if this is a boolean operator (and/or/not) inside nested parens.
            // Match on the decoded value, not the verbatim source slice, so an escaped
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
                part_buf.push(' ');
                trailing_spaces += 1;
            }

            // Remove trailing whitespace before ':' — only the counted programmatic
            // spaces, never a token's own escape-terminator space. (The counter is reset
            // by the token emission just below, which always runs next.)
            if matches!(parser.current_kind, TokenKind::Colon) {
                part_buf.truncate(part_buf.len() - trailing_spaces);
            }

            // Emit the token verbatim from source: identifiers serialize their raw slice so
            // escapes survive (`\@foo` stays `\@foo`), a string keeps its surrounding quotes,
            // and a comment is included verbatim.
            match &parser.current_kind {
                TokenKind::String { quote } => {
                    let content =
                        &parser.source()[parser.current_start + 1..parser.current_end - 1];
                    part_buf.push(*quote);
                    part_buf.push_str(content);
                    part_buf.push(*quote);
                }
                _ => part_buf.push_str(parser.current_value()),
            }
            trailing_spaces = 0;
            let current_kind = parser.current_kind;
            end_pos = parser.base_offset() + parser.current_end;
            parser.advance()?;

            // Add space after boolean operators
            if is_bool_op && !parser.check(TokenKind::Whitespace) {
                part_buf.push(' ');
                trailing_spaces += 1;
            }

            // Add space after comment if followed by non-whitespace
            // (e.g., `/* comment */ grid` needs space before `grid`)
            if is_comment
                && !parser.check(TokenKind::Whitespace)
                && !parser.check(TokenKind::RightParen)
            {
                part_buf.push(' ');
                trailing_spaces += 1;
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
                part_buf.push(' ');
                trailing_spaces += 1;
            }

            prev_token_kind = Some(current_kind);
            if !matches!(current_kind, TokenKind::Whitespace) {
                last_non_whitespace_kind = Some(current_kind);
            }
        }

        // Build the part
        let content = part_buf.trim();
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

/// Parse one `@scope` clause — `(<forgiving-selector-list>)` — with the in-paren
/// leading/trailing gap comments registered (the printer re-emits them from the AST via
/// `comments_to_emit_in_range`, the same wrapping `:is()` args use). Assumes the current token
/// is `(`. The list is **forgiving** (css-cascade-6 makes `<scope-start>`/`<scope-end>`
/// `<forgiving-selector-list>`s, the same production `:is()`/`:where()` use), so an empty
/// or invalid list — `@scope ()`, `@scope (.a, , .b)`, `@scope (.)` — parses (each is
/// kept verbatim), matching parseCss (which captures the prelude raw) and prettier.
/// `what` names the clause for the unterminated-`)` error.
fn parse_scope_clause<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    what: &str,
) -> Result<ScopeClause<'arena>, ParseError> {
    let paren_start = parser.span_pos(parser.current_start);
    parser.advance()?; // consume '('
    parser.skip_whitespace_registering_comments()?; // leading comment
    let list = parse_forgiving_selector_list(parser)?;
    parser.skip_whitespace_registering_comments()?; // trailing comment
    if !parser.check(TokenKind::RightParen) {
        return Err(parser.error_expected_after("')'", what));
    }
    let paren_end = parser.span_pos(parser.current_end);
    parser.advance()?; // consume ')'
    Ok(ScopeClause {
        list,
        paren: Span {
            start: paren_start,
            end: paren_end,
        },
    })
}

/// Parse an @scope prelude into a `PreludeValue::Selectors`.
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
) -> Result<PreludeValue<'arena>, ParseError> {
    // Leading gap comments (`@scope /* c */ …`) register here: the shared at-rule
    // name skip in `parse_atrule` is a plain skip that stops at a comment, so a leading
    // comment is the current token on entry. Capturing `start` *after* this keeps it out
    // of the wire prelude (extracted from `span`), matching parseCss, which drops it.
    parser.skip_whitespace_registering_comments()?;
    let start = parser.span_pos(parser.current_start);
    // Widens to each clause's closing `)`; stays at `start` when no clause is present.
    let mut end = start;

    // Optional root clause `(<scope-start>)`. After it, the between-clause (`) /* c */ to`)
    // and pre-`{` gaps register their comments so the printer can re-emit them.
    let root = if parser.check(TokenKind::LeftParen) {
        let clause = parse_scope_clause(parser, "@scope root selectors")?;
        end = clause.paren.end;
        parser.skip_whitespace_registering_comments()?; // between-clause / pre-`{` comment
        Some(clause)
    } else {
        None
    };

    // Optional limit clause `to (<scope-end>)` — valid with or without a root. `to` is a
    // case-insensitive grammar keyword (lowercased at the printer's ` to ` literal); its
    // span lets the printer tell a between-clause comment (before `to`) from an after-`to`
    // one. The after-`to` and pre-`{` gaps register comments the same way.
    let limit = if parser.check(TokenKind::Identifier)
        && parser.current_identifier().eq_ignore_ascii_case("to")
    {
        let to_span = Span {
            start: parser.span_pos(parser.current_start),
            end: parser.span_pos(parser.current_end),
        };
        parser.advance()?; // consume "to"
        parser.skip_whitespace_registering_comments()?; // after-`to` comment
        if !parser.check(TokenKind::LeftParen) {
            return Err(parser.error_expected_after("'('", "'to' in @scope prelude"));
        }
        let clause = parse_scope_clause(parser, "@scope limit selectors")?;
        end = clause.paren.end;
        parser.skip_whitespace_registering_comments()?; // pre-`{` comment
        Some(ScopeLimit { to_span, clause })
    } else {
        None
    };

    Ok(PreludeValue::Selectors {
        root,
        limit,
        span: Span { start, end },
    })
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
/// - `url('a.css') screen and (min-width: 5px)` (media-type-led query)
/// - `url('b.css') (max-width: 40px)` (bare `<media-condition>` query)
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
    if is_function_token(parser) {
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
    } else if matches!(parser.current_kind, TokenKind::Url)
        && !url_token_has_unclosed_paren(&parser.source()[parser.current_start..parser.current_end])
    {
        // Unquoted `url(...)` — the lexer consumed it as one opaque `<url-token>`. Mirror
        // `parse_function_value`'s empty-args url shape (name + span): the printer and the
        // public-AST conversion reconstruct the verbatim, inner-ws-trimmed `url(...)` from
        // the function span, so structured `@import url(…) layer/supports/media` wrapping
        // still works (unlike a raw fallback, which would drop that structure).
        //
        // A url-token with a *nested* `(` (e.g. `url(a(b))`) is excluded above: the lexer
        // stops the url scan at the first unescaped `)` (css-syntax §4.3.6), truncating the
        // token to `url(a(b)` and leaving a dangling `)`, so the structured split would
        // reject at the trailing `)`. parseCss reads such a prelude raw to `;` (and prettier
        // prints it verbatim), so fall through to the raw path below — the same one
        // `@namespace url(a(b))` already takes.
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
        if is_function_token(parser) {
            // layer() or supports() function
            values.push(parse_function_value(parser)?);
            parser.skip_whitespace_registering_comments()?;
        } else if parser.check(TokenKind::Identifier) && parser.current_identifier() == "layer" {
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
        } else if parser.check(TokenKind::Identifier) || parser.check(TokenKind::LeftParen) {
            // Media-query-list — the last prelude component (css-cascade-5
            // §import-conditions). Consume the rest verbatim to `;`/EOF, preserving
            // original whitespace; the text is recovered from `span` at print time.
            // A query may lead with a media type (`screen and (…)`, an identifier) OR a
            // bare `<media-condition>` (`(max-width: 40px)`, `(width < 100px)`, a `(`) —
            // Media Queries 4 §media-query makes a lone `<media-condition>` a valid query,
            // so both starts are accepted. Any other leading token (e.g. a stray `)`) is
            // not a media-query start and falls through to the reject below.
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
        } else {
            // Not a media-query start (e.g. a stray `)`); leave it for the caller to
            // reject as an unterminated at-rule prelude.
            break;
        }
    }

    let end = values.last().map_or(start, |v| v.span().end);

    Ok(PreludeValue::Values {
        values: values.into_bump_slice(),
        span: Span { start, end },
    })
}

/// Whether an unquoted `<url-token>`'s text has an unclosed `(` — i.e. the lexer stopped
/// the url scan at the first unescaped `)` (css-syntax §4.3.6) *inside* a nested group, so
/// the token is a truncated `url(a(b)` rather than a balanced `url(...)`. Escape-aware: a
/// `\(` / `\)` is literal url content, not a paren delimiter (matching the lexer's own
/// scan), so `url(a\(b)` (balanced) and `url(a\)b)` (escaped close) both read as closed.
fn url_token_has_unclosed_paren(text: &str) -> bool {
    let mut depth: u32 = 0;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                chars.next(); // the escaped code point is content, never a delimiter
            }
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth > 0
}

/// Parse a function value (e.g., url(), layer(), supports())
/// Whether the current token is a CSS `<function-token>` — an identifier
/// immediately followed by `(` with no intervening whitespace (`url(`, `layer(`,
/// `supports(`). The lexer emits the name and `(` as separate tokens (only an
/// unquoted `url(...)` is one opaque `Url` token), so the function-vs-plain-ident
/// distinction is recovered here by peeking the source byte after the identifier.
fn is_function_token(parser: &CssParser<'_, '_>) -> bool {
    parser.check(TokenKind::Identifier) && {
        let end_pos = parser.current_end;
        parser.source.get(end_pos..=end_pos) == Some("(")
    }
}

fn parse_function_value<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<CssValue<'arena>, ParseError> {
    let value_start = parser.span_pos(parser.current_start);

    // Get function name (current token should be identifier)
    let name = if parser.check(TokenKind::Identifier) {
        parser.current_identifier_in_arena()
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
        // Other unknown functions (e.g. `scope((.a) to (.b))`, css-cascade-6 scoped
        // `@import`) — consume the args opaquely to the MATCHING `)`. Per CSS Syntax 3
        // §consume-a-function the contents are a component-value list, so a nested
        // `(…)`/`fn(…)` must not end the arg at its first inner `)`; track paren depth
        // like the `supports` branch above. Args stay empty — the printer and public-AST
        // conversion reconstruct the function verbatim from its span.
        let mut depth: u32 = 0;
        while !parser.check(TokenKind::Eof) {
            match parser.current_kind {
                TokenKind::RightParen if depth == 0 => break,
                TokenKind::LeftParen => depth += 1,
                TokenKind::RightParen => depth -= 1,
                _ => {}
            }
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
