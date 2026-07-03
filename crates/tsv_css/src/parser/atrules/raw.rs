use super::{CssParser, is_boolean_operator};
use crate::lexer::TokenKind;
use crate::url::trim_url_raw;
use tsv_lang::printing::format_string_literal;
use tsv_lang::{ParseError, Span};

/// Build the raw at-rule prelude string (used for both the printer and, for `@media`,
/// the formatter's wrapping). `normalize_whitespace = true` (`@media`) collapses internal
/// whitespace and applies `property: value` / boolean-operator spacing; `false` (every
/// other raw at-rule — `@layer`, `@namespace`, `@keyframes`, …) preserves internal
/// whitespace verbatim, matching prettier and Svelte. `url()` inner whitespace is trimmed
/// in both modes (a spec-mandated `<url-token>` normalization).
pub(super) fn parse_raw_prelude_content<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    is_selector_list_prelude: bool,
    normalize_whitespace: bool,
    // `@namespace` normalizes string / `url()` quotes to prettier's single-quote form
    // (`"x"` → `'x'`, `url("x")` → `url('x')`, per the `singleQuote` option — the same
    // as declaration values and `@import`); other raw at-rules keep quotes verbatim.
    normalize_quotes: bool,
) -> Result<(&'arena str, Span), ParseError> {
    // Add spaces around boolean operators (and, or, not) and after ':' for prettier compatibility
    let prelude_start = parser.span_pos(parser.current_start);
    let mut prelude_parts = Vec::new();
    let mut prev_token_kind: Option<TokenKind> = None;
    let mut last_non_whitespace_kind: Option<TokenKind> = None;
    let mut paren_depth: u32 = 0; // Track parenthesis nesting for selector detection

    // Categorize at-rule by prelude type based on CSS specs:
    // - Selector list preludes (@scope): Format like CSS selectors (.widget:hover)
    // - Query preludes (@media, @container, @supports): Format like properties (min-width: 500px)
    // - No prelude (@font-face, @starting-style): No prelude to normalize
    // - Identifier preludes (@keyframes, @layer): No colons to worry about

    while !parser.at_prelude_end() {
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
                || matches!(parser.peek_kind(), Ok(TokenKind::RightParen))
                || (is_selector_list_prelude
                    && paren_depth > 0
                    && matches!(prev_token_kind, Some(TokenKind::Colon)))
                || (is_selector_list_prelude
                    && paren_depth > 0
                    && matches!(parser.peek_kind(), Ok(TokenKind::Comma)))
                || (is_selector_list_prelude
                    && matches!(prev_token_kind, Some(TokenKind::LeftBracket)))
                || (is_selector_list_prelude
                    && matches!(parser.peek_kind(), Ok(TokenKind::RightBracket)))
                || (is_selector_list_prelude && matches!(prev_token_kind, Some(TokenKind::Equals)))
                || (is_selector_list_prelude
                    && matches!(parser.peek_kind(), Ok(TokenKind::Equals)));

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
            && matches!(parser.peek_kind(), Ok(TokenKind::LeftParen))
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
                let trimmed = trim_url_raw(raw).unwrap_or_else(|| raw.to_string());
                if normalize_quotes {
                    normalize_url_string_quote(&trimmed)
                } else {
                    trimmed
                }
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
                if normalize_quotes {
                    format_string_literal(content, *quote)
                } else {
                    format!("{quote}{content}{quote}")
                }
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

            // Add space before comments or boolean operators if not already preceded
            // by space — but not for a boolean operator right after `(`: `(not …)`
            // stays tight (matching prettier and the structured @supports/@container
            // path's same `not`-after-`(` suppression).
            let after_open_paren = last_non_whitespace_kind == Some(TokenKind::LeftParen);
            if (is_comment || (is_bool_op && !after_open_paren)) && !has_trailing_space {
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

    let joined = prelude_parts.join("");
    let content = parser.alloc_str_in(joined.trim());
    let prelude_end = parser.span_pos(parser.current_start);
    let span = Span {
        start: prelude_start,
        end: prelude_end,
    };

    Ok((content, span))
}

/// Normalize the inner string quote of a `url("x")` / `url('x')` to prettier's
/// single-quote form (matching the `@import` and declaration-value url paths); an
/// unquoted `url(x)` is returned unchanged. Input is `trim_url_raw` output —
/// `url(<inner>)` with a lowercase, whitespace-trimmed `url(` (4 bytes).
fn normalize_url_string_quote(url: &str) -> String {
    if !url.starts_with("url(") || !url.ends_with(')') {
        return url.to_string();
    }
    let inner = &url[4..url.len() - 1];
    let b = inner.as_bytes();
    if b.len() >= 2 && (b[0] == b'"' || b[0] == b'\'') && b[b.len() - 1] == b[0] {
        format!(
            "url({})",
            format_string_literal(&inner[1..inner.len() - 1], b[0] as char)
        )
    } else {
        url.to_string()
    }
}
