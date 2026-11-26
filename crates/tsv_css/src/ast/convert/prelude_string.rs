//! An at-rule prelude's wire string — mounted once per copy of the wire writer, as
//! `declaration_split.rs` is and for its reason.

use crate::ast;
use crate::ast::convert::strip_css_comments;
use std::borrow::Cow;

/// Convert PreludeValue to string representation for the public AST.
///
/// Svelte 5.55.x strips `/* ... */` block comments from at-rule preludes (surrounding
/// whitespace preserved, then trimmed). Applied to all source-extracted variants;
/// `Values` is built from parsed tokens that never contained comments.
pub(in crate::ast) fn convert_prelude_to_string<'src>(
    prelude: &ast::PreludeValue,
    source: &'src str,
) -> Cow<'src, str> {
    match prelude {
        ast::PreludeValue::Values { span, .. } => {
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
        ast::PreludeValue::Raw { span, .. } => strip_css_comments(span.extract(source)),
        // @scope selector lists: `[(root)]? [to (limit)]?`. Extracted verbatim from
        // `span` for fidelity (a bare `@scope` has a zero-width span → `""`), like the
        // sibling raw/condition branches.
        ast::PreludeValue::Selectors { span, .. } => strip_css_comments(span.extract(source)),
        // `@custom-selector`: the span is the raw reader's, so the routed and the
        // verbatim-fallback preludes emit the same bytes.
        ast::PreludeValue::CustomSelector { span, .. } => strip_css_comments(span.extract(source)),
        ast::PreludeValue::Supports { span, .. } => strip_css_comments(span.extract(source)),
        ast::PreludeValue::Container { span, .. } => strip_css_comments(span.extract(source)),
        ast::PreludeValue::Media { span, .. } => strip_css_comments(span.extract(source)),
    }
}
