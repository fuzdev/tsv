//! Fragment analysis and child printing helpers

use super::Printer;
use crate::ast::internal::{Fragment, FragmentNode};

impl<'a> Printer<'a> {
    /// Check if a fragment's content is inline (huggable at both ends).
    ///
    /// Returns true if there's no leading or trailing whitespace around content,
    /// allowing content to be hugged to control flow tags like `{#if cond}<Comp/>{/if}`.
    ///
    /// This differs from checking for newlines because we want to hug content
    /// even if the content itself is multiline (e.g., component with wrapping attrs).
    pub(super) fn is_inline_fragment(&self, fragment: &Fragment<'_>) -> bool {
        // Inline if no leading AND no trailing whitespace
        !self.fragment_has_leading_ws(fragment) && !self.fragment_has_trailing_ws(fragment)
    }

    /// Whether the fragment's boundary whitespace **run** (the leading run of the first
    /// text node / trailing run of the last) contains a newline — a **newline-authored**
    /// boundary, which keeps its layout meaning (the construct stays multiline).
    ///
    /// A space-only run does NOT count: it is render-free (the compiler trims every
    /// fragment edge at compile — `clean_nodes`), so it neither survives inline nor
    /// selects the layout. Interior newlines don't count either — they are fill
    /// separators, not boundary authoring (`{#if c}x\ny{/if}` fills; only the boundary
    /// run speaks for the boundary). ASCII whitespace only: an NBSP is content.
    /// See conformance_prettier.md §Svelte: Blocks.
    pub(super) fn fragment_boundary_newline(
        &self,
        fragment: &Fragment<'_>,
        is_leading: bool,
    ) -> bool {
        let node = if is_leading {
            fragment.nodes.first()
        } else {
            fragment.nodes.last()
        };
        let Some(FragmentNode::Text(text)) = node else {
            return false;
        };
        let raw = text.raw(self.source);
        let run = if is_leading {
            &raw[..raw.len()
                - raw
                    .trim_start_matches(|c: char| c.is_ascii_whitespace())
                    .len()]
        } else {
            &raw[raw
                .trim_end_matches(|c: char| c.is_ascii_whitespace())
                .len()..]
        };
        run.contains('\n')
    }

    /// Check if a fragment has leading whitespace that triggers a line break.
    pub(super) fn fragment_has_leading_ws(&self, fragment: &Fragment<'_>) -> bool {
        self.fragment_has_boundary_ws(fragment, true)
    }

    /// Check if a fragment has trailing whitespace that triggers a line break.
    pub(super) fn fragment_has_trailing_ws(&self, fragment: &Fragment<'_>) -> bool {
        self.fragment_has_boundary_ws(fragment, false)
    }

    /// Check if a fragment has boundary whitespace that triggers a line break.
    ///
    /// `is_leading`: true for first node, false for last node
    ///
    /// Returns true if:
    /// 1. Boundary character is whitespace AND (is newline OR text/fragment has newlines)
    /// 2. Boundary character is NOT whitespace → false (even if text contains newlines elsewhere)
    ///
    /// This prevents text like `,\n\tupdated` from being treated as having boundary whitespace
    /// when the boundary char `,` is not whitespace — important for inline runs like
    /// `{expr}{#if cond}, updated {expr2}{/if}` where the IfBlock body starts with `,`.
    fn fragment_has_boundary_ws(&self, fragment: &Fragment<'_>, is_leading: bool) -> bool {
        if fragment.nodes.is_empty() {
            return false;
        }
        let node = if is_leading {
            fragment.nodes.first()
        } else {
            fragment.nodes.last()
        };
        let Some(FragmentNode::Text(text)) = node else {
            return false;
        };

        // Check the actual boundary character
        let raw = text.raw(self.source);
        let boundary_char = if is_leading {
            raw.chars().next()
        } else {
            raw.chars().last()
        };
        let Some(ch) = boundary_char else {
            return false;
        };

        // Boundary char must be collapsible (ASCII) whitespace to trigger boundary ws;
        // a non-breaking space (U+00A0) is content, not a boundary (so `{#if a} {/if}`
        // with an NBSP body stays inline, matching prettier).
        if !ch.is_ascii_whitespace() {
            return false;
        }

        // Boundary whitespace is a newline → yes
        if ch == '\n' {
            return true;
        }

        // Boundary whitespace is space → only if there are newlines in fragment
        self.fragment_has_any_newlines(fragment)
    }

    /// Check if any text node in the fragment contains a newline.
    fn fragment_has_any_newlines(&self, fragment: &Fragment<'_>) -> bool {
        fragment
            .nodes
            .iter()
            .any(|n| matches!(n, FragmentNode::Text(t) if t.has_newline()))
    }
}
