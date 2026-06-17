//! Diagnostic: detect line comments that swallow the following token.
//!
//! A `//` line comment runs to end-of-line, so any non-newline content the
//! renderer emits on the *same physical line* after a line comment is swallowed
//! — silent content loss that makes output non-idempotent (the historical bug
//! class behind many printer fixes). This module is an opt-in render-time guard:
//! when enabled it records every such swallow. It changes no output and is
//! zero-cost when disabled (a single relaxed atomic load gates all
//! instrumentation; the builder only records line-comment ids when enabled).
//!
//! Coverage is the doc renderer's main loop, which is exactly the *raw*
//! comment-emit path (`build_inline_comments_between_doc` and friends — the
//! swallow-prone sites). Comments routed through `line_suffix` are flushed
//! immediately before a newline by the render model and cannot swallow, so they
//! are intentionally not flagged. Comments written straight to the output buffer
//! (the Svelte template buffer path) bypass the doc renderer entirely and are
//! out of scope here.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static REPORTS: RefCell<Vec<SwallowReport>> = const { RefCell::new(Vec::new()) };
}

/// A detected swallow: a line `comment` immediately followed, on the same
/// physical output line, by `following` content that the `//` consumes.
#[derive(Debug, Clone)]
pub struct SwallowReport {
    /// The line comment text (with its `//` prefix) that swallows.
    pub comment: String,
    /// The content emitted on the same line after the comment (swallowed token).
    pub following: String,
    /// The output line up to the comment, for context.
    pub line_context: String,
}

/// Enable or disable the render-time swallow check (process-global). Off by
/// default. Set this *before* building the doc tree — the builder records
/// line-comment ids only while enabled.
pub fn set_swallow_check(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

/// Whether the swallow check is enabled.
#[inline]
pub fn swallow_check_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Record a detected swallow (module-internal; called by `SwallowTracker`).
fn record(comment: &str, following: &str, line_context: &str) {
    REPORTS.with(|r| {
        r.borrow_mut().push(SwallowReport {
            comment: comment.to_string(),
            following: following.to_string(),
            line_context: line_context.to_string(),
        });
    });
}

/// Take and clear the swallow reports accumulated on this thread.
pub fn take_swallow_reports() -> Vec<SwallowReport> {
    REPORTS.with(|r| std::mem::take(&mut *r.borrow_mut()))
}

/// Render-time state machine that flags a line comment swallowing the content
/// emitted after it on the same physical line. Owns the "pending line comment"
/// state so the renderer's loop just calls [`Self::on_text`] / [`Self::on_newline`]
/// instead of inlining the detection. Inert (and allocation-free) when the check
/// is disabled — construct once per render via [`Self::new`].
pub(crate) struct SwallowTracker {
    enabled: bool,
    /// The text of a line comment just emitted, until a newline clears it.
    pending: Option<String>,
}

impl SwallowTracker {
    /// Snapshot the global flag once for the whole render.
    pub(crate) fn new() -> Self {
        Self {
            enabled: swallow_check_enabled(),
            pending: None,
        }
    }

    /// Whether the check is on — gate the (otherwise wasted) text resolution at
    /// the call site on this.
    #[inline]
    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }

    /// Observe a text node about to be emitted. `is_line_comment` marks it as a
    /// `//` comment, `text` is its resolved content, `output` is the buffer so far
    /// (for the line-context snippet). Records a swallow when a pending comment is
    /// followed by non-newline content on the same line; an empty emit keeps the
    /// `//` dangling.
    pub(crate) fn on_text(&mut self, is_line_comment: bool, text: &str, output: &str) {
        if let Some(comment) = self.pending.take() {
            if text.is_empty() {
                self.pending = Some(comment);
            } else {
                let ctx = output.rsplit('\n').next().unwrap_or(output);
                record(&comment, text, ctx);
            }
        }
        if is_line_comment {
            self.pending = Some(text.to_string());
        }
    }

    /// Observe a line break: a real newline (`emitted`) ends the comment's line.
    #[inline]
    pub(crate) fn on_newline(&mut self, emitted: bool) {
        if emitted {
            self.pending = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EmbedContext;
    use crate::doc::arena::DocArena;
    use crate::doc::arena_print_doc;

    // The check is gated by a process-global flag; serialize the toggling so a
    // parallel test doesn't observe a half-set state. (Reports are thread-local,
    // so only the flag needs guarding.)
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_check<R>(f: impl FnOnce(&DocArena) -> R) -> (R, Vec<SwallowReport>) {
        let _guard = LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        set_swallow_check(true);
        let _ = take_swallow_reports(); // clear stragglers on this thread
        let arena = DocArena::new();
        let r = f(&arena);
        let reports = take_swallow_reports();
        set_swallow_check(false);
        (r, reports)
    }

    #[test]
    fn line_comment_followed_by_text_on_same_line_is_a_swallow() {
        let (_out, reports) = with_check(|d| {
            // `[ // c ]` collapsed flat: the `]` lands on the comment's line.
            let doc = d.concat(&[
                d.text("["),
                d.line_comment_text_owned("// c".to_string()),
                d.text("]"),
            ]);
            arena_print_doc(d, doc, &EmbedContext::default())
        });
        assert_eq!(reports.len(), 1, "expected one swallow");
        assert_eq!(reports[0].comment, "// c");
        assert_eq!(reports[0].following, "]");
    }

    #[test]
    fn line_comment_followed_by_hardline_is_safe() {
        let (_out, reports) = with_check(|d| {
            let doc = d.concat(&[
                d.text("["),
                d.line_comment_text_owned("// c".to_string()),
                d.hardline(),
                d.text("]"),
            ]);
            arena_print_doc(d, doc, &EmbedContext::default())
        });
        assert!(
            reports.is_empty(),
            "hardline after comment must not swallow"
        );
    }

    #[test]
    fn block_comment_text_is_not_flagged() {
        let (_out, reports) = with_check(|d| {
            // A plain text node (block comment shape) is not tagged → no swallow.
            let doc = d.concat(&[d.text("/* c */"), d.text("]")]);
            arena_print_doc(d, doc, &EmbedContext::default())
        });
        assert!(reports.is_empty());
    }
}
