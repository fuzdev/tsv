//! The **line-shape alphabet** the pristine-sweep ratchets key their snapshots
//! on — specifically the arm that answers "how does a line that opens with
//! MARKUP shape?".
//!
//! `fabrication_audit` (what brackets an invented blank) and `width_audit` (how
//! an over-width line opens) ask different questions, but both need the same
//! answer for markup: a tag name without its attributes, a block keyword
//! without its expression. That arm lives here so the two committed snapshots
//! share ONE alphabet — widening the name class or admitting a new block sigil
//! is a single edit, not a silent divergence between two ratchets whose keys
//! then mean subtly different things.
//!
//! What is deliberately NOT here is each audit's fallback for a line markup
//! does not open — `fabrication`'s flat `text`, `width`'s comment / keyword /
//! `IDENT` / punctuation ladder. Those answer the audits' own questions and
//! share nothing, so folding them together would be an abstraction over a
//! coincidence.

/// How a line that OPENS WITH MARKUP shapes, or `None` when it opens with
/// something else (prose, a comment, code — the caller's own question).
///
/// `trimmed` must already be trimmed: the shape reads the first byte, and a
/// leading tab would send every indented line down the `None` arm.
///
/// The alphabet is deliberately coarse — a tag name without its attributes, a
/// block keyword without its expression, a bare `{` for a plain interpolation —
/// because a snapshot key must survive an ordinary fixture edit (renaming a
/// class, changing an expression) without churning.
pub(crate) fn markup_head(trimmed: &str) -> Option<String> {
    if trimmed.starts_with("<!--") {
        return Some("<!--".to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("</") {
        return Some(format!("</{}", name_run(rest)));
    }
    if let Some(rest) = trimmed.strip_prefix('<') {
        return Some(format!("<{}", name_run(rest)));
    }
    if let Some(rest) = trimmed.strip_prefix('{') {
        return Some(match rest.chars().next() {
            Some(sigil @ ('#' | ':' | '/' | '@')) => {
                format!("{{{sigil}{}", name_run(&rest[sigil.len_utf8()..]))
            }
            // A plain `{expr}` interpolation — the expression itself is not part of the shape.
            _ => "{".to_string(),
        });
    }
    None
}

/// The leading name token of `s` — letters, digits, `:`, `-`, `_`, `$`.
///
/// `$` is inert for a markup name (no tag or block keyword can carry one) and
/// load-bearing for `width_audit`'s identifier arm, which runs this over JS
/// source where `$state` / `$derived` open a line.
pub(crate) fn name_run(s: &str) -> String {
    s.chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '-' | '_' | '$'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{markup_head, name_run};

    #[test]
    fn markup_heads_drop_attributes_and_expressions() {
        assert_eq!(
            markup_head("<div class=\"a\" data-x=\"v\">").as_deref(),
            Some("<div")
        );
        assert_eq!(markup_head("</div>").as_deref(), Some("</div"));
        assert_eq!(
            markup_head("<!-- anything at all -->").as_deref(),
            Some("<!--")
        );
        assert_eq!(
            markup_head("<svelte:options runes />").as_deref(),
            Some("<svelte:options")
        );
        assert_eq!(markup_head("{#if cond}").as_deref(), Some("{#if"));
        assert_eq!(markup_head("{:catch error}").as_deref(), Some("{:catch"));
        assert_eq!(markup_head("{/await}").as_deref(), Some("{/await"));
        assert_eq!(markup_head("{@debug a, b}").as_deref(), Some("{@debug"));
        assert_eq!(markup_head("{expr}").as_deref(), Some("{"));
    }

    /// Anything that does not open with markup is the CALLER's question, so the
    /// shared arm must decline rather than guess — a `text` / `IDENT` answer
    /// here would silently pick one audit's alphabet for the other.
    #[test]
    fn a_non_markup_line_declines() {
        assert_eq!(markup_head("some prose"), None);
        assert_eq!(markup_head("// a line comment"), None);
        assert_eq!(markup_head("const a = 1;"), None);
        assert_eq!(markup_head("| SomeUnionMember"), None);
        assert_eq!(markup_head(""), None);
    }

    #[test]
    fn name_runs_stop_at_the_first_non_name_byte() {
        assert_eq!(name_run("div class=\"a\">"), "div");
        assert_eq!(name_run("svelte:options runes />"), "svelte:options");
        assert_eq!(name_run("$derived(x)"), "$derived");
        assert_eq!(name_run("(paren)"), "");
    }
}
