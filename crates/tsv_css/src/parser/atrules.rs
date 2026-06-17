use super::CssParser;
use super::selectors::parse_complex_selector_list;
use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

/// Check if an at-rule name is a keyframes rule (including vendor-prefixed versions)
/// e.g., "keyframes", "-webkit-keyframes", "-moz-keyframes"
fn is_keyframes_atrule(name: &str) -> bool {
    name == "keyframes"
        || name == "-webkit-keyframes"
        || name == "-moz-keyframes"
        || name == "-o-keyframes"
        || name == "-ms-keyframes"
}

/// Check if current token is a CSS boolean operator keyword (and, or, not)
fn is_boolean_operator(parser: &CssParser<'_>) -> bool {
    if let TokenKind::Identifier = &parser.current_kind {
        let identifier = parser
            .current_identifier()
            .unwrap_or_else(|| parser.current_value());
        matches!(identifier, "and" | "or" | "not")
    } else {
        false
    }
}

/// Parse @supports prelude into structured condition parts
///
/// CSS Syntax: `@supports <supports-condition>`
/// where supports-condition is a combination of `(prop: val)` parts connected by `and`/`or`
///
/// Examples:
/// - `(display: grid)` - single condition
/// - `(display: grid) and (flex: 1)` - conjunction
/// - `not (color: red)` - negation
/// - `(a) and (b) or (c)` - mixed (parsed left-to-right)
fn parse_supports_prelude(
    parser: &mut CssParser<'_>,
) -> Result<(SupportsCondition, Span), ParseError> {
    let start = parser.base_offset() + parser.current_start;
    let mut parts = Vec::new();
    let mut current_connector: Option<SupportsConnector> = None;
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
                    SupportsConnector::And
                } else {
                    SupportsConnector::Or
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
        let content = part_content.join("").trim().to_string();
        if !content.is_empty() {
            parts.push(SupportsPart {
                connector: current_connector.take(),
                content,
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

    Ok((SupportsCondition { parts }, span))
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
fn parse_container_prelude(
    parser: &mut CssParser<'_>,
) -> Result<(Option<String>, SupportsCondition, Span), ParseError> {
    let start = parser.base_offset() + parser.current_start;

    // Check for optional container name (identifier before first '(')
    // Container name is an identifier followed by whitespace then '('
    // NOT a function call like style(...) where there's no whitespace
    let container_name = if parser.check(TokenKind::Identifier) {
        let ident = parser
            .current_identifier()
            .unwrap_or_else(|| parser.current_value())
            .to_string();
        // Check if this is actually a name (not 'not' or 'and' or 'or')
        // Also check it's not a function call (identifier directly followed by '(')
        let is_function_call =
            parser.source.get(parser.current_end..=parser.current_end) == Some("(");
        if !matches!(ident.as_str(), "not" | "and" | "or") && !is_function_call {
            parser.advance()?;
            parser.skip_whitespace()?;
            Some(ident)
        } else {
            None
        }
    } else {
        None
    };

    // Now parse the condition parts (same logic as @supports)
    let mut parts = Vec::new();
    let mut current_connector: Option<SupportsConnector> = None;
    let mut end_pos = parser.base_offset() + parser.current_start;

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
                current_connector = Some(if ident == "and" {
                    SupportsConnector::And
                } else {
                    SupportsConnector::Or
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

        // Check for function-style condition like `style(--custom: value)`
        if parser.check(TokenKind::Identifier) {
            let ident = parser
                .current_identifier()
                .unwrap_or_else(|| parser.current_value());
            let is_function =
                parser.source.get(parser.current_end..=parser.current_end) == Some("(");
            if is_function {
                part_content.push(ident.to_string());
                parser.advance()?;
            }
        }

        // Now parse the parenthesized condition
        if !parser.check(TokenKind::LeftParen) {
            break;
        }

        // Parse until we close all parens
        let mut prev_token_kind: Option<TokenKind> = None;
        let mut last_non_whitespace_kind: Option<TokenKind> = None;

        while !parser.check(TokenKind::Eof) {
            if parser.check(TokenKind::LeftParen) {
                paren_depth += 1;
            } else if parser.check(TokenKind::RightParen) {
                if paren_depth == 0 {
                    break;
                }
                paren_depth -= 1;
            }

            if paren_depth == 0 && parser.check(TokenKind::RightParen) {
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
                TokenKind::Comment => parser.current_value().to_string(),
                _ => parser.current_value().to_string(),
            };

            let is_comment = matches!(parser.current_kind, TokenKind::Comment);
            let is_bool_op = matches!(&parser.current_kind, TokenKind::Identifier)
                && matches!(part.as_str(), "and" | "or" | "not");

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

            if is_bool_op && !parser.check(TokenKind::Whitespace) {
                part_content.push(" ".to_string());
            }

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

        let content = part_content.join("").trim().to_string();
        if !content.is_empty() {
            parts.push(SupportsPart {
                connector: current_connector.take(),
                content,
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

    Ok((container_name, SupportsCondition { parts }, span))
}

/// Parse @scope prelude into structured selector lists
///
/// CSS Syntax: `@scope (<scope-start>) [to (<scope-end>)]`
///
/// Examples:
/// - `(.card)` - scope root only
/// - `(.card) to (.footer)` - scope root and limit
/// - `(article > header)` - with combinator
fn parse_scope_prelude(
    parser: &mut CssParser<'_>,
) -> Result<(SelectorList, Option<SelectorList>, Span), ParseError> {
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
fn parse_import_prelude(parser: &mut CssParser<'_>) -> Result<(Vec<CssValue>, Span), ParseError> {
    let start = parser.base_offset() + parser.current_start;
    let mut values = Vec::new();

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
    } else if let TokenKind::String { quote } = &parser.current_kind {
        // Bare string
        let value_start = (parser.base_offset() + parser.current_start) as u32;
        let value_end = (parser.base_offset() + parser.current_end) as u32;
        // Extract content without quotes
        let content = parser.source()[parser.current_start + 1..parser.current_end - 1].to_string();
        values.push(CssValue::String {
            content,
            quote: *quote,
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
                .unwrap_or_else(|| parser.current_value())
                .to_string();

            if ident == "layer" {
                // Bare "layer" keyword (without function call)
                let value_start = (parser.base_offset() + parser.current_start) as u32;
                let value_end = (parser.base_offset() + parser.current_end) as u32;
                values.push(CssValue::Identifier {
                    name: ident,
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
                let name = parser.source()[media_local_start..media_local_end].to_string();

                if !name.is_empty() {
                    values.push(CssValue::Identifier {
                        name,
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
        values,
        Span {
            start: start as u32,
            end,
        },
    ))
}

/// Parse a function value (e.g., url(), layer(), supports())
fn parse_function_value(parser: &mut CssParser<'_>) -> Result<CssValue, ParseError> {
    let value_start = (parser.base_offset() + parser.current_start) as u32;

    // Get function name (current token should be identifier)
    let name = if parser.check(TokenKind::Identifier) {
        parser
            .current_identifier()
            .unwrap_or_else(|| parser.current_value())
            .to_string()
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
    let mut args = Vec::new();

    if name == "url" {
        // url() - parse the URL argument (string or bare URL)
        parser.skip_whitespace()?;
        if let TokenKind::String { quote } = &parser.current_kind {
            let arg_start = (parser.base_offset() + parser.current_start) as u32;
            let arg_end = (parser.base_offset() + parser.current_end) as u32;
            // Extract content without quotes
            let content =
                parser.source()[parser.current_start + 1..parser.current_end - 1].to_string();
            args.push(CssValue::String {
                content,
                quote: *quote,
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
            let ident = parser
                .current_identifier()
                .unwrap_or_else(|| parser.current_value())
                .to_string();
            args.push(CssValue::Identifier {
                name: ident,
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

        // Store the normalized condition text as an identifier
        let condition_text = condition_parts.join("").trim().to_string();
        if !condition_text.is_empty() {
            args.push(CssValue::Identifier {
                name: condition_text,
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
        args,
        span: Span {
            start: value_start,
            end: value_end,
        },
    })
}

/// Parse a CSS at-rule: `@media (...) { ... }` or `@import "...";`
///
/// `nested_in_rule`: true if this at-rule is nested inside a regular rule's declaration block
pub(crate) fn parse_atrule(
    parser: &mut CssParser<'_>,
    nested_in_rule: bool,
) -> Result<CssAtrule, ParseError> {
    let start = parser.base_offset() + parser.current_start;

    // Expect @ symbol
    parser.expect(TokenKind::AtSign)?;

    // Parse at-rule name (identifier after @)
    if !parser.check(TokenKind::Identifier) {
        return Err(parser.error_expected_after("at-rule name", "@"));
    }

    // Internal AST: use decoded value (spec-compliant)
    let name = parser
        .current_identifier()
        .unwrap_or_else(|| parser.current_value())
        .to_string();
    parser.advance()?;

    parser.skip_whitespace()?;

    // Parse prelude based on at-rule type
    let prelude = if name == "import" {
        // Parse @import prelude structurally (url/string + layer/supports/media)
        let (values, span) = parse_import_prelude(parser)?;
        PreludeValue::Values { values, span }
    } else if name == "scope" {
        // Parse @scope prelude as structured selector lists
        let (root, limit, span) = parse_scope_prelude(parser)?;
        PreludeValue::Selectors { root, limit, span }
    } else if name == "supports" {
        // Parse @supports prelude as structured conditions (for line-width wrapping)
        let (condition, span) = parse_supports_prelude(parser)?;
        PreludeValue::Supports { condition, span }
    } else if name == "container" {
        // Parse @container prelude as structured conditions (for line-width wrapping)
        let (name, condition, span) = parse_container_prelude(parser)?;
        PreludeValue::Container {
            name,
            condition,
            span,
        }
    } else if name == "media" {
        // Parse @media as raw string to preserve comments
        // Wrapping is handled in the printer by finding and/or boundaries
        // Fully structuring preludes is a deferred design option — see
        // docs/architecture.md § "Red-Green Trees (Deferred)"
        let (content, span) = parse_raw_prelude_content(parser, false, true)?;
        PreludeValue::Media { content, span }
    } else {
        // Parse as raw string for other at-rules (@keyframes, @layer, @page, etc.).
        // Most have no `property: value` / media-query grammar, so prettier keeps the
        // prelude verbatim (outer-trimmed; only `url()` inner whitespace is trimmed) —
        // preserve internal whitespace (`@layer a , b` must not become `a, b`).
        // `@namespace` is the exception: prettier re-parses its prelude as a value (see
        // postcss `parser-postcss.js`), normalizing whitespace to single spaces, so it
        // takes the normalizing path. The public AST stays source-verbatim either way
        // (the printer-facing `content` is what differs); see `convert.rs`.
        let normalize_whitespace = name == "namespace";
        let (content, span) = parse_raw_prelude_content(parser, false, normalize_whitespace)?;
        PreludeValue::Raw { content, span }
    };

    // Parse block (if present)
    let (block, end) = if parser.check(TokenKind::LeftBrace) {
        let block = parse_atrule_block(parser, &name, nested_in_rule)?;
        let end = block.span.end;
        (Some(block), end)
    } else if parser.check(TokenKind::Semicolon) {
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
        return Err(parser.error_expected_after("'{' or ';'", "at-rule prelude"));
    };

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

/// Build the raw at-rule prelude string (used for both the printer and, for `@media`,
/// the formatter's wrapping). `normalize_whitespace = true` (`@media`) collapses internal
/// whitespace and applies `property: value` / boolean-operator spacing; `false` (every
/// other raw at-rule — `@layer`, `@namespace`, `@keyframes`, …) preserves internal
/// whitespace verbatim, matching prettier and Svelte. `url()` inner whitespace is trimmed
/// in both modes (a spec-mandated `<url-token>` normalization).
fn parse_raw_prelude_content(
    parser: &mut CssParser<'_>,
    is_selector_list_prelude: bool,
    normalize_whitespace: bool,
) -> Result<(String, Span), ParseError> {
    // Add spaces around boolean operators (and, or, not) and after ':' for prettier compatibility
    let prelude_start = parser.base_offset() + parser.current_start;
    let mut prelude_parts = Vec::new();
    let mut prev_token_kind: Option<TokenKind> = None;
    let mut last_non_whitespace_kind: Option<TokenKind> = None;
    let mut paren_depth: u32 = 0; // Track parenthesis nesting for selector detection

    // Categorize at-rule by prelude type based on CSS specs:
    // - Selector list preludes (@scope): Format like CSS selectors (.widget:hover)
    // - Query preludes (@media, @container, @supports): Format like properties (min-width: 500px)
    // - No prelude (@font-face, @starting-style): No prelude to normalize
    // - Identifier preludes (@keyframes, @layer): No colons to worry about

    while !parser.check(TokenKind::LeftBrace)
        && !parser.check(TokenKind::Semicolon)
        && !parser.check(TokenKind::Eof)
    {
        if parser.check(TokenKind::Whitespace) {
            // Verbatim mode (non-@media raw at-rules): preserve the source whitespace
            // exactly — prettier and Svelte keep it (`@layer a  ,  b` stays `a  ,  b`).
            if !normalize_whitespace {
                let ws = parser.current_value().to_string();
                parser.advance()?;
                prelude_parts.push(ws);
                prev_token_kind = Some(TokenKind::Whitespace);
                continue;
            }
            // Skip whitespace in selector list preludes (inside parentheses for @scope):
            // - After '(' or before ')'
            // - After ':' (pseudo-classes like :hover) - only for selector list preludes
            // - Before ',' (selector lists) - only for selector list preludes
            // - After '[' or before ']' (attribute selectors) - only for selector list preludes
            // - Before/after '=' (attribute selectors) - only for selector list preludes
            let skip_whitespace = matches!(prev_token_kind, Some(TokenKind::LeftParen))
                || matches!(parser.peek(), Ok(TokenKind::RightParen))
                || (is_selector_list_prelude
                    && paren_depth > 0
                    && matches!(prev_token_kind, Some(TokenKind::Colon)))
                || (is_selector_list_prelude
                    && paren_depth > 0
                    && matches!(parser.peek(), Ok(TokenKind::Comma)))
                || (is_selector_list_prelude
                    && matches!(prev_token_kind, Some(TokenKind::LeftBracket)))
                || (is_selector_list_prelude
                    && matches!(parser.peek(), Ok(TokenKind::RightBracket)))
                || (is_selector_list_prelude && matches!(prev_token_kind, Some(TokenKind::Equals)))
                || (is_selector_list_prelude && matches!(parser.peek(), Ok(TokenKind::Equals)));

            parser.advance()?;

            if skip_whitespace {
                continue;
            }
            prelude_parts.push(" ".to_string());
            prev_token_kind = Some(TokenKind::Whitespace);
            continue;
        }

        // `url(...)` in a raw prelude (e.g. `@namespace url(http://…)`): the content is
        // an opaque `<url-token>`, not a `property: value` query, so raw-extract it
        // verbatim. Otherwise the property-colon normalization below inserts a space
        // after the `:` in `http://`, corrupting it to `http: //`. Shares the
        // declaration-path's `url::trim_url_raw` (and matches prettier's
        // `printer-postcss.js`) — only the whitespace just inside the parens is trimmed.
        // Quoted `url('…')` is preserved verbatim too (unchanged).
        // Detect `url` on the raw source slice, not the decoded identifier: `advance()`
        // drops the decoded value when a token arrives via the peek cache (which the
        // whitespace branch above populates), so `current_identifier()` is unreliable
        // here. A `url(` function token requires the literal `url`, so the raw slice is
        // also the correct thing to match. Match case-insensitively (so the opaque
        // content is raw-extracted, dodging the property-colon corruption, for `URL(`
        // too) but only *trim* the inner whitespace for the lowercase spelling: per
        // css-syntax-3 a `<url-token>` is matched ASCII-case-insensitively, yet prettier
        // (postcss) only canonicalizes the lowercase `url(`, preserving `URL(  …  )`
        // verbatim — so trimming uppercase would diverge from prettier.
        if matches!(parser.current_kind, TokenKind::Identifier)
            && parser.current_value().eq_ignore_ascii_case("url")
            && matches!(parser.peek(), Ok(TokenKind::LeftParen))
        {
            let is_lowercase_url = parser.current_value() == "url";
            let url_start = parser.current_start;
            parser.advance()?; // consume `url`
            // Consume the balanced parens, tracking depth so a nested `(` can't end it early.
            let mut depth: u32 = 0;
            let mut url_end;
            loop {
                match parser.current_kind {
                    TokenKind::LeftParen => depth += 1,
                    TokenKind::RightParen => depth = depth.saturating_sub(1),
                    TokenKind::Eof => {
                        url_end = parser.current_start;
                        break;
                    }
                    _ => {}
                }
                let is_close = depth == 0 && matches!(parser.current_kind, TokenKind::RightParen);
                url_end = parser.current_end;
                parser.advance()?;
                if is_close {
                    break;
                }
            }
            let raw = &parser.source()[url_start..url_end];
            let part = if is_lowercase_url {
                crate::url::trim_url_raw(raw).unwrap_or_else(|| raw.to_string())
            } else {
                raw.to_string()
            };
            prelude_parts.push(part);
            prev_token_kind = Some(TokenKind::RightParen);
            last_non_whitespace_kind = Some(TokenKind::RightParen);
            continue;
        }

        let part = match &parser.current_kind {
            // Use the raw source slice, not the decoded identifier: an at-rule prelude
            // is serialized verbatim (Svelte stores the raw string, prettier preserves
            // it), so escapes must survive — `@keyframes \@mymove` must not collapse to
            // `@keyframes @mymove` (which would re-parse as an at-rule) and `\31 23` must
            // not collapse to `123`.
            TokenKind::Identifier => parser.current_value().to_string(),
            TokenKind::String { quote } => {
                let content = &parser.source()[parser.current_start + 1..parser.current_end - 1];
                format!("{quote}{content}{quote}")
            }
            TokenKind::Number | TokenKind::Percentage | TokenKind::Dimension { .. } => {
                parser.current_value().to_string()
            }
            TokenKind::Comment => {
                // Include comments in prelude (Svelte includes them in the prelude string)
                parser.current_value().to_string()
            }
            _ => parser.current_value().to_string(),
        };

        // Add space before boolean operators (and, or, not) or comments if not preceded by space
        // Note: @scope preludes are now parsed structurally, so they don't go through this code
        let is_bool_op = is_boolean_operator(parser);
        let is_comment = matches!(parser.current_kind, TokenKind::Comment);

        // Whitespace-rewriting (property/boolean/comma spacing) applies only to the
        // normalized `@media` path; verbatim raw at-rules keep the source spacing.
        if normalize_whitespace {
            // Check if we already have a trailing space (from programmatic insertion or whitespace token)
            let has_trailing_space = prelude_parts.last().is_some_and(|s| s == " ");

            // Add space before comments or boolean operators if not already preceded by space
            if (is_comment || is_bool_op) && !has_trailing_space {
                prelude_parts.push(" ".to_string());
            }

            // Remove trailing whitespace before ':' or ',' (CSS convention: no space before these)
            if matches!(parser.current_kind, TokenKind::Colon | TokenKind::Comma) {
                while prelude_parts.last().is_some_and(|s| s == " ") {
                    prelude_parts.pop();
                }
            }
        }

        prelude_parts.push(part);

        let current_kind = parser.current_kind;

        // Track parenthesis depth for selector detection
        if matches!(current_kind, TokenKind::LeftParen) {
            paren_depth += 1;
        } else if matches!(current_kind, TokenKind::RightParen) {
            paren_depth = paren_depth.saturating_sub(1);
        }

        parser.advance()?;

        // Add space after boolean operators, comments, commas, or ':' if not followed by whitespace
        // Note: @scope preludes are now parsed structurally, so they don't go through this code
        if normalize_whitespace && !parser.check(TokenKind::Whitespace) {
            if is_bool_op {
                prelude_parts.push(" ".to_string());
            } else if is_comment {
                // Add space after comment, but not if followed by comma, close paren, or semicolon
                if !matches!(
                    parser.current_kind,
                    TokenKind::Comma | TokenKind::RightParen | TokenKind::Semicolon
                ) {
                    prelude_parts.push(" ".to_string());
                }
            } else if matches!(current_kind, TokenKind::Comma) {
                // Add space after comma in media queries (comma acts as OR)
                prelude_parts.push(" ".to_string());
            } else if matches!(current_kind, TokenKind::Colon) {
                // Add space after ':' for property:value pairs (preceded by identifier/number/dimension)
                // For selector list preludes (@scope): Don't add space inside parentheses (pseudo-classes like :hover)
                // For query preludes (@media, @supports, @container): Always add space (property:value in queries)
                // Use last_non_whitespace_kind to check (handles case where whitespace was removed before colon)
                let should_add_space = (!is_selector_list_prelude || paren_depth == 0)
                    && matches!(
                        last_non_whitespace_kind,
                        Some(TokenKind::Identifier)
                            | Some(TokenKind::Number)
                            | Some(TokenKind::Dimension { .. })
                            | Some(TokenKind::Percentage)
                    );

                if should_add_space {
                    prelude_parts.push(" ".to_string());
                }
            }
        }

        prev_token_kind = Some(current_kind);
        // Track last non-whitespace token for colon spacing logic
        if !matches!(current_kind, TokenKind::Whitespace) {
            last_non_whitespace_kind = Some(current_kind);
        }
    }

    let content = prelude_parts.join("").trim().to_string();
    let prelude_end = parser.base_offset() + parser.current_start;
    let span = Span {
        start: prelude_start as u32,
        end: prelude_end as u32,
    };

    Ok((content, span))
}
/// Parse an at-rule block: `{ ... }`
/// Block contents depend on at-rule type and nesting context:
/// - @media, @supports, @layer: contain rules (top-level) or declarations (nested)
/// - @keyframes: contains keyframe blocks (rules with percentage/from/to selectors)
/// - @font-face, @page: contain declarations
///
/// `nested_in_rule`: true if this at-rule is nested inside a regular rule's declaration block
fn parse_atrule_block(
    parser: &mut CssParser<'_>,
    atrule_name: &str,
    nested_in_rule: bool,
) -> Result<CssAtruleBlock, ParseError> {
    let start = parser.base_offset() + parser.current_start;

    // Expect {
    parser.expect(TokenKind::LeftBrace)?;
    parser.skip_whitespace()?;

    let mut children = Vec::new();

    // Determine what content to expect based on at-rule type and nesting context
    // When nested inside a rule, at-rules that normally contain rules should contain declarations instead
    let expect_rules = (matches!(
        atrule_name,
        "media"
            | "supports"
            | "layer"
            | "container"
            | "starting-style"
            | "scope"
            | "font-feature-values"
    ) || is_keyframes_atrule(atrule_name))
        && !nested_in_rule;
    let expect_declarations = matches!(
        atrule_name,
        "font-face" | "page" | "property" | "counter-style" | "color-profile" | "position-try" | "font-palette-values"
        // @font-feature-values nested at-rules (all contain declarations)
        | "stylistic" | "styleset" | "character-variant" | "swash" | "ornaments" | "annotation"
        // Page margin boxes (nested within @page, all contain declarations)
        | "top-left-corner" | "top-left" | "top-center" | "top-right" | "top-right-corner"
        | "left-top" | "left-middle" | "left-bottom"
        | "right-top" | "right-middle" | "right-bottom"
        | "bottom-left-corner" | "bottom-left" | "bottom-center" | "bottom-right" | "bottom-right-corner"
    );
    // Conditional group at-rules (@media, @supports, etc.) nested inside a rule
    // can contain BOTH declarations and nested rules — they fall through to the
    // generic fallback which uses is_nested_rule_start() to disambiguate.

    while !parser.check(TokenKind::RightBrace) && !parser.check(TokenKind::Eof) {
        if matches!(&parser.current_kind, TokenKind::Comment) {
            let comment = parser.parse_block_comment()?;
            children.push(CssBlockChild::Comment(comment));
            continue;
        }

        // Handle nested at-rules
        if parser.check(TokenKind::AtSign) {
            // Propagate the nesting context: an at-rule nested inside a conditional
            // group at-rule that is itself inside a style rule is still in a nesting
            // context, so its block holds declarations (per CSS Nesting). Outside a
            // rule, `nested_in_rule` is false and this stays a normal at-rule block.
            let atrule = parse_atrule(parser, nested_in_rule)?;
            children.push(CssBlockChild::Atrule(atrule));
            parser.skip_whitespace()?;
            continue;
        }

        // For @font-face and @page, parse declarations
        if expect_declarations && parser.check(TokenKind::Identifier) {
            let decl = super::declarations::parse_declaration(parser)?;
            children.push(CssBlockChild::Declaration(decl));
            parser.skip_whitespace()?;
            continue;
        }

        // For @media, @supports, @layer, @keyframes, parse rules
        if expect_rules {
            let rule = super::declarations::parse_rule(parser, false)?;
            children.push(CssBlockChild::Rule(rule));
            parser.skip_whitespace()?;
            continue;
        }

        // Generic fallback for unknown at-rules: try to detect whether this is a declaration or rule
        // by checking if the current position looks like a nested rule start
        let looks_like_rule = super::declarations::is_nested_rule_start(parser)?;
        if looks_like_rule {
            // Parse as rule (selector + block) — use nested=true to allow leading combinators
            let rule = super::declarations::parse_rule(parser, true)?;
            children.push(CssBlockChild::Rule(rule));
            parser.skip_whitespace()?;
            continue;
        } else if parser.check(TokenKind::Identifier) {
            // Parse as declaration (property: value)
            let decl = super::declarations::parse_declaration(parser)?;
            children.push(CssBlockChild::Declaration(decl));
            parser.skip_whitespace()?;
            continue;
        }

        // Fallback: unexpected token
        return Err(parser.error_unexpected(&format!("token in @{atrule_name} block")));
    }

    // Expect }
    if !parser.check(TokenKind::RightBrace) {
        return Err(parser.error_expected("'}'"));
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
