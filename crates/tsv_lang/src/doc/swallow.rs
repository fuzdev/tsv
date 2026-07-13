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
//! Coverage is every render that appends to the output buffer: the doc
//! renderer's main loop (the *raw* comment-emit path —
//! `build_inline_comments_between_doc` and friends) **and** its sub-renders (fill
//! segments, the line-suffix flush). A swallow is a property of the physical
//! output *line*, and a sub-render appends to the same line, so all of them drive
//! one per-thread state machine (the `PENDING` thread-local below), reached through
//! one [`SwallowTracker`] handle each. A `line_suffix` comment is not exempt: two of
//! them flushed at the same line break land back-to-back on one line
//! (`x; // c1 // c2`), and the first `//` swallows the second. Comments written
//! straight to the output buffer (the Svelte template buffer path) bypass the doc
//! renderer entirely and are out of scope here.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static REPORTS: RefCell<Vec<SwallowReport>> = const { RefCell::new(Vec::new()) };
    /// The line comment emitted on the current output line, until a newline clears
    /// it. Thread-local rather than a [`SwallowTracker`] field because the main
    /// render loop and its sub-renders (fill segments, the line-suffix flush) all
    /// append to the same physical line and must share one state machine.
    static PENDING: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Record a detected swallow (module-internal; called by [`SwallowTracker`]).
fn record(comment: &str, following: &str, line_context: &str) {
    REPORTS.with(|r| {
        r.borrow_mut().push(SwallowReport {
            comment: comment.to_string(),
            following: following.to_string(),
            line_context: line_context.to_string(),
        });
    });
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

/// Take and clear the swallow reports accumulated on this thread.
pub fn take_swallow_reports() -> Vec<SwallowReport> {
    REPORTS.with(|r| std::mem::take(&mut *r.borrow_mut()))
}

/// The per-render handle onto the thread-local swallow state machine.
///
/// A swallow is a property of the physical output *line*, and the main render loop
/// and its sub-renders (fill segments, the line-suffix flush) all append to the same
/// line — so they share one [`PENDING`] state machine rather than tracking
/// separately. This type is the single access mechanism to it: every renderer holds
/// one, and the two constructors are the only difference between them.
///
/// It snapshots the global flag once, so the render loop can gate the (otherwise
/// wasted) text resolution on [`Self::enabled`]. Inert when the check is disabled.
pub(crate) struct SwallowTracker {
    enabled: bool,
}

impl SwallowTracker {
    /// Begin a **top-level** render: snapshot the flag and clear any pending comment
    /// left behind by a previous render (each starts a fresh output buffer).
    pub(crate) fn begin_render() -> Self {
        let enabled = swallow_check_enabled();
        if enabled {
            PENDING.with(|p| *p.borrow_mut() = None);
        }
        Self { enabled }
    }

    /// Join the enclosing render from a **sub-render** (fill segment, line-suffix
    /// flush). Deliberately does *not* clear `PENDING`: the sub-render continues the
    /// same physical output line, so a comment pending from the main loop must stay
    /// pending — clearing it here is exactly how the line-suffix flush used to hide
    /// `x; // c1 // c2`.
    pub(crate) fn join_render() -> Self {
        Self {
            enabled: swallow_check_enabled(),
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
        PENDING.with(|p| {
            let mut pending = p.borrow_mut();
            if let Some(comment) = pending.take() {
                if text.is_empty() {
                    *pending = Some(comment);
                } else {
                    let ctx = output.rsplit('\n').next().unwrap_or(output);
                    record(&comment, text, ctx);
                }
            }
            if is_line_comment {
                *pending = Some(text.to_string());
            }
        });
    }

    /// Observe a line break: a real newline (`emitted`) ends the comment's line.
    #[inline]
    pub(crate) fn on_newline(&mut self, emitted: bool) {
        if emitted {
            PENDING.with(|p| *p.borrow_mut() = None);
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
            let doc = d.concat(&[d.text("["), d.line_comment_text_pooled("// c"), d.text("]")]);
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
                d.line_comment_text_pooled("// c"),
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

    #[test]
    fn two_line_suffix_comments_flushed_together_swallow() {
        // Two trailing comments deferred to the same line break flush back-to-back
        // onto one line — the first `//` swallows the second. The flush is a
        // sub-render, so this only surfaces because it shares the main loop's state
        // machine. A doc that reaches this shape is a printer bug; the check is what
        // makes it visible instead of silently losing `// c2`.
        let (out, reports) = with_check(|d| {
            let doc = d.concat(&[
                d.text("x;"),
                d.line_suffix(d.line_comment_text_pooled(" // c1")),
                d.line_suffix(d.line_comment_text_pooled(" // c2")),
                d.hardline(),
            ]);
            arena_print_doc(d, doc, &EmbedContext::default())
        });
        assert_eq!(
            out, "x; // c1 // c2\n",
            "expected both suffixes flushed onto one line, in queue order"
        );
        assert_eq!(reports.len(), 1, "expected one swallow, got {reports:?}");
        assert_eq!(reports[0].comment, " // c1");
        assert_eq!(reports[0].following, " // c2");
    }

    #[test]
    fn line_suffixes_flush_in_queue_order() {
        // The flush is FIFO, matching prettier: its `lineSuffix.reverse()` only
        // cancels the LIFO pop of its command stack. Reversing here would silently
        // reorder two trailing comments queued on one line.
        let (out, _reports) = with_check(|d| {
            let doc = d.concat(&[
                d.text("x;"),
                d.line_suffix(d.text(" /* c1 */")),
                d.line_suffix(d.text(" /* c2 */")),
                d.hardline(),
            ]);
            arena_print_doc(d, doc, &EmbedContext::default())
        });
        assert_eq!(
            out, "x; /* c1 */ /* c2 */\n",
            "suffixes must flush in order"
        );
    }

    #[test]
    fn lone_line_suffix_comment_is_safe() {
        let (_out, reports) = with_check(|d| {
            let doc = d.concat(&[
                d.text("x;"),
                d.line_suffix(d.line_comment_text_pooled(" // c")),
                d.hardline(),
            ]);
            arena_print_doc(d, doc, &EmbedContext::default())
        });
        assert!(
            reports.is_empty(),
            "a single trailing comment ends its line — no swallow"
        );
    }
}
