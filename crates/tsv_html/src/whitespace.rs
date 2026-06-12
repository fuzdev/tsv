// HTML whitespace preservation classification (language-level)
//
// Pure functions for determining which HTML elements preserve whitespace.
// These are language-level utilities independent of any specific tool.
//
// Reference: HTML spec white-space: pre (WHITESPACE_HTML.md line 145237)

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
