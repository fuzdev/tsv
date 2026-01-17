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
    // SAFETY: serde_json always produces valid UTF-8
    #[allow(clippy::unwrap_used)]
    Ok(String::from_utf8(buf).unwrap())
}
