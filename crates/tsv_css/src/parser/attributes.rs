use super::CssParser;
use crate::ast::internal::*;
use crate::lexer::TokenKind;
use crate::whitespace::boundary_split_offset;
use tsv_lang::{ParseError, Span};

/// Parse attribute selector: `[attr]`, `[attr="value"]`, `[attr^="prefix"]`
/// Supports namespace prefixes: [ns|attr], [*|attr], [|attr]
///
/// Every juncture admits a `/* */` comment — the production is token-level and a comment
/// produces no token (css-syntax-3 §4) — so each gap registers what it skips for the
/// printer to re-emit. The two whitespace-forbidden regions are skipped **comments-only**
/// (`register_and_skip_comments`) so a `<whitespace-token>` there still rejects: the
/// `<wq-name>`'s `|` juncture, and the `<attr-matcher>`'s (`parse_attribute_matcher`).
///
/// ⚠️ The gap *before* the `|` is the one position where whitespace-forbidden is not a
/// property of the gap alone: `[svg |attr]` is a `<wq-name>` (forbidden) while
/// `[attr |= 'v']` is a name→`<attr-matcher>` gap (allowed — the brackets bound it), and
/// the two are told apart only by the token after the `|`. So whitespace is skipped there
/// and **reported**, and the `<wq-name>` reading rejects retroactively.
pub(crate) fn parse_attribute_selector<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    start: usize,
) -> Result<SimpleSelector<'arena>, ParseError> {
    parser.expect(TokenKind::LeftBracket)?;
    // `[` is its own `allow_whitespace()` juncture (`read_selector`'s attribute arm skips
    // before the name), so the run belongs to neither the bracket nor the name. Looped with
    // the comment arm, since a run may sit on either side of a comment tsv admits here and
    // `parseCss` does not.
    parser.skip_boundary_whitespace_registering_comments()?;

    // Parse namespace prefix (optional):
    // - [ns|attr] - namespace prefix "ns"
    // - [*|attr] - universal namespace "*"
    // - [|attr] - explicit no namespace ""
    // - [attr] - implicit no namespace (None)
    let namespace_span = if parser.check(TokenKind::Asterisk) {
        // Universal namespace: *|attr — the `*` can only be a wq-name prefix here, so a
        // comment before the `|` is unambiguous.
        let prefix = Span {
            start: parser.span_pos(parser.current_start),
            end: parser.span_pos(parser.current_end),
        };
        parser.advance()?;
        parser.register_and_skip_comments()?;
        if !parser.check(TokenKind::Pipe) {
            return Err(parser.error_expected_after("'|'", "'*' in attribute selector"));
        }
        parser.advance()?; // consume |
        parser.register_and_skip_comments()?;
        Some(prefix)
    } else if parser.check(TokenKind::Pipe) {
        // Explicit no namespace: |attr — the prefix's leading token is absent, so its
        // span is empty at the `|`.
        let prefix = Span {
            start: parser.span_pos(parser.current_start),
            end: parser.span_pos(parser.current_start),
        };
        parser.advance()?; // consume |
        parser.register_and_skip_comments()?;
        Some(prefix)
    } else if parser.check(TokenKind::Identifier) {
        // Could be: ns|attr or just attr (or lang with |= operator). Capture the identifier's
        // span before consuming it: it is the verbatim span of either reading — the attribute
        // *name*, or the `<ns-prefix>`'s leading token.
        let maybe_namespace_span = Span {
            start: parser.span_pos(parser.current_start),
            end: parser.span_pos(parser.current_end),
        };
        parser.advance()?;
        // Whitespace here is legal only under the `|=` reading — see the ⚠️ above. The
        // boundary skip, because `read_selector`'s `allow_whitespace()` after the name is one
        // run however its members are spelled: a run GLUED to the name was swallowed by
        // `read_identifier` on both sides and never reaches here, but `[a <NBSP>]` puts it in
        // a token of its own, where the plain skip left it standing and it read as a matcher.
        let space_before_pipe = parser.skip_boundary_whitespace_registering_comments()?;

        // Check if this is a namespace prefix (identifier|identifier)
        // vs dash-match operator (identifier|=value)
        if parser.check(TokenKind::Pipe) {
            // Peek ahead to distinguish namespace from |= operator. Either reading admits
            // a comment past the `|` — the wq-name's second juncture, or the
            // `<attr-matcher>`'s — and both are whitespace-forbidden, so only comments are
            // skipped here.
            parser.advance()?; // consume |
            parser.register_and_skip_comments()?;

            // If next token is =, this was the |= operator, not a namespace
            if parser.check(TokenKind::Equals) {
                // Backtrack: restore the identifier as attribute name
                // and let matcher parsing handle |=
                // We need to restore state...
                // Actually, we can't easily backtrack here. Let me restructure.

                // Since we've already consumed the |, we need to handle |= here
                // and return early. The identifier before `|=` is the attribute name —
                // use its captured span (the namespace reading was a false start).
                parser.advance()?; // consume =
                // `read_selector`'s `allow_whitespace()` before `read_attribute_value`.
                parser.skip_boundary_whitespace_registering_comments()?;

                // Parse value (identifier or string)
                let value_span = parse_attribute_value(parser)?;

                let flags = parse_attribute_flags(parser)?;

                // Expect ] and capture its end position
                let end = parser.expect_and_capture(TokenKind::RightBracket)?;

                return Ok(SimpleSelector::Attribute {
                    namespace_span: None,
                    name_span: maybe_namespace_span,
                    matcher: Some(AttributeMatcher::DashMatch),
                    value_span,
                    flags,
                    span: Span {
                        start: start as u32,
                        end,
                    },
                });
            }

            // Otherwise it's a namespace prefix: ns|attr — and under *that* reading the
            // gap before the `|` sat inside a `<wq-name>`, where a space is forbidden.
            if space_before_pipe {
                return Err(parser.error_msg(
                    "No whitespace allowed between the components of a namespace-qualified name",
                ));
            }
            Some(maybe_namespace_span)
        } else {
            // It's the attribute name (no namespace) — use its captured span (the name is
            // recovered from source).

            let (matcher, value_span) = parse_attribute_matcher_and_value(parser)?;
            let flags = parse_attribute_flags(parser)?;

            // Expect ] and capture its end position
            let end = parser.expect_and_capture(TokenKind::RightBracket)?;

            return Ok(SimpleSelector::Attribute {
                namespace_span: None, // No namespace prefix (implicit)
                name_span: maybe_namespace_span,
                matcher,
                value_span,
                flags,
                span: Span {
                    start: start as u32,
                    end,
                },
            });
        }
    } else {
        return Err(parser.error_expected("attribute name, '*', or '|'"));
    };

    // Now parse the attribute name (after namespace|)
    if !parser.check(TokenKind::Identifier) {
        return Err(parser.error_expected_after("attribute name", "namespace"));
    }

    // Name recovered verbatim from its span (printer emits raw; convert half-decodes — see
    // ast/convert/mod.rs). The `Identifier` check above guarantees a name token, so no decoded
    // copy is stored.
    let name_span = Span {
        start: parser.span_pos(parser.current_start),
        end: parser.span_pos(parser.current_end),
    };
    parser.advance()?;
    // `read_selector`'s `allow_whitespace()` after the name — see the no-namespace twin above.
    parser.skip_boundary_whitespace_registering_comments()?;

    let (matcher, value_span) = parse_attribute_matcher_and_value(parser)?;
    let flags = parse_attribute_flags(parser)?;

    // Expect ] and capture its end position
    let end = parser.expect_and_capture(TokenKind::RightBracket)?;

    Ok(SimpleSelector::Attribute {
        namespace_span,
        name_span,
        matcher,
        value_span,
        flags,
        span: Span {
            start: start as u32,
            end,
        },
    })
}

/// Parse the optional matcher + value that may follow an attribute name: a
/// closing `]` immediately yields `(None, None)` (bare `[attr]`); otherwise a
/// matcher (`=`, `~=`, `|=`, `^=`, `$=`, `*=`) and its value.
fn parse_attribute_matcher_and_value(
    parser: &mut CssParser<'_, '_>,
) -> Result<(Option<AttributeMatcher>, Option<Span>), ParseError> {
    if parser.check(TokenKind::RightBracket) {
        Ok((None, None)) // Just [attr]
    } else {
        let matcher = parse_attribute_matcher(parser)?;
        // `read_selector`'s `allow_whitespace()` between `<attr-matcher>` and value — and a
        // boundary one, since a run here would otherwise open the value token and ride into
        // `value` (`[a=<NBSP>b]` → `"\u{a0}b"`, where `read_attribute_value` trims it off).
        parser.skip_boundary_whitespace_registering_comments()?;
        let value = parse_attribute_value(parser)?;
        Ok((Some(matcher), value))
    }
}

/// Parse an optional attribute case flag (`i`/`I` = case-insensitive, `s`/`S` =
/// case-sensitive), preserving the original case. A trailing identifier that
/// isn't a flag is left unconsumed. Advances past the flag + trailing
/// whitespace when present.
///
/// The flag is the ASCII-letter prefix of the token, which is what `REGEX_ATTRIBUTE_FLAGS`
/// (`/[a-zA-Z]+/y`) reads: a boundary run glued behind it (`[a=b i<NBSP>]`) is the
/// flags→`]` `allow_whitespace()` to `parseCss`, where the lexer read one identifier
/// `i<NBSP>`. The reader ends the flag at the letters and re-seats past them, so the skip
/// below steps the run exactly as it steps a run the lexer tokenized on its own. Read from
/// the RAW token for the same reason: an escaped spelling (`\69`) is no flag to that regex.
fn parse_attribute_flags<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<Option<&'arena str>, ParseError> {
    if !parser.check(TokenKind::Identifier) {
        return Ok(None);
    }
    let (start, end) = (parser.current_start, parser.current_end);
    let raw = &parser.source()[start..end];
    let letters = raw.bytes().take_while(u8::is_ascii_alphabetic).count();
    let flag = &raw[..letters];
    // ASCII-cased by the grammar (`i`/`I`/`s`/`S`), so no `to_lowercase` allocation — and
    // no Unicode folding to widen what counts as a flag.
    if !(flag.eq_ignore_ascii_case("i") || flag.eq_ignore_ascii_case("s")) {
        return Ok(None);
    }
    let flag = parser.alloc_str_in(flag);
    parser.advance_from(start + letters)?;
    // `read_selector`'s `allow_whitespace()` between the flags and `]`.
    parser.skip_boundary_whitespace_registering_comments()?;
    Ok(Some(flag)) // Preserve original case
}

/// Parse an attribute selector's value (identifier or string), recording the raw
/// token span (a string keeps its quotes, an identifier its escapes — the wire
/// derives its quote-stripped text via `attribute_value_text`, matching Svelte's
/// undecoded `value`). Advances past the value token and trailing whitespace.
///
/// A bare value ends where `read_attribute_value` ends it — at the first JS-`\s` code point
/// (`REGEX_CLOSING_BRACKET`), which the lexer read as identifier content: `[a=b<NBSP>]` is the
/// value `b` and `[a=b<NBSP>i]` the value `b` with the flag `i` behind the run. The reader
/// ends the span at the split (`boundary_split_offset`) and re-seats there, so the skip below
/// steps the run as it steps one the lexer tokenized on its own; the head of the token was
/// stepped by the matcher→value juncture before it was read.
fn parse_attribute_value(parser: &mut CssParser<'_, '_>) -> Result<Option<Span>, ParseError> {
    let (start, mut end) = (parser.current_start, parser.current_end);
    match &parser.current_kind {
        TokenKind::Identifier => {
            if let Some(split) = boundary_split_offset(&parser.source()[start..end]) {
                end = start + split;
            }
        }
        TokenKind::String { .. } => {}
        _ => {
            return Err(parser.error_expected("attribute value"));
        }
    }
    let span = Span {
        start: parser.span_pos(start),
        end: parser.span_pos(end),
    };
    parser.advance_from(end)?;
    // `read_selector`'s `allow_whitespace()` between the value and the flags — the run is a
    // separator on both sides of `parseCss` (`read_attribute_value` also stops on it, since
    // its `REGEX_CLOSING_BRACKET` is JS `\s`).
    parser.skip_boundary_whitespace_registering_comments()?;
    Ok(Some(span))
}

/// Parse attribute matcher: =, ~=, |=, ^=, $=, *=
///
/// The two components of a two-character matcher are separate `<delim-token>`s (css-syntax-3
/// gives none of `~ | ^ $ =` a case in *consume a token*), so a comment may sit between them
/// — but a `<whitespace-token>` may not, which is why only comments are skipped.
fn parse_attribute_matcher(parser: &mut CssParser<'_, '_>) -> Result<AttributeMatcher, ParseError> {
    let matcher = match &parser.current_kind {
        TokenKind::Equals => AttributeMatcher::Exact, // =
        TokenKind::Tilde => {
            // ~= (contains in whitespace-separated list)
            parser.advance()?;
            parser.register_and_skip_comments()?;
            if !parser.check(TokenKind::Equals) {
                return Err(parser.error_expected_after("'='", "'~'"));
            }
            parser.advance()?; // consume =
            return Ok(AttributeMatcher::Contains);
        }
        TokenKind::Pipe => {
            // |= (dash-match)
            parser.advance()?;
            parser.register_and_skip_comments()?;
            if !parser.check(TokenKind::Equals) {
                return Err(parser.error_expected_after("'='", "'|'"));
            }
            parser.advance()?; // consume =
            return Ok(AttributeMatcher::DashMatch);
        }
        TokenKind::Caret => {
            // ^= (prefix match)
            parser.advance()?;
            parser.register_and_skip_comments()?;
            if !parser.check(TokenKind::Equals) {
                return Err(parser.error_expected_after("'='", "'^'"));
            }
            parser.advance()?; // consume =
            return Ok(AttributeMatcher::Prefix);
        }
        TokenKind::Dollar => {
            // $= (suffix match)
            parser.advance()?;
            parser.register_and_skip_comments()?;
            if !parser.check(TokenKind::Equals) {
                return Err(parser.error_expected_after("'='", "'$'"));
            }
            parser.advance()?; // consume =
            return Ok(AttributeMatcher::Suffix);
        }
        TokenKind::Asterisk => {
            // *= (substring match)
            parser.advance()?;
            parser.register_and_skip_comments()?;
            if !parser.check(TokenKind::Equals) {
                return Err(parser.error_expected_after("'='", "'*'"));
            }
            parser.advance()?; // consume =
            return Ok(AttributeMatcher::Substring);
        }
        _ => {
            return Err(parser.error_msg(&format!(
                "Unsupported attribute matcher: {:?}",
                parser.current_kind
            )));
        }
    };

    parser.advance()?; // consume = for Exact match
    Ok(matcher)
}
