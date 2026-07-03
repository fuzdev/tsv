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
// (`strip_css_comments`, `split_declaration_svelte_compat`,
// `raw_selector_name`, …), so the Svelte scan semantics live in one place. It
// is the sole emission path; `convert_ast_json_bytes` calls it and
// `convert_ast_json` parses its bytes back into a `Value`.

use super::internal;
use std::borrow::Cow;
use tsv_lang::Span;
use tsv_lang::source_scan::{TriviaProfile, find_char};

mod write;
pub use write::write_css_node;
pub(crate) use write::write_stylesheet_file_bytes;

/// Whether the wire JSON is being built for a standalone `.css` file or an
/// embedded `<style>` block. `parseCss()` attaches constant `metadata` to
/// `Rule`/`ComplexSelector`/`RelativeSelector` for standalone CSS but never for
/// embedded `<style>`; the writer threads this so one node pass produces both
/// shapes — no separate metadata walk.
#[derive(Debug, Clone, Copy)]
pub(super) enum AstScope {
    /// Standalone `.css` file (`parseCss()` shape, `metadata` attached).
    Standalone,
    /// Embedded `<style>` block in a `.svelte` file (no `metadata`).
    Embedded,
}

impl AstScope {
    /// Standalone CSS carries `parseCss()` metadata; embedded `<style>` doesn't.
    pub(super) fn has_metadata(self) -> bool {
        matches!(self, AstScope::Standalone)
    }
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
pub(super) fn split_declaration_svelte_compat(decl_source: &str) -> (&str, &str) {
    // The real `property : value` colon is the first one outside any comment or
    // string — a property comment may itself contain a `:` (`color /* x:y */: red`).
    let Some(colon_pos) = find_char(
        decl_source.as_bytes(),
        0,
        decl_source.len(),
        b':',
        TriviaProfile::CSS,
    ) else {
        return (decl_source, "");
    };

    let before_colon = &decl_source[..colon_pos];

    // Look for /* that appears after some property text
    if let Some(comment_idx) = before_colon.find("/*") {
        // Only apply quirk if there's actual property content before the comment
        let before_comment = &before_colon[..comment_idx];
        if !before_comment.trim().is_empty() {
            // SVELTE QUIRK: Comment between property and colon
            // Property = just the text before the comment (trimmed)
            // Value = comment + colon + actual value (everything from comment onward)
            let property = before_comment.trim();
            let value = &decl_source[comment_idx..];
            return (property, value);
        }
    }

    // Normal case: split at colon
    let property = &decl_source[..colon_pos];
    let value = decl_source[colon_pos + 1..].trim_start();
    (property, value)
}

/// Remove all `/* ... */` block comments from a CSS string, then trim outer whitespace.
///
/// Matches Svelte 5.55+ behavior for Declaration `value` and Atrule `prelude` strings:
/// comments are stripped in place (surrounding whitespace preserved), then the result
/// is trimmed.
///
/// String- and url()-aware: `/*` sequences inside `"..."`, `'...'`, or `url(...)` are
/// treated as content, not comments. Unterminated comments are left intact (parse
/// error caught elsewhere).
pub(super) fn strip_css_comments(input: &str) -> Cow<'_, str> {
    // Fast path: no block-comment delimiter anywhere means nothing is stripped, so
    // the result is just the trimmed input — a borrowed sub-slice, no allocation.
    // (Conservative: a `/*` inside a string/url is preserved either way, so those
    // rare inputs fall to the owned path; correctness is unaffected.)
    if !input.contains("/*") {
        return Cow::Borrowed(input.trim());
    }
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(ch) = rest.chars().next() {
        // Block comment — strip
        if ch == '/' && rest.as_bytes().get(1) == Some(&b'*') {
            if let Some(end_rel) = rest[2..].find("*/") {
                rest = &rest[2 + end_rel + 2..];
                continue;
            }
            // Unterminated — keep verbatim
            out.push_str(rest);
            break;
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
    Cow::Owned(out.trim().to_string())
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

/// Advance past whitespace and block comments to the `;`/`}` terminator, returning its index.
///
/// Mirrors Svelte's `read_declaration`: `read_value` returns with the scan index AT the
/// terminator and the declaration's `end` is taken there — so trailing whitespace and
/// comments after the value (and after `!important`) sit inside the declaration extent.
/// Only whitespace, comments, and the `!important` tail can occur between the parsed
/// value's end and the terminator, so a flat byte walk is safe (no string/url content).
pub(super) fn scan_to_terminator(source: &str, from: usize) -> usize {
    let bytes = source.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        match bytes[i] {
            b';' | b'}' => break,
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i = source[i + 2..]
                    .find("*/")
                    .map_or(bytes.len(), |rel| i + 2 + rel + 2);
            }
            _ => i += 1,
        }
    }
    i
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
            // Optional single whitespace terminator (Svelte: `(\r\n|\s)?`)
            if chars.peek() == Some(&'\r') {
                chars.next();
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
            } else if chars.peek().is_some_and(|c| c.is_whitespace()) {
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

/// The end position of a pseudo selector's name, excluding any `(args)`.
///
/// A pseudo's `span` covers the whole `:name(args)` / `::name(args)`, so when it has
/// arguments the name runs only up to the first `(`; without arguments the whole span is
/// the name. Used to bound the `raw_selector_name` slice (and, for pseudo-elements, the
/// public `end`) to just the name — the decoded internal name is never re-serialized.
pub(super) fn pseudo_name_end(source: &str, span: Span, has_args: bool) -> u32 {
    if has_args {
        let raw = &source[span.start as usize..span.end as usize];
        raw.find('(').map_or(span.end, |i| span.start + i as u32)
    } else {
        span.end
    }
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
}
