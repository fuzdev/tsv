use super::CssParser;
use super::selectors::parse_complex_selector_list;
use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

/// Check if current token is a CSS boolean operator keyword (and, or, not)
fn is_boolean_operator(parser: &CssParser) -> bool {
    if let TokenKind::Identifier = &parser.current_kind {
        let identifier = parser
            .current_identifier()
            .unwrap_or_else(|| parser.current_value());
        matches!(identifier, "and" | "or" | "not")
    } else {
        false
    }
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
    parser: &mut CssParser,
) -> Result<(SelectorList, Option<SelectorList>, Span), ParseError> {
    let start = parser.base_offset() + parser.current_start;

    // Expect opening paren
    if !parser.check(&TokenKind::LeftParen) {
        return Err(ParseError::InvalidSyntax {
            message: "Expected '(' in @scope prelude".to_string(),
            position: parser.base_offset() + parser.current_start,
            context: None,
        });
    }
    parser.advance()?; // consume '('
    parser.skip_whitespace()?;

    // Parse root selector list
    let root = parse_complex_selector_list(parser)?;

    parser.skip_whitespace()?;

    // Expect closing paren
    if !parser.check(&TokenKind::RightParen) {
        return Err(ParseError::InvalidSyntax {
            message: "Expected ')' after @scope root selectors".to_string(),
            position: parser.base_offset() + parser.current_start,
            context: None,
        });
    }
    let end_after_root_paren = parser.base_offset() + parser.current_end;
    parser.advance()?; // consume ')'

    // Note: Don't skip whitespace yet - we need to check for "to" keyword
    // Check for optional "to" clause
    parser.skip_whitespace()?;
    let (limit, end_pos) = if parser.check(&TokenKind::Identifier) {
        let identifier = parser
            .current_identifier()
            .unwrap_or_else(|| parser.current_value());
        if identifier == "to" {
            parser.advance()?; // consume "to"
            parser.skip_whitespace()?;

            // Expect opening paren
            if !parser.check(&TokenKind::LeftParen) {
                return Err(ParseError::InvalidSyntax {
                    message: "Expected '(' after 'to' in @scope prelude".to_string(),
                    position: parser.base_offset() + parser.current_start,
                    context: None,
                });
            }
            parser.advance()?; // consume '('
            parser.skip_whitespace()?;

            // Parse limit selector list
            let limit_selectors = parse_complex_selector_list(parser)?;

            parser.skip_whitespace()?;

            // Expect closing paren
            if !parser.check(&TokenKind::RightParen) {
                return Err(ParseError::InvalidSyntax {
                    message: "Expected ')' after @scope limit selectors".to_string(),
                    position: parser.base_offset() + parser.current_start,
                    context: None,
                });
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
fn parse_import_prelude(parser: &mut CssParser) -> Result<(Vec<CssValue>, Span), ParseError> {
    let start = parser.base_offset() + parser.current_start;
    let mut values = Vec::new();

    parser.skip_whitespace()?;

    // Parse first value: url() function or bare string
    let is_function = parser.check(&TokenKind::Identifier) && {
        // Check if next char in source is '(' (function call)
        let end_pos = parser.current_end;
        parser.source.get(end_pos..=end_pos) == Some("(")
    };

    if is_function {
        // url() function
        values.push(parse_function_value(parser)?);
    } else if let TokenKind::String { content, quote } = &parser.current_kind {
        // Bare string
        let value_start = (parser.base_offset() + parser.current_start) as u32;
        let value_end = (parser.base_offset() + parser.current_end) as u32;
        values.push(CssValue::String {
            content: content.clone(),
            quote: *quote,
            span: Span {
                start: value_start,
                end: value_end,
            },
        });
        parser.advance()?;
    } else {
        return Err(ParseError::InvalidSyntax {
            message: "@import expects url() or string".to_string(),
            position: parser.base_offset() + parser.current_start,
            context: None,
        });
    }

    parser.skip_whitespace()?;

    // Parse optional layer(), supports() functions and other conditions
    while !parser.check(&TokenKind::Semicolon) && !parser.check(&TokenKind::Eof) {
        let is_function = parser.check(&TokenKind::Identifier) && {
            let end_pos = parser.current_end;
            parser.source.get(end_pos..=end_pos) == Some("(")
        };

        if is_function {
            // layer() or supports() function
            values.push(parse_function_value(parser)?);
            parser.skip_whitespace()?;
        } else if parser.check(&TokenKind::Identifier) {
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
                parser.skip_whitespace()?;
            } else {
                // Media query - capture the rest as raw identifier/text
                // Build up remaining tokens until semicolon
                let media_start = (parser.base_offset() + parser.current_start) as u32;
                let mut media_parts = Vec::new();

                while !parser.check(&TokenKind::Semicolon) && !parser.check(&TokenKind::Eof) {
                    media_parts.push(parser.current_value().to_string());
                    let media_end = (parser.base_offset() + parser.current_end) as u32;
                    parser.advance()?;

                    // Store as a single identifier with the complete media query text
                    if parser.check(&TokenKind::Semicolon) || parser.check(&TokenKind::Eof) {
                        values.push(CssValue::Identifier {
                            name: media_parts.join(" "),
                            span: Span {
                                start: media_start,
                                end: media_end,
                            },
                        });
                        break;
                    }

                    // Add space between tokens
                    if !parser.check(&TokenKind::Whitespace) {
                        media_parts.push(" ".to_string());
                    }
                }
                break;
            }
        } else {
            break;
        }
    }

    let end = if values.is_empty() {
        start as u32
    } else {
        // SAFETY: We just checked is_empty() is false
        #[allow(clippy::unwrap_used)]
        { values.last().unwrap().span().end }
    };

    Ok((
        values,
        Span {
            start: start as u32,
            end,
        },
    ))
}

/// Parse a function value (e.g., url(), layer(), supports())
fn parse_function_value(parser: &mut CssParser) -> Result<CssValue, ParseError> {
    let value_start = (parser.base_offset() + parser.current_start) as u32;

    // Get function name (current token should be identifier)
    let name = if parser.check(&TokenKind::Identifier) {
        parser
            .current_identifier()
            .unwrap_or_else(|| parser.current_value())
            .to_string()
    } else {
        return Err(ParseError::InvalidSyntax {
            message: "Expected function name".to_string(),
            position: parser.base_offset() + parser.current_start,
            context: None,
        });
    };

    parser.advance()?; // consume function name

    // Expect '('
    if !parser.check(&TokenKind::LeftParen) {
        return Err(ParseError::InvalidSyntax {
            message: "Expected ( after function name".to_string(),
            position: parser.base_offset() + parser.current_start,
            context: None,
        });
    }

    parser.advance()?; // consume '('

    // For @import functions (url, layer, supports), parse arguments based on function type
    let mut args = Vec::new();

    if name == "url" {
        // url() - parse the URL argument (string or bare URL)
        parser.skip_whitespace()?;
        if let TokenKind::String { content, quote } = &parser.current_kind {
            let arg_start = (parser.base_offset() + parser.current_start) as u32;
            let arg_end = (parser.base_offset() + parser.current_end) as u32;
            args.push(CssValue::String {
                content: content.clone(),
                quote: *quote,
                span: Span {
                    start: arg_start,
                    end: arg_end,
                },
            });
            parser.advance()?;
        }
        parser.skip_whitespace()?;
    } else if name == "layer" {
        // layer(name) - parse the layer name as identifier
        parser.skip_whitespace()?;
        if parser.check(&TokenKind::Identifier) {
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

        while !parser.check(&TokenKind::RightParen) && !parser.check(&TokenKind::Eof) {
            // Skip whitespace after '(' or before ')'
            if parser.check(&TokenKind::Whitespace) {
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
                TokenKind::String { content, quote } => format!("{quote}{content}{quote}"),
                TokenKind::Number(n) => n.to_string(),
                TokenKind::Percentage(n) => format!("{n}%"),
                TokenKind::Dimension(n, unit) => format!("{n}{unit}"),
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

            let current_kind = parser.current_kind.clone();
            condition_end = (parser.base_offset() + parser.current_end) as u32;
            parser.advance()?;

            // Add space after boolean operators or ':'
            if !parser.check(&TokenKind::Whitespace) {
                if is_bool_op {
                    condition_parts.push(" ".to_string());
                } else if matches!(current_kind, TokenKind::Colon) {
                    // Add space after ':' in property:value pairs
                    if matches!(
                        last_non_whitespace_kind,
                        Some(TokenKind::Identifier)
                            | Some(TokenKind::Number(_))
                            | Some(TokenKind::Dimension(_, _))
                            | Some(TokenKind::Percentage(_))
                    ) {
                        condition_parts.push(" ".to_string());
                    }
                }
            }

            prev_token_kind = Some(current_kind.clone());
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
        while !parser.check(&TokenKind::RightParen) && !parser.check(&TokenKind::Eof) {
            parser.advance()?;
        }
    }

    if !parser.check(&TokenKind::RightParen) {
        return Err(ParseError::InvalidSyntax {
            message: "Expected ) to close function".to_string(),
            position: parser.base_offset() + parser.current_start,
            context: None,
        });
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
    parser: &mut CssParser,
    nested_in_rule: bool,
) -> Result<CssAtrule, ParseError> {
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
    } else {
        // Parse as raw string for other at-rules (@media, @keyframes, @supports, etc.)
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
        //
        // Spec references:
        // @scope: ../csswg-drafts/css-cascade-6/Overview.bs:439
        // @media: ../csswg-drafts/css-conditional-3/Overview.bs:268
        // @container: ../csswg-drafts/css-conditional-5/Overview.bs:977
        // @supports: ../csswg-drafts/css-conditional-3/Overview.bs
        // @starting-style: ../csswg-drafts/css-transitions-2/Overview.bs:215 (NO prelude)
        let is_selector_list_prelude = matches!(name.as_str(), "scope");

        while !parser.check(&TokenKind::LeftBrace)
            && !parser.check(&TokenKind::Semicolon)
            && !parser.check(&TokenKind::Eof)
        {
            if parser.check(&TokenKind::Whitespace) {
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
                    || (is_selector_list_prelude
                        && matches!(prev_token_kind, Some(TokenKind::Equals)))
                    || (is_selector_list_prelude && matches!(parser.peek(), Ok(TokenKind::Equals)));

                parser.advance()?;

                if skip_whitespace {
                    continue;
                }
                prelude_parts.push(" ".to_string());
                prev_token_kind = Some(TokenKind::Whitespace);
                continue;
            }

            let part = match &parser.current_kind {
                // Internal AST: use decoded value (spec-compliant)
                TokenKind::Identifier => parser
                    .current_identifier()
                    .unwrap_or_else(|| parser.current_value())
                    .to_string(),
                TokenKind::String { content, quote } => format!("{quote}{content}{quote}"),
                TokenKind::Number(n) => n.to_string(),
                TokenKind::Percentage(n) => format!("{n}%"),
                TokenKind::Dimension(n, unit) => format!("{n}{unit}"),
                TokenKind::Comment(_) => {
                    // Include comments in prelude (Svelte includes them in the prelude string)
                    parser.current_value().to_string()
                }
                _ => parser.current_value().to_string(),
            };

            // Add space before boolean operators (and, or, not) if not preceded by whitespace
            // Note: @scope preludes are now parsed structurally, so they don't go through this code
            let is_bool_op = is_boolean_operator(parser);

            if is_bool_op && !matches!(prev_token_kind, Some(TokenKind::Whitespace)) {
                prelude_parts.push(" ".to_string());
            }

            // Remove trailing whitespace before ':' (CSS convention: property: value, not property : value)
            if matches!(parser.current_kind, TokenKind::Colon) {
                while prelude_parts.last().is_some_and(|s| s == " ") {
                    prelude_parts.pop();
                }
            }

            prelude_parts.push(part);

            let current_kind = parser.current_kind.clone();

            // Track parenthesis depth for selector detection
            if matches!(current_kind, TokenKind::LeftParen) {
                paren_depth += 1;
            } else if matches!(current_kind, TokenKind::RightParen) {
                paren_depth = paren_depth.saturating_sub(1);
            }

            parser.advance()?;

            // Add space after boolean operators, commas, or ':' if not followed by whitespace
            // Note: @scope preludes are now parsed structurally, so they don't go through this code
            if !parser.check(&TokenKind::Whitespace) {
                if is_bool_op {
                    prelude_parts.push(" ".to_string());
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
                                | Some(TokenKind::Number(_))
                                | Some(TokenKind::Dimension(_, _))
                                | Some(TokenKind::Percentage(_))
                        );

                    if should_add_space {
                        prelude_parts.push(" ".to_string());
                    }
                }
            }

            prev_token_kind = Some(current_kind.clone());
            // Track last non-whitespace token for colon spacing logic
            if !matches!(current_kind, TokenKind::Whitespace) {
                last_non_whitespace_kind = Some(current_kind);
            }
        }

        let content = prelude_parts.join("").trim().to_string();
        let prelude_end = parser.base_offset() + parser.current_start;
        PreludeValue::Raw {
            content,
            span: Span {
                start: prelude_start as u32,
                end: prelude_end as u32,
            },
        }
    };

    // Parse block (if present)
    let (block, end) = if parser.check(&TokenKind::LeftBrace) {
        let block = parse_atrule_block(parser, &name, nested_in_rule)?;
        let end = block.span.end;
        (Some(block), end)
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
/// Block contents depend on at-rule type and nesting context:
/// - @media, @supports, @layer: contain rules (top-level) or declarations (nested)
/// - @keyframes: contains keyframe blocks (rules with percentage/from/to selectors)
/// - @font-face, @page: contain declarations
///
/// `nested_in_rule`: true if this at-rule is nested inside a regular rule's declaration block
fn parse_atrule_block(
    parser: &mut CssParser,
    atrule_name: &str,
    nested_in_rule: bool,
) -> Result<CssAtruleBlock, ParseError> {
    let start = parser.base_offset() + parser.current_start;

    // Expect {
    parser.expect(&TokenKind::LeftBrace)?;
    parser.skip_whitespace()?;

    let mut children = Vec::new();

    // Determine what content to expect based on at-rule type and nesting context
    // When nested inside a rule, at-rules that normally contain rules should contain declarations instead
    let expect_rules = matches!(
        atrule_name,
        "media"
            | "supports"
            | "layer"
            | "keyframes"
            | "container"
            | "starting-style"
            | "scope"
            | "font-feature-values"
    ) && !nested_in_rule;
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
    ) || (matches!(
        atrule_name,
        "media" | "supports" | "layer" | "container" | "starting-style" | "scope"
    ) && nested_in_rule);

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
            // Nested at-rules inside at-rules are not "nested in rule" context
            let atrule = parse_atrule(parser, false)?;
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

        // Generic fallback for unknown at-rules: try to detect whether this is a declaration or rule
        // by checking if the current position looks like a nested rule start
        let looks_like_rule = super::declarations::is_nested_rule_start(parser)?;
        if looks_like_rule {
            // Parse as rule (selector + block)
            let rule = super::declarations::parse_rule(parser)?;
            children.push(CssBlockChild::Rule(rule));
            parser.skip_whitespace()?;
            continue;
        } else if parser.check(&TokenKind::Identifier) {
            // Parse as declaration (property: value)
            let decl = super::declarations::parse_declaration(parser)?;
            children.push(CssBlockChild::Declaration(decl));
            parser.skip_whitespace()?;
            continue;
        }

        // Fallback: unexpected token
        return Err(ParseError::InvalidSyntax {
            message: format!("Unexpected token in @{atrule_name} block"),
            position: parser.base_offset() + parser.current_start,
            context: None,
        });
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
