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
    // One growable buffer rather than a `Vec<String>` of per-token / per-space pieces
    // joined at the end (the perf10 `parse_declaration` idiom, extended to the at-rule
    // prelude siblings): each token value is `push_str`ed directly and each normalized
    // separator is a single `push(' ')`, so a `@media (min-width: …) and (…)` prelude
    // no longer allocs a heap `String` per token and per space.
    let mut prelude = String::new();
    // Count of trailing programmatically-inserted single spaces — the normalize path's
    // collapse unit. Tracked as a counter (truncate this many bytes) instead of scanning
    // `prelude.ends_with(' ')`, so an identifier whose raw slice ends in an escape-terminator
    // space (`\41 `, kept verbatim so escapes survive) is never mistaken for a collapsible
    // separator. Mirrors the old `Vec<String>` "last part is `\" \"`" test exactly.
    let mut trailing_spaces: usize = 0;
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
                // Verbatim mode never reads `trailing_spaces` (the collapse logic below is
                // normalize-only), so the raw whitespace goes straight in.
                prelude.push_str(parser.current_value());
                parser.advance()?;
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
            prelude.push(' ');
            trailing_spaces += 1;
            prev_token_kind = Some(TokenKind::Whitespace);
            continue;
        }

        // Unquoted `url(...)` in a raw prelude (e.g. `@namespace url(http://…)`): the
        // lexer consumed it as one opaque `<url-token>` (so an interior `/*`/`:` is
        // literal, not a comment / property-colon). Raw-extract it verbatim, trimming only
        // the whitespace just inside the parens — like the declaration path (shared
        // `url::trim_url_raw`) and prettier's `printer-postcss.js`. Only the lowercase
        // `url(` is canonicalized; `URL(  …  )` stays verbatim (postcss preserves it), so
        // trimming is gated on the lowercase spelling.
        if matches!(parser.current_kind, TokenKind::Url) {
            let raw = parser.current_value();
            if raw.starts_with("url(") {
                match trim_url_raw(raw) {
                    Some(trimmed) => prelude.push_str(&trimmed),
                    None => prelude.push_str(raw),
                }
            } else {
                prelude.push_str(raw);
            }
            trailing_spaces = 0;
            prev_token_kind = Some(TokenKind::Url);
            last_non_whitespace_kind = Some(TokenKind::Url);
            parser.advance()?;
            continue;
        }

        // Quoted `url("…")` stays ident + `(` + string (a function-token, not a url-token),
        // so it reaches here as an `Identifier`. Consume the balanced parens as one unit and
        // normalize the inner string quote (`url("x")` → `url('x')`) under `normalize_quotes`
        // (@namespace), matching prettier. Detect `url` on the raw source slice, not the
        // decoded identifier: `advance()` drops the decoded value when a token arrives via
        // the peek cache (which the whitespace branch above populates), so
        // `current_identifier()` is unreliable here — and a `url(` function requires the
        // literal `url` anyway. Only the lowercase spelling is canonicalized (as above).
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
            prelude.push_str(&part);
            trailing_spaces = 0;
            prev_token_kind = Some(TokenKind::RightParen);
            last_non_whitespace_kind = Some(TokenKind::RightParen);
            continue;
        }

        // Add space before boolean operators (and, or, not) or comments if not preceded by space
        // Note: @scope preludes are now parsed structurally, so they don't go through this code
        let is_bool_op = is_boolean_operator(parser);
        let is_comment = matches!(parser.current_kind, TokenKind::Comment);

        // Whitespace-rewriting (property/boolean/comma spacing) applies only to the
        // normalized `@media` path; verbatim raw at-rules keep the source spacing.
        if normalize_whitespace {
            // Check if we already have a trailing space (from programmatic insertion or whitespace token)
            let has_trailing_space = trailing_spaces > 0;

            // Add space before comments or boolean operators if not already preceded
            // by space — but not for a boolean operator right after `(`: `(not …)`
            // stays tight (matching prettier and the structured @supports/@container
            // path's same `not`-after-`(` suppression).
            let after_open_paren = last_non_whitespace_kind == Some(TokenKind::LeftParen);
            if (is_comment || (is_bool_op && !after_open_paren)) && !has_trailing_space {
                prelude.push(' ');
                trailing_spaces += 1;
            }

            // Remove trailing whitespace before ':' or ',' (CSS convention: no space before these).
            // Only the counted programmatic spaces are stripped, never a token's own bytes — so an
            // identifier ending in an escape-terminator space is left intact. (The counter is reset
            // by the token emission just below, which always runs next.)
            if matches!(parser.current_kind, TokenKind::Colon | TokenKind::Comma) {
                prelude.truncate(prelude.len() - trailing_spaces);
            }
        }

        // Emit the token verbatim from source — identifiers / numbers / comments keep their
        // raw slice so escapes survive (`@keyframes \@mymove` must not collapse to
        // `@keyframes @mymove`, `\31 23` must not collapse to `123`); only a string's
        // surrounding quotes are normalized under `normalize_quotes`.
        match &parser.current_kind {
            TokenKind::String { quote } => {
                let content = &parser.source()[parser.current_start + 1..parser.current_end - 1];
                if normalize_quotes {
                    prelude.push_str(&format_string_literal(content, *quote));
                } else {
                    prelude.push(*quote);
                    prelude.push_str(content);
                    prelude.push(*quote);
                }
            }
            _ => prelude.push_str(parser.current_value()),
        }
        trailing_spaces = 0;

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
                prelude.push(' ');
                trailing_spaces += 1;
            } else if is_comment {
                // Add space after comment, but not if followed by comma, close paren, or semicolon
                if !matches!(
                    parser.current_kind,
                    TokenKind::Comma | TokenKind::RightParen | TokenKind::Semicolon
                ) {
                    prelude.push(' ');
                    trailing_spaces += 1;
                }
            } else if matches!(current_kind, TokenKind::Comma) {
                // Add space after comma in media queries (comma acts as OR)
                prelude.push(' ');
                trailing_spaces += 1;
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
                    prelude.push(' ');
                    trailing_spaces += 1;
                }
            }
        }

        prev_token_kind = Some(current_kind);
        // Track last non-whitespace token for colon spacing logic
        if !matches!(current_kind, TokenKind::Whitespace) {
            last_non_whitespace_kind = Some(current_kind);
        }
    }

    let content = parser.alloc_str_in(prelude.trim());
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
