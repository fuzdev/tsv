//! Internal renderer width configuration.
//!
//! Production callers go through the public renderer entry points
//! ([`super::arena_render::arena_print_doc`] etc.), which always use
//! [`RenderConfig::default()`] — i.e. [`crate::PRINT_WIDTH`] and
//! [`crate::INDENT`]. These values are hardcoded for the formatter and
//! never overridden by users.
//!
//! This struct exists so the doc-builder unit tests can exercise the
//! algorithm with smaller widths (e.g. `print_width: 10`) without bloating
//! test inputs. It is `pub(crate)` and intentionally not part of the public
//! API.

use crate::{INDENT, PRINT_WIDTH};

/// What a render's output is *for*, which decides whether reaching a comment's doc node
/// counts as that comment being emitted.
///
/// The comment ledger records an emit when the render loop reaches a tagged node, on the
/// premise that anything reaching the loop is being written to the document (a losing
/// `conditional_group` candidate and a `fits()` lookahead never get there). A **measurement**
/// render breaks that premise: it drives the same loop over the same nodes, but its output is
/// thrown away after being measured. Left unmarked it double-counts every comment in the
/// measured subtree — the comment is printed once and the ledger reports two, a false
/// DOUBLE-PRINTED that costs the audit its accuracy.
///
/// Carried on [`RenderConfig`] rather than passed alongside it because the config is already
/// threaded to every sub-render (fill segments, the line-suffix flush), so a measurement
/// render's nested renders inherit the purpose instead of each needing to re-derive it.
///
/// `cfg`-gated on `comment_check` along with the field, so a production `RenderConfig` is
/// byte-identical to one built before this existed: like `swallow_check`, a diagnostic
/// feature compiles out *entirely* rather than leaving dead state in the type the production
/// renderer threads.
#[cfg(feature = "comment_check")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RenderPurpose {
    /// The rendered text becomes part of the formatted document — reaching a comment's node
    /// *is* the emit.
    Output,
    /// The rendered text is measured and discarded (e.g. a flat width probe). Records nothing.
    Measure,
}

#[cfg(feature = "comment_check")]
impl RenderPurpose {
    /// Whether reaching a comment's doc node under this purpose counts as an emit.
    #[inline]
    pub(crate) fn records_comment_emits(self) -> bool {
        matches!(self, Self::Output)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RenderConfig {
    pub print_width: usize,
    pub indent: &'static str,
    /// Read only by the `comment_check` ledger — see [`RenderPurpose`]. Absent without the
    /// feature, so production builds carry no diagnostic state.
    #[cfg(feature = "comment_check")]
    pub purpose: RenderPurpose,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            print_width: PRINT_WIDTH,
            indent: INDENT,
            #[cfg(feature = "comment_check")]
            purpose: RenderPurpose::Output,
        }
    }
}
