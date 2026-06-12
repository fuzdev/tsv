//! Shared `url(...)` token handling.
//!
//! An unquoted `url(...)` is an opaque `<url-token>` (css-syntax-3): its content —
//! colons (`http://`), commas, escapes — is preserved verbatim, with only the
//! whitespace just inside the parens trimmed. This single rule is needed in two places
//! that can't depend on each other: the printer's declaration-value path
//! (`printer/values.rs`) and the parser's raw at-rule prelude
//! (`parser/atrules.rs`, e.g. `@namespace url(http://…)`), so it lives here at the
//! crate root rather than in either sibling module.

/// Trim a raw `url(...)` token to prettier's canonical form: strip only the whitespace
/// immediately inside the parens (after `(`, before the final `)`), leaving the opaque
/// content — including colons and commas — verbatim, and preserving the original
/// `url`/`URL` casing. Returns `None` when the raw text isn't a parenthesized token, so
/// the caller can fall back (rejoin parsed args, or keep the source slice as-is).
pub(crate) fn trim_url_raw(raw: &str) -> Option<String> {
    let open = raw.find('(')?;
    let close = raw.rfind(')')?;
    if close < open {
        return None;
    }
    let inner = raw[open + 1..close].trim();
    Some(format!("{}{})", &raw[..=open], inner))
}
