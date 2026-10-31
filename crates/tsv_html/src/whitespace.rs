// HTML whitespace preservation classification (language-level)
//
// Pure functions for determining which HTML elements preserve whitespace.
// These are language-level utilities independent of any specific tool.
//
// The set is Svelte's and prettier-plugin-svelte's (`<pre>`, `<textarea>`), not the HTML
// spec's `white-space: pre` UA rule (../html/source, Rendering §"Flow content"), which also
// names `listing`, `plaintext` and `xmp` and gives `textarea` `pre-wrap`.

/// Whether an HTML element preserves the whitespace of its content: `<pre>` and `<textarea>`,
/// whose content is rendered with every space and line break kept, so a formatter must not
/// collapse, trim or re-indent it.
///
/// The set is the one Svelte's compiler and prettier-plugin-svelte treat that way, not the
/// HTML spec's: the spec's user-agent stylesheet also sets `white-space: pre` on the obsolete
/// `<listing>`, `<plaintext>` and `<xmp>`, which neither tool special-cases (they format as
/// ordinary inline elements), so a Svelte-compatible formatter leaves them out too.
#[inline]
pub fn preserves_whitespace(tag_name: &str) -> bool {
    matches!(tag_name, "pre" | "textarea")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preserves_whitespace() {
        assert!(preserves_whitespace("pre"));
        assert!(preserves_whitespace("textarea"));
        assert!(!preserves_whitespace("div"));
        // `<code>` is a common false-friend — it does not preserve whitespace.
        assert!(!preserves_whitespace("code"));
        // Case-sensitive: callers pass already-lowercased tag names.
        assert!(!preserves_whitespace("PRE"));
    }
}
