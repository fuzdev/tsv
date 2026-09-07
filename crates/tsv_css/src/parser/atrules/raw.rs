use std::borrow::Cow;

use super::{CssParser, is_boolean_operator};
use crate::lexer::TokenKind;
use crate::url::trim_url_raw;
use tsv_lang::printing::format_string_literal;
use tsv_lang::{ParseError, Span};

/// Build the raw at-rule prelude string (used for both the printer and, for the media
/// reader, the formatter's wrapping). `normalize_whitespace = true` (`@media` and
/// `@custom-media`, the two names prettier hands to `parseMediaQuery`) applies the media
/// reader's node split — `property: value` / boolean-operator spacing, a name's runs
/// collapsed, a value's run kept; `false` (every other raw at-rule — `@layer`,
/// `@namespace`, `@keyframes`, …) preserves internal whitespace verbatim, matching
/// prettier and Svelte. `url()` inner whitespace is trimmed in both modes (a
/// spec-mandated `<url-token>` normalization).
///
/// The flag is printer-facing only: the prelude *span* is the same either way and
/// `convert` emits the wire from the span, so routing a name between the two modes moves
/// no wire byte.
pub(super) fn parse_raw_prelude_content<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    normalize_whitespace: bool,
    // `@namespace` normalizes string / `url()` quotes to prettier's single-quote form
    // (`"x"` → `'x'`, `url("x")` → `url('x')`, per the `singleQuote` option — the same
    // as declaration values and `@import`); other raw at-rules keep quotes verbatim.
    normalize_quotes: bool,
) -> Result<(&'arena str, Span), ParseError> {
    // Add spaces around boolean operators (and, or, not) and after ':' for prettier compatibility
    let prelude_start = parser.span_pos(parser.current_start);
    // One growable buffer rather than a `Vec<String>` of per-token / per-space pieces
    // joined at the end (the `parse_declaration` raw-buffer idiom, extended to the at-rule
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

    // The media reader's NODE SPLIT. `parseMediaQuery` cuts the prelude into nodes
    // (media types, keywords, feature expressions) and joins them with one space;
    // `parseMediaFeature` then cuts a feature expression's interior at its **first**
    // colon — parens do not protect it — into a `media-feature` name and a
    // `media-value`. The two halves are printed by different rules, and whitespace is
    // where they part: the name collapses its runs, the value is emitted **verbatim**
    // (`(a: x  ,  9px)` keeps both runs), and only the expression's OWN parens trim —
    // whitespace beside an inner paren is node text and survives, which is what makes
    // `(not ( screen and ( color ) ))` a fixed point on both formatters.
    //
    // The split's state — this depth and the two flags below — is read only on the
    // normalize path (the media reader); the verbatim raw branch never consults it.
    let mut paren_depth: u32 = 0;
    // Whether this feature expression's splitting colon has been passed — i.e. whether
    // the tokens now arriving are value position. Reset at each expression's close,
    // since every expression has its own name/value split.
    let mut feature_colon_seen = false;
    // Whether the token just emitted was that splitting colon. The value's own leading
    // whitespace is not part of it (`parseMediaFeature` puts it in the node's `before`
    // and prints `l.trim()`), so this one run is the `media-colon`'s single space rather
    // than verbatim value text — `(a:   9px  )` is `(a: 9px)`, not `(a:   9px)`.
    let mut after_splitting_colon = false;
    // Whether the buffer ends in a verbatim value run — whitespace that is `media-value`
    // CONTENT rather than a separator. It is the second half of the question
    // `trailing_spaces` answers: that counter deliberately holds only the *programmatic*
    // spaces, so the `:`/`,` truncation can never eat a token's own bytes, which leaves
    // it at zero after a verbatim run. Every rule asking "am I already separated?" must
    // read both, or it pads a run that is already whitespace and the padding compounds on
    // the next pass (`(a: b /* c */)` growing a space per format — an F1 break, and a
    // fixed point on every gate but a prettier `compare`).
    let mut ends_in_verbatim_run = false;

    // Categorize at-rule by prelude type based on CSS specs:
    // - Selector list preludes (@scope): Format like CSS selectors (.widget:hover)
    // - Query preludes (@media, @container, @supports): Format like properties (min-width: 500px)
    // - No prelude (@font-face, @starting-style): No prelude to normalize
    // - Identifier preludes (@keyframes, @layer): No colons to worry about

    while !parser.at_prelude_end() {
        if parser.check(TokenKind::Whitespace) {
            // Verbatim mode (every raw at-rule off the media reader): preserve the source
            // whitespace exactly — prettier and Svelte keep it (`@layer a  ,  b` stays
            // `a  ,  b`).
            if !normalize_whitespace {
                // Verbatim mode never reads `trailing_spaces` (the collapse logic below is
                // normalize-only), so the raw whitespace goes straight in.
                prelude.push_str(parser.current_value());
                parser.advance()?;
                prev_token_kind = Some(TokenKind::Whitespace);
                continue;
            }
            // Skip a whitespace run right inside the feature expression's OWN parens —
            // its two halves are `.trim()`ed by `parseMediaFeature`. Only depth 1 does
            // this: a run beside an INNER paren is part of the node's text and prettier
            // keeps it, so dropping it there is what collapsed `(not ( screen and (
            // color ) ))`. A run right after the SPLITTING colon (`(min-width: )`, empty
            // value) is the prettier-mandated single space after `:` — keep it before
            // `)` rather than dropping it, or `(a: )` would collapse to `(a:)` while
            // `(a:)` gains the space (the colon-space rule below), an F1 oscillation.
            // Only that colon: a later one is value text and holds nothing open, so
            // `(a: b:   )` trims to `(a: b:)` like any other run against that paren.
            let at_expression_open =
                matches!(prev_token_kind, Some(TokenKind::LeftParen)) && paren_depth == 1;
            let at_expression_close = matches!(parser.peek_kind(), Ok(TokenKind::RightParen))
                && paren_depth == 1
                && !after_splitting_colon;
            let skip_whitespace = at_expression_open || at_expression_close;

            // A `media-value`'s run is the node's own text, which prettier emits with no
            // collapse at all. One exception, tsv's: a run carrying a newline still
            // collapses, because tsv never emits a raw newline inside a prelude — it
            // would defeat the width accounting and the `and`/`or` wrap that re-decides
            // the prelude's own line breaks. A cataloged divergence
            // (conformance_prettier_css.md §CSS: At-Rules, `@media value newline`).
            let raw_run = parser.current_value();
            let verbatim_run = paren_depth >= 1
                && feature_colon_seen
                && !after_splitting_colon
                && !raw_run.contains(['\n', '\r']);
            let raw_run: Option<String> = verbatim_run.then(|| raw_run.to_owned());

            parser.advance()?;

            if skip_whitespace {
                continue;
            }
            match raw_run {
                // Verbatim: not a programmatic separator, so it is not collapsible and
                // the `:`/`,` truncation below must not eat it — which is why it is
                // announced on `ends_in_verbatim_run` instead of counted.
                Some(run) => {
                    prelude.push_str(&run);
                    trailing_spaces = 0;
                    ends_in_verbatim_run = true;
                }
                // The separator run, `after_splitting_colon`'s included — so that flag is
                // spent here, and only the ONE run against the colon is ever the
                // `media-colon`'s.
                None => {
                    prelude.push(' ');
                    trailing_spaces += 1;
                    ends_in_verbatim_run = false;
                    after_splitting_colon = false;
                }
            }
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
            ends_in_verbatim_run = false;
            after_splitting_colon = false;
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
            let part: Cow<'_, str> = if is_lowercase_url {
                let trimmed = trim_url_raw(raw).unwrap_or(Cow::Borrowed(raw));
                if normalize_quotes {
                    Cow::Owned(normalize_url_string_quote(&trimmed))
                } else {
                    trimmed
                }
            } else {
                Cow::Borrowed(raw)
            };
            prelude.push_str(&part);
            trailing_spaces = 0;
            ends_in_verbatim_run = false;
            after_splitting_colon = false;
            prev_token_kind = Some(TokenKind::RightParen);
            last_non_whitespace_kind = Some(TokenKind::RightParen);
            continue;
        }

        // Add space before boolean operators (and, or, not) or comments if not preceded by space
        // Note: @scope preludes are parsed structurally, so they don't go through this code
        let is_bool_op = is_boolean_operator(parser);
        let is_comment = matches!(parser.current_kind, TokenKind::Comment);
        // Add space before comments or boolean operators if not already preceded
        // by space — but not for a boolean operator right after `(`: `(not …)`
        // stays tight (matching prettier and the structured @supports/@container
        // path's same `not`-after-`(` suppression).
        // A boolean operator INSIDE a feature expression is part of the node's own
        // text, which prettier neither spaces nor collapses (`(a and(b))` is its own
        // fixed point), so only a query-level one is padded. Read on both sides of the
        // token, so the pair can never leave a one-sided space.
        let pads_boolean_operator = is_bool_op
            && last_non_whitespace_kind != Some(TokenKind::LeftParen)
            && paren_depth == 0;
        // A comment INSIDE a feature expression is that node's own text for the same
        // reason, and prettier inserts nothing there: the name half only collapses its
        // whitespace runs and the value half keeps them, so a comment the author glued to
        // the expression's paren, to a name or to a value stays glued
        // (`(/* c */a: b)`, `(a: b/* c */)` are prettier fixed points). Read on both sides
        // of the token like the operator's, so the pair can never leave a one-sided space.
        // ⚠️ Depth 0 is NOT this rule, and tsv models no node split there at all: prettier
        // joins query-level nodes with one space and returns a `media-unknown` verbatim, so
        // `@media a/* c */b` is one node it keeps whole where tsv pads. An open gap,
        // cataloged in conformance_prettier_css.md §CSS: At-Rules.
        let pads_comment = is_comment && paren_depth == 0;

        // Whitespace-rewriting (property/boolean/comma spacing) applies only to the
        // normalized media-reader path; verbatim raw at-rules keep the source spacing.
        if normalize_whitespace {
            // Check if we already have a trailing space (from programmatic insertion or whitespace token)
            // Both halves of "the buffer already ends in whitespace" — see
            // `ends_in_verbatim_run`, which a value run leaves set and `trailing_spaces`
            // cannot show.
            let has_trailing_space = trailing_spaces > 0 || ends_in_verbatim_run;

            if (pads_comment || pads_boolean_operator) && !has_trailing_space {
                prelude.push(' ');
                trailing_spaces += 1;
            }

            // Remove trailing whitespace before ':' or ',' (CSS convention: no space before these).
            // Only the counted programmatic spaces are stripped, never a token's own bytes — so an
            // identifier ending in an escape-terminator space is left intact. (The counter is reset
            // by the token emission just below, which always runs next.)
            //
            // Inside a feature expression only the SPLITTING colon strips: it is where
            // `parseMediaFeature` trims the name. A later colon and every comma there are
            // value text (`(a: b,c)`, `(a , b)` are both prettier fixed points).
            let strips_leading_space = match parser.current_kind {
                TokenKind::Colon => paren_depth == 0 || !feature_colon_seen,
                TokenKind::Comma => paren_depth == 0,
                _ => false,
            };
            if strips_leading_space {
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
        ends_in_verbatim_run = false;

        let current_kind = parser.current_kind;

        parser.advance()?;

        // Add space after boolean operators, comments, commas, or ':' if not followed by whitespace
        // Note: @scope preludes are parsed structurally, so they don't go through this code
        if normalize_whitespace && !parser.check(TokenKind::Whitespace) {
            if pads_boolean_operator {
                prelude.push(' ');
                trailing_spaces += 1;
            } else if pads_comment {
                // Add space after comment, but not if followed by comma, close paren, or semicolon
                if !matches!(
                    parser.current_kind,
                    TokenKind::Comma | TokenKind::RightParen | TokenKind::Semicolon
                ) {
                    prelude.push(' ');
                    trailing_spaces += 1;
                }
            } else if matches!(current_kind, TokenKind::Comma) && paren_depth == 0 {
                // Add space after a query-list comma (comma acts as OR). A comma inside a
                // feature expression is that node's own text — `strips_leading_space`
                // leaves its left side alone for the same reason.
                prelude.push(' ');
                trailing_spaces += 1;
            } else if matches!(current_kind, TokenKind::Colon)
                && (paren_depth == 0 || !feature_colon_seen)
            {
                // Add space after ':' for property:value pairs in query preludes (@media,
                // @supports, @container) — preceded by identifier/number/dimension. Uses
                // last_non_whitespace_kind (handles whitespace removed before the colon).
                // Only the expression's SPLITTING colon gets it; a later one is value text
                // (`media-colon` is a node, and there is exactly one per expression).
                let should_add_space = matches!(
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

        // Advance the node-split state LAST, so every rule above reads the position the
        // token it is deciding about sits at: a `(` is decided at its enclosing depth, a
        // `)` and the splitting `:` at the depth inside the expression they belong to.
        // ⚠️ The two `url(` arms above `continue` past here and so must carry their own
        // state, which is why each spells it: the DEPTH needs nothing (both consume their
        // own balanced parens, so what they would add and remove cancels), but
        // `after_splitting_colon` and `ends_in_verbatim_run` are about the token just
        // emitted and go stale — a url left the first set, and the run after it then read
        // as the `media-colon`'s separator and collapsed (`(a: url(x)  y)`).
        // Set by the splitting colon itself — the FIRST at depth, the only one that is a
        // `media-colon` node — and spent by whatever comes next: a whitespace run clears
        // it in the branch above (which never reaches here), every other token clears it
        // here, so only the ONE run against the colon is the separator and a second run
        // further along is value text again.
        after_splitting_colon =
            matches!(current_kind, TokenKind::Colon) && paren_depth >= 1 && !feature_colon_seen;
        match current_kind {
            TokenKind::LeftParen => paren_depth += 1,
            TokenKind::RightParen => {
                paren_depth = paren_depth.saturating_sub(1);
                // Each feature expression carries its own name/value split.
                if paren_depth == 0 {
                    feature_colon_seen = false;
                }
            }
            TokenKind::Colon if paren_depth >= 1 => feature_colon_seen = true,
            _ => {}
        }
    }

    // Escape-aware: `@layer a\ ;` ends in an escape whose payload IS that space (§4.3.4 /
    // §4.3.7). Trimming it strands the backslash onto the `;` (or the block's `{`), which
    // it then escapes — output that no longer parses. The CSS-only trim also leaves an
    // NBSP alone: it is prelude content, not padding.
    let content = parser.alloc_str_in(crate::escapes::trim_end_preserving_escape(
        crate::escapes::trim_start_css(&prelude),
    ));
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
