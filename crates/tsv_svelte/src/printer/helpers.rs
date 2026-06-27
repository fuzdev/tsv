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
        let source = self.source;
        fragment.nodes.iter().any(|n| {
            if let FragmentNode::Text(t) = n {
                t.raw(source).contains('\n')
            } else {
                false
            }
        })
    }

    /// Check if a fragment has leading whitespace of any kind (space or newline).
    pub(super) fn fragment_has_any_leading_ws(&self, fragment: &Fragment<'_>) -> bool {
        self.fragment_has_any_boundary_ws(fragment, true)
    }

    /// Check if a fragment has trailing whitespace of any kind (space or newline).
    pub(super) fn fragment_has_any_trailing_ws(&self, fragment: &Fragment<'_>) -> bool {
        self.fragment_has_any_boundary_ws(fragment, false)
    }

    /// Check if a fragment has any whitespace at a boundary (leading or trailing).
    fn fragment_has_any_boundary_ws(&self, fragment: &Fragment<'_>, is_leading: bool) -> bool {
        let node = if is_leading {
            fragment.nodes.first()
        } else {
            fragment.nodes.last()
        };
        let source = self.source;
        node.is_some_and(|n| {
            if let FragmentNode::Text(t) = n {
                let ch = if is_leading {
                    t.raw(source).chars().next()
                } else {
                    t.raw(source).chars().last()
                };
                ch.is_some_and(|c: char| c.is_ascii_whitespace())
            } else {
                false
            }
        })
    }

    /// Check if a fragment has space-only whitespace (no newlines) at boundaries.
    ///
    /// Returns true if the fragment has leading OR trailing whitespace that is
    /// space-only (no newlines). This triggers expansion to multiline for patterns
    /// like `{#if a} content {/if}`.
    ///
    /// Note: Prettier has a quirk where the last block in a file doesn't expand.
    /// We consistently expand all such blocks regardless of position.
    pub(super) fn fragment_has_space_only_ws(&self, fragment: &Fragment<'_>) -> bool {
        // If there are newlines, the existing ws detection handles expansion
        if self.fragment_has_any_newlines(fragment) {
            return false;
        }

        // Expand if there's any leading OR trailing space (not newline)
        self.fragment_has_any_leading_ws(fragment) || self.fragment_has_any_trailing_ws(fragment)
    }

    /// Get leading/trailing whitespace status for a fragment, considering context.
    ///
    /// When `in_multiline_context` is true, or the fragment has space-only whitespace,
    /// even simple spaces (not just newlines) trigger line breaks.
    ///
    /// Returns `(has_leading, has_trailing)`.
    pub(super) fn fragment_ws_status(
        &self,
        fragment: &Fragment<'_>,
        in_multiline_context: bool,
    ) -> (bool, bool) {
        let has_space_only = self.fragment_has_space_only_ws(fragment);
        let use_any_ws = in_multiline_context || has_space_only;

        let has_leading = if use_any_ws {
            self.fragment_has_any_leading_ws(fragment)
        } else {
            self.fragment_has_leading_ws(fragment)
        };
        let has_trailing = if use_any_ws {
            self.fragment_has_any_trailing_ws(fragment)
        } else {
            self.fragment_has_trailing_ws(fragment)
        };

        (has_leading, has_trailing)
    }
}
