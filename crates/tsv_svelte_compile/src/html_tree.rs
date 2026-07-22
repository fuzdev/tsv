//! HTML content-model validation — a faithful port of the oracle's
//! `src/html-tree-validation.js`.
//!
//! The rules answer one question: would a browser **repair** this markup by
//! moving, removing, or inserting elements? Svelte errors on such a component
//! (`node_invalid_placement`) because the repaired DOM breaks its assumptions
//! about component structure, so tsv must refuse it rather than emit output for
//! input the oracle rejects.
//!
//! # Why a port and not a spec implementation
//!
//! The oracle's tables are deliberately *narrower* than the HTML spec's content
//! model: its own comment says "there are more elements that are invalid inside
//! other elements, but they're not repaired and so don't break SSR and are
//! therefore not listed here". Implementing the spec would over-refuse. The
//! tables below are transcribed entry for entry.
//!
//! # The one transcription subtlety
//!
//! The oracle builds `disallowed_children` as `{ ...autoclosing_children, … }`,
//! and four keys — `tr` / `tbody` / `thead` / `tfoot` — appear in **both**. A JS
//! object spread **replaces** the whole value, so those four keep only their
//! `only` list and **lose** the `direct` list `autoclosing_children` gave them.
//! Merging the two (the natural reading of "spread then extend") would refuse
//! `<tbody><tfoot>` shapes the oracle accepts. Only the ten entries not
//! re-keyed below — `li`, `dt`, `dd`, `p`, `rt`, `rp`, `optgroup`, `option`,
//! `td`, `th` — survive from the autoclosing map.

/// What a parent/ancestor tag disallows. Mirrors the oracle's three value
/// shapes; a tag carries exactly one of them.
enum Disallowed {
    /// Illegal as a **direct child** only.
    Direct(&'static [&'static str]),
    /// Illegal anywhere below, until a `reset_by` tag re-opens them.
    Descendant {
        names: &'static [&'static str],
        reset_by: &'static [&'static str],
    },
    /// An allow-list: every child not named here is illegal.
    Only(&'static [&'static str]),
}

const HEADINGS: &[&str] = &["h1", "h2", "h3", "h4", "h5", "h6"];
const TABLE_SECTION_ONLY: &[&str] = &["tr", "style", "script", "template"];

/// The oracle's `disallowed_children`, after the spread described in the module
/// docs has been applied.
fn disallowed_children(tag: &str) -> Option<Disallowed> {
    Some(match tag {
        // --- surviving `autoclosing_children` entries ---
        "li" => Disallowed::Direct(&["li"]),
        "dt" | "dd" => Disallowed::Descendant {
            names: &["dt", "dd"],
            reset_by: &["dl"],
        },
        "p" => Disallowed::Descendant {
            names: &[
                "address",
                "article",
                "aside",
                "blockquote",
                "div",
                "dl",
                "fieldset",
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
                "main",
                "menu",
                "nav",
                "ol",
                "p",
                "pre",
                "section",
                "table",
                "ul",
            ],
            reset_by: &[],
        },
        "rt" | "rp" => Disallowed::Descendant {
            names: &["rt", "rp"],
            reset_by: &[],
        },
        "optgroup" => Disallowed::Descendant {
            names: &["optgroup"],
            reset_by: &[],
        },
        "option" => Disallowed::Descendant {
            names: &["option", "optgroup"],
            reset_by: &[],
        },
        "td" | "th" => Disallowed::Direct(&["td", "th", "tr"]),
        // --- entries added by `disallowed_children` itself ---
        "form" => Disallowed::Descendant {
            names: &["form"],
            reset_by: &[],
        },
        "a" => Disallowed::Descendant {
            names: &["a"],
            reset_by: &[],
        },
        "button" => Disallowed::Descendant {
            names: &["button"],
            reset_by: &[],
        },
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => Disallowed::Descendant {
            names: HEADINGS,
            reset_by: &[],
        },
        // ⚠️ These four REPLACE their autoclosing entry — see the module docs.
        "tr" => Disallowed::Only(&["th", "td", "style", "script", "template"]),
        "tbody" | "thead" | "tfoot" => Disallowed::Only(TABLE_SECTION_ONLY),
        "colgroup" => Disallowed::Only(&["col", "template"]),
        "table" => Disallowed::Only(&[
            "caption", "colgroup", "tbody", "thead", "tfoot", "style", "script", "template",
        ]),
        "head" => Disallowed::Only(&[
            "base", "basefont", "bgsound", "link", "meta", "title", "noscript", "noframes",
            "style", "script", "template",
        ]),
        "html" => Disallowed::Only(&["head", "body", "frameset"]),
        "frameset" => Disallowed::Only(&["frame"]),
        _ => return None,
    })
}

/// A custom element may contain anything, and may go anywhere — the oracle's
/// `tag.includes('-')` short-circuit, which is also what makes `<foo-bar>` reset
/// the `dt`/`dd` descendant rules in its own sample.
fn is_custom_element(tag: &str) -> bool {
    tag.contains('-')
}

/// The oracle's `is_tag_valid_with_ancestor` — the grandparent-and-above test.
///
/// Only the `descendant` shape participates; `direct` and `only` are
/// parent-relative and are checked by [`is_tag_valid_with_parent`] alone.
///
/// `ancestors` starts at the **parent** and ends at the ancestor under test, so
/// it always holds at least two entries.
pub(crate) fn is_tag_valid_with_ancestor(child_tag: &str, ancestors: &[&str]) -> Option<String> {
    if is_custom_element(child_tag) {
        return None;
    }

    let ancestor_tag = *ancestors.last()?;
    let Disallowed::Descendant { names, reset_by } = disallowed_children(ancestor_tag)? else {
        return None;
    };

    // A reset means the forbidden descendants are allowed again. The scan covers
    // everything strictly below the ancestor, parent included.
    //
    // ⚠️ The oracle gates this whole loop on `reset_by` being PRESENT, so it runs
    // for `dt`/`dd` and for nothing else. The custom-element short-circuit lives
    // INSIDE it and is therefore gated too: an intervening `<foo-bar>` resets a
    // `dt`/`dd` chain but does NOT rescue a `<p>` descendant, whose entry carries
    // no `reset_by`. Hoisting the check out of the guard silently under-refuses.
    if !reset_by.is_empty() {
        for ancestor in ancestors[..ancestors.len() - 1].iter().rev() {
            if is_custom_element(ancestor) || reset_by.contains(ancestor) {
                return None;
            }
        }
    }

    names
        .contains(&child_tag)
        .then(|| format!("`<{child_tag}>` cannot be a descendant of `<{ancestor_tag}>`"))
}

/// The oracle's `is_tag_valid_with_parent` — the direct-child test.
pub(crate) fn is_tag_valid_with_parent(child_tag: &str, parent_tag: &str) -> Option<String> {
    if is_custom_element(child_tag) || is_custom_element(parent_tag) {
        return None;
    }

    // No error or warning is thrown for the immediate children of a `<template>`.
    if parent_tag == "template" {
        return None;
    }

    if let Some(disallowed) = disallowed_children(parent_tag) {
        match disallowed {
            Disallowed::Direct(names) if names.contains(&child_tag) => {
                return Some(format!(
                    "`<{child_tag}>` cannot be a direct child of `<{parent_tag}>`"
                ));
            }
            Disallowed::Descendant { names, .. } if names.contains(&child_tag) => {
                return Some(format!(
                    "`<{child_tag}>` cannot be a child of `<{parent_tag}>`"
                ));
            }
            Disallowed::Only(names) => {
                if names.contains(&child_tag) {
                    return None;
                }
                let allowed = names
                    .iter()
                    .map(|d| format!("`<{d}>`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                return Some(format!(
                    "`<{child_tag}>` cannot be a child of `<{parent_tag}>`. \
                     `<{parent_tag}>` only allows these children: {allowed}"
                ));
            }
            _ => {}
        }
    }

    // These tags are valid only under a few parents with special child-parsing
    // rules. Reaching here means none of those matched, and the caller only calls
    // this when the parent IS known, so every remaining case is invalid.
    match child_tag {
        "body" | "caption" | "col" | "colgroup" | "frameset" | "frame" | "head" | "html" => Some(
            format!("`<{child_tag}>` cannot be a child of `<{parent_tag}>`"),
        ),
        "thead" | "tbody" | "tfoot" => Some(format!(
            "`<{child_tag}>` must be the child of a `<table>`, not a `<{parent_tag}>`"
        )),
        "td" | "th" => Some(format!(
            "`<{child_tag}>` must be the child of a `<tr>`, not a `<{parent_tag}>`"
        )),
        "tr" => Some(format!(
            "`<tr>` must be the child of a `<thead>`, `<tbody>`, or `<tfoot>`, not a `<{parent_tag}>`"
        )),
        _ => None,
    }
}
