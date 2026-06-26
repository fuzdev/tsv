use super::{CssParser, is_boolean_operator};
use crate::lexer::TokenKind;
use crate::url::trim_url_raw;
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
) -> Result<(&'arena str, Span), ParseError> {
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
                trim_url_raw(raw).unwrap_or_else(|| raw.to_string())
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

    let joined = prelude_parts.join("");
    let content = parser.alloc_str_in(joined.trim());
    let prelude_end = parser.base_offset() + parser.current_start;
    let span = Span {
        start: prelude_start as u32,
        end: prelude_end as u32,
    };

    Ok((content, span))
}
