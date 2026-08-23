// Conversion from the internal AST to the public wire JSON.
//
// ARCHITECTURE: clean model inside, Svelte's scan semantics at the boundary.
//
// The internal AST is the spec-faithful semantic representation (decoded
// strings/escapes, structured values, normalized once during parsing) and is
// what the FORMATTER derives from. The public JSON strings, by contrast, are
// deliberately reconstructed from RAW SOURCE, because Svelte's parseCss builds
// them by raw text scanning and tsv's public AST is a drop-in for it:
// - Declaration `property`/`value` — raw split at the colon, block comments
//   stripped, ends trimmed (`read_declaration`/`read_value` semantics; the
//   structured internal value is never re-serialized into the JSON)
// - Declaration `end` — the `;`/`}` terminator scan position
// - Selector names — half-decoded like `read_identifier` (hex escapes decode,
//   identity escapes keep the backslash)
// Spans always index the real file; Svelte's `remove_bom` shift is a
// documented divergence (docs/conformance_svelte.md), not replicated.
//
// The writer (`write.rs`) emits the wire JSON directly from the internal AST
// in one walk and **reuses the raw-source reconstruction helpers below**
// (`strip_css_comments_collecting`, `split_declaration_svelte_compat`,
// `raw_selector_name`, …), so the Svelte scan semantics live in one place. It
// is the sole emission path; `convert_ast_json_bytes` calls it and
// `convert_ast_json` parses its bytes back into a `Value`. That one walk also
// GATHERS the wire's flat `CSSComment[]`: a declaration's comments exist only
// in the strip its `value` comes from, so collecting them there rather than in
// a second pass is what keeps their offsets indexing the string they describe.

use super::internal;
use std::borrow::Cow;
use tsv_lang::Span;
use tsv_lang::is_js_whitespace;

mod write;
pub(crate) use write::write_stylesheet_file_bytes;
pub use write::{CssComments, write_css_children, write_css_comments};

/// Trim a wire STRING the way `parseCss` does.
///
/// ⚠️ [`is_js_whitespace`], **not** `str::trim`: every trim on this path mirrors a JS
/// `.trim()` / `.trimStart()` in Svelte's `read_value` / `read_attribute_value`
/// (`1-parse/read/style.js`), which is the JS `\s` class. Rust's `White_Space` **deletes a
/// U+0085** `parseCss` keeps and **keeps a U+FEFF** it deletes — `color: red<NEL>` came back
/// through the wire as `red`, a character of the author's declaration gone, and the printer
/// then emitted the shortened value.
///
/// ⚠️ Not `is_css_whitespace` either. That one is ASCII-only (css-syntax-3 §4.2) and is the
/// right class for value *separation* and value-text *collapsing* — a different question one
/// layer down. Here the oracle is a JS `.trim()` and nothing else.
///
/// The three spellings exist so a call site names which end it trims; they are one rule, and
/// [`strip_css_comments_inner`]'s trim and the no-comment fast paths that stand in for it
/// (`write_declaration`, [`split_declaration_svelte_compat`]) must all ask the same one — a
/// fast path on a different class is the shortcut silently disagreeing with what it shortcuts.
#[inline]
fn trim_wire(s: &str) -> &str {
    s.trim_matches(is_js_whitespace)
}

/// The leading half of [`trim_wire`], same class and same reason.
#[inline]
pub(super) fn trim_wire_start(s: &str) -> &str {
    s.trim_start_matches(is_js_whitespace)
}

/// The trailing half of [`trim_wire`], same class and same reason.
#[inline]
pub(super) fn trim_wire_end(s: &str) -> &str {
    s.trim_end_matches(is_js_whitespace)
}

/// Split a declaration source into property and value, matching Svelte's quirky behavior.
///
/// SVELTE QUIRK: When there's a CSS comment between the property name and the colon,
/// Svelte puts the comment AND the colon into the value instead of the property.
///
/// Example: `color /* comment */ : red`
/// - Normal split: property=`color /* comment */ `, value=`red`
/// - Svelte quirk: property=`color`, value=`/* comment */ : red`
///
/// This is a tokenization bug in Svelte's CSS parser, but we replicate it for compatibility.
/// Our internal AST remains semantically correct; this quirk is only applied in conversion.
///
/// Note: the writer runs `strip_css_comments` on the returned value, so the
/// public AST for `color /* c */ : red` ends up as property=`color`, value=`": red"`
/// (Svelte 5.55+ strips block comments from value strings post-split).
///
/// `colon_pos` is the declaration-relative byte offset of the real
/// `property : value` colon, recorded at parse time (`CssDeclaration::colon_offset`
/// minus the declaration start) — the writer only calls this on the comment-bearing
/// path, so the no-comment common case never re-derives it. A property comment may
/// itself contain a `:` (`color /* x:y */: red`), which is why the parser's colon is
/// the authority rather than a naive `find(':')`.
pub(super) fn split_declaration_svelte_compat(decl_source: &str, colon_pos: usize) -> (&str, &str) {
    let before_colon = &decl_source[..colon_pos];

    // Look for /* that appears after some property text
    if let Some(comment_idx) = before_colon.find("/*") {
        // Only apply quirk if there's actual property content before the comment
        let before_comment = &before_colon[..comment_idx];
        if !trim_wire(before_comment).is_empty() {
            // SVELTE QUIRK: Comment between property and colon
            // Property = just the text before the comment (trimmed)
            // Value = comment + colon + actual value (everything from comment onward)
            let property = trim_wire(before_comment);
            let value = &decl_source[comment_idx..];
            return (property, value);
        }
    }

    // Normal case: split at colon
    let property = &decl_source[..colon_pos];
    let value = trim_wire_start(&decl_source[colon_pos + 1..]);
    (property, value)
}

/// Remove all `/* ... */` block comments from a CSS string, then trim outer whitespace.
///
/// Matches Svelte 5.55+ behavior for Declaration `value` and Atrule `prelude` strings:
/// comments are stripped in place (surrounding whitespace preserved), then the result
/// is trimmed.
///
/// ⚠️ The trim is [`is_js_whitespace`], because the oracle's is `value.trim()` /
/// `value.trimStart()` in `read_value` (`1-parse/read/style.js`) — JS `\s`. Rust's
/// `str::trim` is `White_Space`, which **deletes a U+0085** `parseCss` keeps: `color:
/// red<NEL>` came back through the wire as `red`, a character of the author's declaration
/// gone, and the printer then emitted the shortened value. Not `is_css_whitespace`
/// either — that one is ASCII-only and is the right class for value *separation*, a
/// different question one layer down; here the oracle is a JS `.trim()` and nothing else.
///
/// String- and url()-aware: `/*` sequences inside `"..."`, `'...'`, or `url(...)` are
/// treated as content, not comments. Unterminated comments are left intact (parse
/// error caught elsewhere).
fn strip_css_comments(input: &str) -> Cow<'_, str> {
    strip_css_comments_inner(input, 0, None)
}

/// `strip_css_comments`, additionally recording every comment it strips as a wire
/// `CSSComment` on `sink` — the shape Svelte's `read_value` produces for a
/// declaration value or an at-rule prelude, where a captured comment carries both
/// its source span and a `position` offset into the emitted string.
///
/// `base` is `input`'s start offset in the document, so the recorded spans are in
/// document coordinates. Comments arrive in source order.
pub(super) fn strip_css_comments_collecting<'a>(
    input: &'a str,
    base: u32,
    sink: &mut Vec<WireComment>,
) -> Cow<'a, str> {
    strip_css_comments_inner(input, base, Some(sink))
}

/// One emitted `CSSComment`. `position` is `Some` only for a comment lifted out of
/// a declaration `value` / at-rule `prelude` string — Svelte sets the field there
/// and nowhere else, so a structural comment (between rules, in a block, in a
/// selector gap) carries the span alone.
#[derive(Clone, Copy)]
pub(super) struct WireComment {
    pub(super) span: Span,
    pub(super) position: Option<u32>,
}

fn strip_css_comments_inner<'a>(
    input: &'a str,
    base: u32,
    sink: Option<&mut Vec<WireComment>>,
) -> Cow<'a, str> {
    // Fast path: no block-comment delimiter anywhere means nothing is stripped, so
    // the result is just the trimmed input — a borrowed sub-slice, no allocation,
    // and nothing to record. (Conservative: a `/*` inside a string/url is preserved
    // either way, so those rare inputs fall to the owned path; correctness is
    // unaffected — the owned path records only what it actually strips.)
    if !input.contains("/*") {
        return Cow::Borrowed(trim_wire(input));
    }
    let mut out = String::with_capacity(input.len());
    // `(byte offset in `out`, source span)` per stripped comment. The offset is the
    // UNTRIMMED accumulated length, matching Svelte's `value.length` read at the
    // moment it consumes the comment; it becomes a `position` once the leading
    // whitespace is known (below).
    let mut stripped: Vec<(usize, Span)> = Vec::new();
    let mut rest = input;
    while let Some(ch) = rest.chars().next() {
        // Block comment — strip
        if crate::comments::is_comment_start(rest.as_bytes(), 0) {
            // `comment_end_checked`, not `comment_end`: an unterminated comment is kept
            // verbatim here rather than swallowed to end-of-input. Svelte's `read_comment`
            // errors on one instead of capturing it, so it goes unrecorded either way.
            let Some(end) = crate::comments::comment_end_checked(rest.as_bytes(), 0) else {
                out.push_str(rest);
                break;
            };
            if sink.is_some() {
                let start = base as usize + (input.len() - rest.len());
                stripped.push((out.len(), Span::new(start as u32, (start + end) as u32)));
            }
            rest = &rest[end..];
            continue;
        }
        // String literal — copy through unchanged (escape-aware)
        if ch == '"' || ch == '\'' {
            emit(&mut out, &mut rest, ch);
            copy_quoted(&mut out, &mut rest, ch);
            continue;
        }
        // url(...) — copy through to matching ')'
        if starts_with_url_open(rest) {
            out.push_str(&rest[..4]);
            rest = &rest[4..];
            copy_balanced_parens(&mut out, &mut rest);
            continue;
        }
        emit(&mut out, &mut rest, ch);
    }
    // Trim in place — truncate trailing whitespace, then drain leading — instead of
    // `out.trim().to_string()`, which copied the whole (already-owned) buffer again.
    let end = trim_wire_end(&out).len();
    let leading = out.len() - trim_wire_start(&out).len();
    if let Some(sink) = sink {
        // Svelte re-bases each captured position on the trimmed value by subtracting
        // the leading whitespace (clamped at zero, since a comment can sit ahead of
        // it), and never re-bases on the TRAILING trim — so a comment after the last
        // value token keeps a position past the end of the value it is attached to.
        // Positions are JS string indices, i.e. UTF-16 code units of that value.
        //
        // The offsets are recorded in source order, so the conversion walks `out`
        // once with a cursor rather than re-measuring each prefix from `leading`.
        sink.reserve(stripped.len());
        let (mut cursor, mut position) = (leading, 0usize);
        for (offset, span) in stripped {
            let offset = offset.max(leading);
            position += out[cursor..offset]
                .chars()
                .map(char::len_utf16)
                .sum::<usize>();
            cursor = offset;
            sink.push(WireComment {
                span,
                position: Some(position as u32),
            });
        }
    }
    out.truncate(end);
    out.drain(..leading.min(end));
    Cow::Owned(out)
}

/// Push `ch` to `out` and advance `rest` past it.
fn emit(out: &mut String, rest: &mut &str, ch: char) {
    out.push(ch);
    *rest = &rest[ch.len_utf8()..];
}

/// Copy a CSS string body (opening quote already emitted) through `out`,
/// advancing `rest` past the closing quote. Handles backslash escapes.
fn copy_quoted(out: &mut String, rest: &mut &str, quote: char) {
    while let Some(ch) = rest.chars().next() {
        emit(out, rest, ch);
        if ch == '\\' {
            if let Some(esc) = rest.chars().next() {
                emit(out, rest, esc);
            }
        } else if ch == quote {
            break;
        }
    }
}

/// Copy through `out` until the depth-1 close paren that ends `url(...)` (or eof).
/// Skips over quoted strings so embedded `)` characters are not treated as terminators.
fn copy_balanced_parens(out: &mut String, rest: &mut &str) {
    let mut depth: u32 = 1;
    while depth > 0 {
        let Some(ch) = rest.chars().next() else { break };
        emit(out, rest, ch);
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '"' | '\'' => copy_quoted(out, rest, ch),
            _ => {}
        }
    }
}

/// Whether `s` begins with `url(` (case-insensitive for `url`).
fn starts_with_url_open(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() >= 4
        && bytes[0].eq_ignore_ascii_case(&b'u')
        && bytes[1].eq_ignore_ascii_case(&b'r')
        && bytes[2].eq_ignore_ascii_case(&b'l')
        && bytes[3] == b'('
}

/// Convert PreludeValue to string representation for the public AST.
///
/// Svelte 5.55.x strips `/* ... */` block comments from at-rule preludes (surrounding
/// whitespace preserved, then trimmed). Applied to all source-extracted variants;
/// `Values` is built from parsed tokens that never contained comments.
pub(super) fn convert_prelude_to_string<'src>(
    prelude: &internal::PreludeValue<'_>,
    source: &'src str,
) -> Cow<'src, str> {
    match prelude {
        internal::PreludeValue::Values { span, .. } => {
            // Extract the prelude verbatim from source and strip comments, matching
            // Svelte (which removes `/* ... */` from the `@import` prelude string while
            // preserving the surrounding whitespace, then trims). Extracting from the
            // span (rather than rejoining the structured values) keeps the public AST
            // byte-for-byte with Svelte even when comments sit between the url/string and
            // the media query — the structured values exist for the printer's quote
            // normalization and media-query wrapping.
            strip_css_comments(span.extract(source))
        }
        // Extract verbatim from source (comments stripped, outer-trimmed) so the public
        // AST matches Svelte, which stores the raw prelude — e.g. `@layer a , b` → `a , b`
        // and `@namespace url(  x  )` → `url(  x  )`. The internal `content` string is a
        // normalized (printer-facing) form; the AST must stay source-faithful, like the
        // `Media`/`Supports`/`Container`/`Values` branches.
        internal::PreludeValue::Raw { span, .. } => strip_css_comments(span.extract(source)),
        // @scope selector lists: `[(root)]? [to (limit)]?`. Extracted verbatim from
        // `span` for fidelity (a bare `@scope` has a zero-width span → `""`), like the
        // sibling raw/condition branches.
        internal::PreludeValue::Selectors { span, .. } => strip_css_comments(span.extract(source)),
        internal::PreludeValue::Supports { span, .. } => strip_css_comments(span.extract(source)),
        internal::PreludeValue::Container { span, .. } => strip_css_comments(span.extract(source)),
        internal::PreludeValue::Media { span, .. } => strip_css_comments(span.extract(source)),
    }
}

/// Check if a complex selector contains Invalid simple selectors (from forgiving parsing)
pub(super) fn selector_contains_invalid(complex: &internal::ComplexSelector<'_>) -> bool {
    for relative in complex.children {
        for simple in relative.selectors {
            if matches!(simple, internal::SimpleSelector::Invalid { .. }) {
                return true;
            }
        }
    }
    false
}

/// Extract a selector name from source, skipping `prefix_len` bytes of sigil (`.`/`#`),
/// half-decoded the way Svelte's `read_identifier` does it: hex escapes (`\3A `,
/// `\1F4A9`, optional single whitespace terminator) decode to their codepoint, while
/// identity escapes (`\?`) keep the backslash. The internal AST stores the fully
/// decoded spec form; this reconstructs Svelte's public form at the boundary.
pub(super) fn raw_selector_name(source: &str, span: Span, prefix_len: usize) -> Cow<'_, str> {
    let raw = &source[span.start as usize + prefix_len..span.end as usize];
    // Fast path: no backslash means no escapes to decode, so the name is the raw
    // source slice verbatim — borrowed, no allocation. (The vast majority of names.)
    if !raw.contains('\\') {
        return Cow::Borrowed(raw);
    }
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        if chars.peek().is_some_and(char::is_ascii_hexdigit) {
            let mut hex = String::new();
            for _ in 0..6 {
                match chars.peek() {
                    Some(&d) if d.is_ascii_hexdigit() => {
                        hex.push(d);
                        chars.next();
                    }
                    _ => break,
                }
            }
            // Optional single whitespace terminator (Svelte: `(\r\n|\s)?`).
            //
            // ⚠️ [`is_js_whitespace`] — that `\s` is a JS regex class, **not**
            // `char::is_whitespace`. This is the SECOND reader of the terminator rule (the
            // lexer's `read_identifier` is the first, and decides the token BOUNDARY), so
            // the two must ask the same class or the wire disagrees with the span it was cut
            // from: `.a\41<ZWNBSP>b` is `aAb` to `parseCss` and `.a\41<NEL>b` is a rejection.
            if chars.peek() == Some(&'\r') {
                chars.next();
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
            } else if chars.peek().copied().is_some_and(is_js_whitespace) {
                chars.next();
            }
            // Surrogate/overflow codepoints are unrepresentable in Rust strings —
            // dropped, same as `escapes::decode_escape_sequences`
            if let Ok(cp) = u32::from_str_radix(&hex, 16)
                && let Some(c) = char::from_u32(cp)
            {
                out.push(c);
            }
        } else if let Some(next) = chars.next() {
            out.push('\\');
            out.push(next);
        } else {
            out.push('\\');
        }
    }
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Owns the `Cow` result so assertions can compare against `&str` literals.
    fn strip(s: &str) -> String {
        strip_css_comments(s).into_owned()
    }

    #[test]
    fn strip_css_comments_basic_removal_and_trim() {
        assert_eq!(strip("/* c */ 12px"), "12px");
        assert_eq!(strip("blue /* c */"), "blue");
        assert_eq!(strip("/* a */ red"), "red");
    }

    #[test]
    fn strip_css_comments_interior_whitespace_preserved() {
        assert_eq!(strip("var(--a, /* c */ red)"), "var(--a,  red)",);
        assert_eq!(
            strip("sidebar /* x */ (min-width: 100px)"),
            "sidebar  (min-width: 100px)",
        );
    }

    #[test]
    fn strip_css_comments_inside_strings_are_preserved() {
        assert_eq!(strip("\"/* not a comment */\""), "\"/* not a comment */\"",);
        assert_eq!(strip("'/* keep */'"), "'/* keep */'");
    }

    #[test]
    fn strip_css_comments_inside_url_are_preserved() {
        assert_eq!(
            strip("url(\"data:image/svg+xml,/* x */\")"),
            "url(\"data:image/svg+xml,/* x */\")",
        );
    }

    #[test]
    fn strip_css_comments_inside_other_functions_are_stripped() {
        // Only url() is special — calc/var/etc. follow normal CSS tokenization,
        // so block comments inside them are stripped just like at top level.
        assert_eq!(strip("calc(/* x */ 1px + 2px)"), "calc( 1px + 2px)",);
        assert_eq!(strip("URL(/* keep */)"), "URL(/* keep */)");
    }

    #[test]
    fn strip_css_comments_unterminated_kept_verbatim() {
        assert_eq!(strip("red /* oops"), "red /* oops");
    }

    #[test]
    fn strip_css_comments_escaped_quote_does_not_close_string() {
        assert_eq!(
            strip("\"a\\\" /* in str */ b\" /* real */ c"),
            "\"a\\\" /* in str */ b\"  c",
        );
    }

    /// One recorded comment, flattened: `(span start, span end, position)`.
    type Recorded = (u32, u32, Option<u32>);

    /// The emitted value plus what a collecting strip recorded alongside it.
    fn collect(input: &str, base: u32) -> (String, Vec<Recorded>) {
        let mut sink = Vec::new();
        let value = strip_css_comments_collecting(input, base, &mut sink).into_owned();
        let recorded = sink
            .iter()
            .map(|c| (c.span.start, c.span.end, c.position))
            .collect();
        (value, recorded)
    }

    /// A `position` is an index into the *emitted* value, so it can only be
    /// graded beside the value it indexes — which is why these assert the pair.
    #[test]
    fn strip_css_comments_collecting_records_spans_and_positions() {
        // Spans are document-absolute (`base`-shifted); positions are not.
        assert_eq!(
            collect(" red /* c */ blue", 100),
            ("red  blue".to_owned(), vec![(105, 112, Some(4))]),
        );
        // Every comment, in source order, each measured on the value so far.
        assert_eq!(
            collect("a /* c1 */ b /* c2 */ c", 0),
            (
                "a  b  c".to_owned(),
                vec![(2, 10, Some(2)), (13, 21, Some(5))],
            ),
        );
    }

    /// The three edges of Svelte's own rebasing, which is what the offsets mean.
    #[test]
    fn strip_css_comments_collecting_position_edges() {
        // Clamped at zero: a comment AHEAD of the leading whitespace it is
        // rebased by (`max(0, …)` in `read_value`).
        assert_eq!(
            collect("/* c */ red", 0),
            ("red".to_owned(), vec![(0, 7, Some(0))])
        );
        // Never rebased on the TRAILING trim, so a comment after the last token
        // keeps a position one past the end of the value it is attached to.
        assert_eq!(
            collect("red /* c */", 0),
            ("red".to_owned(), vec![(4, 11, Some(4))])
        );
        // UTF-16 code units, not bytes or chars — a JS string index. The span
        // stays in BYTES (7 of them precede the comment; the value is 5 units).
        assert_eq!(
            collect("'😀' /* c */", 0),
            ("'😀'".to_owned(), vec![(7, 14, Some(5))]),
        );
    }

    /// An unterminated comment is kept verbatim rather than swallowed, and goes
    /// unrecorded — Svelte's `read_comment` errors on one instead of capturing it.
    #[test]
    fn strip_css_comments_collecting_skips_unterminated() {
        assert_eq!(
            collect("red /* oops", 0),
            ("red /* oops".to_owned(), vec![])
        );
    }
}
