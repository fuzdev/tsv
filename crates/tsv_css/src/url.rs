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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_inner_whitespace_preserving_content_and_casing() {
        // Inner whitespace trimmed; colons/slashes in the opaque content kept.
        assert_eq!(
            trim_url_raw("url( http://x.com )").as_deref(),
            Some("url(http://x.com)")
        );
        // Original url/URL casing is preserved.
        assert_eq!(trim_url_raw("URL(  a  )").as_deref(), Some("URL(a)"));
        // Whitespace-only content collapses to empty parens.
        assert_eq!(trim_url_raw("url(  )").as_deref(), Some("url()"));
    }

    #[test]
    fn returns_none_when_not_parenthesized() {
        assert_eq!(trim_url_raw("noparens"), None);
        // A ')' before the '(' is not a valid token.
        assert_eq!(trim_url_raw(")x("), None);
    }

    #[test]
    fn matches_any_parenthesized_token_and_uses_last_paren() {
        // Not restricted to the `url` prefix — any parenthesized token works.
        assert_eq!(trim_url_raw("(x)").as_deref(), Some("(x)"));
        // rfind(')') is used, so inner content may itself contain ')'.
        assert_eq!(trim_url_raw("url(a)b)").as_deref(), Some("url(a)b)"));
    }
}
