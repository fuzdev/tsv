//! The Svelte property/value split of a comment-bearing declaration — mounted once per copy of
//! the wire writer (here for the JSON copy, in `write_tape.rs` for the tape copy), so the JSON
//! copy's helper keeps its own caller set in every build: shared, it was outlined out of the
//! JSON copy once the tape copy called it too.

use crate::ast::convert::{trim_wire, trim_wire_start};

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
pub(in crate::ast) fn split_declaration_svelte_compat(
    decl_source: &str,
    colon_pos: usize,
) -> (&str, &str) {
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
