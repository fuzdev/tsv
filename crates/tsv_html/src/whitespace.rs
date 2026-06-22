// HTML whitespace preservation classification (language-level)
//
// Pure functions for determining which HTML elements preserve whitespace.
// These are language-level utilities independent of any specific tool.
//
// Reference: HTML spec `white-space: pre` UA rule — ../html/source, Rendering §"Flow content"

/// Check if an HTML element preserves whitespace
///
/// Elements like `<pre>` and `<textarea>` render whitespace literally,
/// without collapsing multiple spaces or trimming leading/trailing whitespace.
///
/// Examples: `<pre>`, `<textarea>`
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
