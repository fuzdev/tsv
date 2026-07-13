//! Shared `url(...)` token handling.
//!
//! An unquoted `url(...)` is an opaque `<url-token>` (css-syntax-3): its content —
//! colons (`http://`), commas, escapes — is preserved verbatim, with only the
//! whitespace just inside the parens trimmed. This single rule is needed in two places
//! that can't depend on each other: the printer's declaration-value path
//! (`printer/values.rs`) and the parser's raw at-rule prelude
//! (`parser/atrules/raw.rs`, e.g. `@namespace url(http://…)`), so it lives here at the
//! crate root rather than in either sibling module.

/// Trim a raw `url(...)` token to prettier's canonical form: strip only the whitespace
/// immediately inside the parens (after `(`, before the final `)`), leaving the opaque
/// content — including colons and commas — verbatim, and preserving the original
/// `url`/`URL` casing. Returns `None` when the raw text isn't a parenthesized token, so
/// the caller can fall back (rejoin parsed args, or keep the source slice as-is).
///
/// The trailing trim spares an **escape's payload**: the url tokenizer consumes `\ ` as
/// an escape (css-syntax-3 §4.3.6 defers to §4.3.7), so `url(x\ )` is the url `x `, and
/// that space is content. Trimming it strands the backslash onto the closing `)`, which
/// it then escapes — the url token never terminates and the output stops parsing
/// entirely.
pub(crate) fn trim_url_raw(raw: &str) -> Option<String> {
    let open = raw.find('(')?;
    let close = raw.rfind(')')?;
    if close < open {
        return None;
    }
    let inner = crate::escapes::trim_end_preserving_escape(raw[open + 1..close].trim_start());
    Some(format!("{}{})", &raw[..=open], inner))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_inner_whitespace_preserving_content_and_casing() {
        // Inner whitespace trimmed; colons/slashes in the opaque content kept.
        assert_eq!(
            trim_url_raw("url( https://fuz.dev )").as_deref(),
            Some("url(https://fuz.dev)")
        );
        // Original url/URL casing is preserved.
        assert_eq!(trim_url_raw("URL(  foo  )").as_deref(), Some("URL(foo)"));
        // Whitespace-only content collapses to empty parens.
        assert_eq!(trim_url_raw("url(  )").as_deref(), Some("url()"));
    }

    #[test]
    fn returns_none_when_not_parenthesized() {
        assert_eq!(trim_url_raw("noparens"), None);
        // A ')' before the '(' is not a valid token.
        assert_eq!(trim_url_raw(")foo("), None);
    }

    #[test]
    fn matches_any_parenthesized_token_and_uses_last_paren() {
        // Not restricted to the `url` prefix — any parenthesized token works.
        assert_eq!(trim_url_raw("(foo)").as_deref(), Some("(foo)"));
        // rfind(')') is used, so inner content may itself contain ')'.
        assert_eq!(trim_url_raw("url(a)b)").as_deref(), Some("url(a)b)"));
    }
}
