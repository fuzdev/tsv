//! Svelte's tag-name answers: the character class its tag-name pattern admits after a hyphen,
//! and the table its parser closes elements by when an end tag is omitted.
//!
//! Both are Svelte's, not the HTML spec's. The class is the tail of Svelte's
//! `REGEX_VALID_TAG_NAME`, which follows an older `PCENChar` production the spec has since
//! replaced (a custom element name is now any valid element local name that starts with an ASCII
//! lowercase letter, holds a hyphen and no ASCII uppercase). The table is Svelte's
//! `autoclosing_children`, a reading of the spec's "Optional tags" section that departs from it
//! in both directions (see [`OptionalEndTag`]).

/// A character Svelte admits in a tag name's run after its first hyphen
/// (`<my-café>`, `<emotion-😍>`, `<a-·>`): the tail class of Svelte's
/// `REGEX_VALID_TAG_NAME` (the Svelte repository's `packages/svelte/src/utils.js`), whose
/// non-ASCII ranges are the `PCENChar` production the HTML spec once defined custom element
/// names by. Unlike `PCENChar` it admits ASCII uppercase `A`–`Z`, as Svelte's class does.
/// Surrogate code points are unreachable, since a Rust `char` is never a surrogate.
///
/// Used both by the tokenizer (to keep a whole custom-element name in one token,
/// including the non-alphanumeric members `·`/ZWNJ/ZWJ/astral) and by the name
/// validator (the hyphen-tail run) — a single source of truth for the ranges.
#[must_use]
pub(crate) fn is_custom_element_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(c, '-' | '.' | '_' | '\u{00B7}')
        || matches!(c,
            '\u{00C0}'..='\u{00D6}' | '\u{00D8}'..='\u{00F6}' | '\u{00F8}'..='\u{037D}'
            | '\u{037F}'..='\u{1FFF}' | '\u{200C}'..='\u{200D}' | '\u{203F}'..='\u{2040}'
            | '\u{2070}'..='\u{218F}' | '\u{2C00}'..='\u{2FEF}' | '\u{3001}'..='\u{D7FF}'
            | '\u{F900}'..='\u{FDCF}' | '\u{FDF0}'..='\u{FFFD}' | '\u{10000}'..='\u{EFFFF}')
}

/// Declare the optional-end-tag table once: [`optional_end_tag`], a `match` over the keys, and
/// [`OptionalEndTag::is_closed_by`], a `match` per key over its triggers — plus, for the tests,
/// the same table as a plain slice to grade both against.
///
/// A `match` rather than a perfect hash: the keys and triggers are a few short names, which a
/// length switch and a few word compares answer outright, where a hash reads every byte of the
/// name through SipHash before it can compare anything.
macro_rules! optional_end_tags {
    ($($variant:ident: $key:literal => [$($trigger:literal),+ $(,)?]),+ $(,)?) => {
        /// Which row of the table an [`OptionalEndTag`] is — one variant per key.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        enum OptionalEndTagKey {
            $($variant),+
        }

        /// The table's row for `tag_name`: whether Svelte's parser closes `tag_name` without an
        /// end tag, and if so which following tags close it. `None` for every other element,
        /// which never auto-closes.
        ///
        /// Exact and case-sensitive: tag names arrive as authored.
        #[inline]
        #[must_use]
        pub(crate) fn optional_end_tag(tag_name: &str) -> Option<OptionalEndTag> {
            Some(OptionalEndTag(match tag_name {
                $($key => OptionalEndTagKey::$variant,)+
                _ => return None,
            }))
        }

        impl OptionalEndTag {
            /// Whether `next`, as the next opening tag in the markup, implicitly closes this
            /// element — one of the key's listed triggers.
            #[inline]
            #[must_use]
            pub(crate) fn is_closed_by(self, next: &str) -> bool {
                match self.0 {
                    $(OptionalEndTagKey::$variant => matches!(next, $($trigger)|+),)+
                }
            }
        }

        /// The table as a plain slice — what the equivalence test grades both lookups against.
        #[cfg(test)]
        const OPTIONAL_END_TAG_TABLE: &[(&str, &[&str])] = &[$(($key, &[$($trigger),+])),+];
    };
}

/// An element Svelte's parser closes without an end tag (auto-close) when a particular following
/// tag or the end of its parent arrives — a row of Svelte's `autoclosing_children` table, found
/// by [`optional_end_tag`].
///
/// Svelte's table, not the spec's "Optional tags" rules: it has no row for the elements whose
/// end tag the spec lets an author omit only at a whitespace or comment boundary (`html`,
/// `head`, `body`, `colgroup`, `caption`); its `<p>` triggers lack `details`, `dialog`,
/// `figcaption`, `figure` and `search`, and its `<option>` / `<optgroup>` triggers lack `hr`;
/// it closes `<tr>` at `<tbody>`, `<td>` / `<th>` at `<tr>` and `<tfoot>` at `<tbody>`, which
/// the spec does not; and every row closes at the end of its parent, where the spec excepts
/// `dt` and `thead`, and `p` inside some parents.
///
/// Mirrors Svelte's `closing_tag_omitted(current, next)`: `current` auto-closes when it has a row
/// and `next` is either absent (the end of the parent's content / EOF — having a row at all) or
/// one of its listed triggers ([`is_closed_by`](Self::is_closed_by)). Split in two so a parser
/// can ask the first half once per element and the second once per child.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct OptionalEndTag(OptionalEndTagKey);

// Svelte's `autoclosing_children` (the Svelte repository's
// `packages/svelte/src/html-tree-validation.js`),
// which it bases on the spec's tag-omission section.
//
// Each entry lists the "next" tag names that force the key element to auto-close.
// Svelte's source splits these into `direct` (immediate child) and `descendant`
// (any descendant) variants, but that split only affects the *validation* error
// wording — Svelte's `closing_tag_omitted` treats both as one membership test — so the two
// are flattened here into a single trigger list per element.
//
// Scope: this is only the *parse-time* auto-close half. Svelte's validation-side
// table (`disallowed_children` + `is_tag_valid_with_parent`/`_ancestor`, which do
// need the direct/descendant/`reset_by`/`only` distinctions) belongs to a future
// diagnostics layer and is a separate port from the same source file.
optional_end_tags! {
    Li: "li" => ["li"],
    Dt: "dt" => ["dt", "dd"],
    Dd: "dd" => ["dt", "dd"],
    P: "p" => [
        "address", "article", "aside", "blockquote", "div", "dl", "fieldset",
        "footer", "form", "h1", "h2", "h3", "h4", "h5", "h6", "header", "hgroup",
        "hr", "main", "menu", "nav", "ol", "p", "pre", "section", "table", "ul",
    ],
    Rt: "rt" => ["rt", "rp"],
    Rp: "rp" => ["rt", "rp"],
    Optgroup: "optgroup" => ["optgroup"],
    OptionEl: "option" => ["option", "optgroup"],
    Thead: "thead" => ["tbody", "tfoot"],
    Tbody: "tbody" => ["tbody", "tfoot"],
    Tfoot: "tfoot" => ["tbody"],
    Tr: "tr" => ["tr", "tbody"],
    Td: "td" => ["td", "th", "tr"],
    Th: "th" => ["td", "th", "tr"],
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Element names beside the table's own: block-level names (`<p>`'s triggers are a near-copy
    /// of them, so a block name missing from the triggers is the likeliest slip) and void ones.
    const NEIGHBOR_PROBES: &[&str] = &[
        "address",
        "article",
        "aside",
        "blockquote",
        "details",
        "dialog",
        "dd",
        "div",
        "dl",
        "dt",
        "fieldset",
        "figcaption",
        "figure",
        "footer",
        "form",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "header",
        "hgroup",
        "hr",
        "li",
        "main",
        "menu",
        "nav",
        "ol",
        "p",
        "pre",
        "section",
        "summary",
        "table",
        "ul",
        "area",
        "base",
        "br",
        "col",
        "embed",
        "img",
        "input",
        "link",
        "meta",
        "source",
        "track",
        "wbr",
    ];

    /// Svelte's `closing_tag_omitted(current, next)`, composed from the two halves the way a
    /// caller asks them: a row for `current`, then `next` absent or among its triggers.
    fn closing_tag_omitted(current: &str, next: Option<&str>) -> bool {
        optional_end_tag(current).is_some_and(|row| next.is_none_or(|next| row.is_closed_by(next)))
    }

    /// Grade both `match`es against a plain scan of the table they were generated from, over
    /// every (current, next) pair of a probe alphabet: every key and every trigger, each cased
    /// three ways and one byte off in both directions, plus [`NEIGHBOR_PROBES`] and the empty and
    /// non-ASCII inputs. Lookups are exact: a case variant is a different name, never a member.
    #[test]
    fn optional_end_tag_agrees_with_a_plain_scan() {
        let mut probes: Vec<String> = OPTIONAL_END_TAG_TABLE
            .iter()
            .flat_map(|(key, triggers)| std::iter::once(key).chain(triggers.iter()))
            .chain(NEIGHBOR_PROBES)
            .flat_map(|name| {
                [
                    (*name).to_string(),
                    name.to_uppercase(),
                    format!("{}{}", name[..1].to_uppercase(), &name[1..]),
                    format!("{name}x"),
                    name[..name.len() - 1].to_string(),
                    format!("x{name}"),
                ]
            })
            .chain(
                [
                    "", "span", "Li", "LI", "td ", " td", "é", "tbodyé", "!doctype", "Button",
                ]
                .iter()
                .map(|s| (*s).to_string()),
            )
            .collect();
        probes.sort();
        probes.dedup();

        let mut cases = 0usize;
        for current in &probes {
            let row = OPTIONAL_END_TAG_TABLE
                .iter()
                .find(|(key, _)| *key == current.as_str());
            assert_eq!(
                optional_end_tag(current).is_some(),
                row.is_some(),
                "optional_end_tag disagrees with the table on {current:?}"
            );
            for next in &probes {
                let expected = row.is_some_and(|(_, triggers)| triggers.contains(&next.as_str()));
                assert_eq!(
                    optional_end_tag(current).is_some_and(|row| row.is_closed_by(next)),
                    expected,
                    "is_closed_by disagrees with the table on {current:?} then {next:?}"
                );
                cases += 1;
            }
        }
        // Every key and every trigger is a probe, so the table's whole content is graded.
        assert!(cases > 10_000, "probe alphabet shrank: {cases} cases");
    }

    #[test]
    fn test_closing_tag_omitted() {
        // `<li>` auto-closes at a sibling `<li>`, and at end-of-parent (`None`).
        assert!(closing_tag_omitted("li", Some("li")));
        assert!(closing_tag_omitted("li", None));
        // ...but not at an unrelated sibling.
        assert!(!closing_tag_omitted("li", Some("span")));
        assert!(!closing_tag_omitted("li", Some("ul")));

        // `<p>` auto-closes at block-level siblings (descendant list), incl. `<div>`.
        assert!(closing_tag_omitted("p", Some("div")));
        assert!(closing_tag_omitted("p", Some("p")));
        assert!(closing_tag_omitted("p", Some("ul")));
        // ...but not at an inline sibling.
        assert!(!closing_tag_omitted("p", Some("span")));

        // Table family: thead/tbody/tfoot and the tr/td/th triggers.
        assert!(closing_tag_omitted("thead", Some("tbody")));
        assert!(closing_tag_omitted("tfoot", Some("tbody")));
        assert!(!closing_tag_omitted("tfoot", Some("thead")));
        assert!(closing_tag_omitted("td", Some("th")));
        assert!(closing_tag_omitted("th", Some("tr")));
        assert!(closing_tag_omitted("tr", Some("tr")));

        // dt/dd and rt/rp reciprocal pairs; option/optgroup.
        assert!(closing_tag_omitted("dt", Some("dd")));
        assert!(closing_tag_omitted("dd", Some("dt")));
        assert!(closing_tag_omitted("rt", Some("rp")));
        assert!(closing_tag_omitted("option", Some("optgroup")));
        assert!(closing_tag_omitted("optgroup", Some("optgroup")));
        assert!(!closing_tag_omitted("optgroup", Some("option")));

        // Elements outside the table never auto-close.
        assert!(!closing_tag_omitted("div", Some("div")));
        assert!(!closing_tag_omitted("div", None));
        assert!(!closing_tag_omitted("span", Some("span")));
    }
}
