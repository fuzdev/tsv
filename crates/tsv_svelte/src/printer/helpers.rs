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
    pub(super) fn is_inline_fragment(&self, fragment: &Fragment) -> bool {
        // Inline if no leading AND no trailing whitespace
        !self.fragment_has_leading_ws(fragment) && !self.fragment_has_trailing_ws(fragment)
    }

    /// Check if a fragment has leading whitespace that triggers a line break.
    pub(super) fn fragment_has_leading_ws(&self, fragment: &Fragment) -> bool {
        self.fragment_has_boundary_ws(fragment, true)
    }

    /// Check if a fragment has trailing whitespace that triggers a line break.
    pub(super) fn fragment_has_trailing_ws(&self, fragment: &Fragment) -> bool {
        self.fragment_has_boundary_ws(fragment, false)
    }

    /// Check if a fragment has boundary whitespace that triggers a line break.
    ///
    /// `is_leading`: true for first node, false for last node
    ///
    /// Returns true if:
    /// 1. Boundary node is text containing a newline, OR
    /// 2. Boundary node is text with whitespace AND any text node has a newline
    fn fragment_has_boundary_ws(&self, fragment: &Fragment, is_leading: bool) -> bool {
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

        // Direct newline in boundary node
        if text.raw.contains('\n') {
            return true;
        }

        // Boundary space + newlines elsewhere → treat as boundary newline
        if text.raw.chars().any(char::is_whitespace) {
            return self.fragment_has_any_newlines(fragment);
        }

        false
    }

    /// Check if any text node in the fragment contains a newline.
    fn fragment_has_any_newlines(&self, fragment: &Fragment) -> bool {
        fragment.nodes.iter().any(|n| {
            if let FragmentNode::Text(t) = n {
                t.raw.contains('\n')
            } else {
                false
            }
        })
    }

    /// Check if a fragment has leading whitespace of any kind (space or newline).
    pub(super) fn fragment_has_any_leading_ws(&self, fragment: &Fragment) -> bool {
        self.fragment_has_any_boundary_ws(fragment, true)
    }

    /// Check if a fragment has trailing whitespace of any kind (space or newline).
    pub(super) fn fragment_has_any_trailing_ws(&self, fragment: &Fragment) -> bool {
        self.fragment_has_any_boundary_ws(fragment, false)
    }

    /// Check if a fragment has any whitespace at a boundary (leading or trailing).
    fn fragment_has_any_boundary_ws(&self, fragment: &Fragment, is_leading: bool) -> bool {
        let node = if is_leading {
            fragment.nodes.first()
        } else {
            fragment.nodes.last()
        };
        node.is_some_and(|n| {
            if let FragmentNode::Text(t) = n {
                let ch = if is_leading {
                    t.raw.chars().next()
                } else {
                    t.raw.chars().last()
                };
                ch.is_some_and(char::is_whitespace)
            } else {
                false
            }
        })
    }

    /// Check if a fragment has space-only whitespace (no newlines) that should trigger expansion.
    ///
    /// Patterns like `{#if a} content {/if}` or `{#if a} content{/if}` should expand to multiline,
    /// matching prettier's typical behavior. Only fully hugged content stays inline.
    pub(super) fn fragment_has_space_only_ws(&self, fragment: &Fragment) -> bool {
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
        fragment: &Fragment,
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
