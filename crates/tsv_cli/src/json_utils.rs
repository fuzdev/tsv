//! JSON serialization utilities

use serde::Serialize;

/// Serialize to JSON with tab indentation
///
/// `serde_json::to_string_pretty` uses 2 spaces by default.
/// This function uses tabs to match our codebase formatting conventions.
///
/// # Examples
///
/// ```rust,ignore
/// let value = json!({"type": "Root", "css": null});
/// let json = to_json_with_tabs(&value)?;
/// // Output uses tab indentation:
/// // {
/// //     "type": "Root",
/// //     "css": null
/// // }
/// ```
pub fn to_json_with_tabs<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    value.serialize(&mut ser)?;
    Ok(String::from_utf8(buf).unwrap())
}

/// Ensure a string ends with a newline character
///
/// If the string already ends with `\n`, returns it unchanged.
/// Otherwise, appends a newline.
///
/// # Examples
///
/// ```rust
/// # use tsv_cli::json_utils::ensure_trailing_newline;
/// assert_eq!(ensure_trailing_newline("hello".to_string()), "hello\n");
/// assert_eq!(ensure_trailing_newline("hello\n".to_string()), "hello\n");
/// ```
pub fn ensure_trailing_newline(s: String) -> String {
    if s.ends_with('\n') {
        s
    } else {
        format!("{}\n", s)
    }
}
