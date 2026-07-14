//! Diagnostic: the print-once comment ledger.
//!
//! Comments are **detached** — a flat `Vec<Comment>` on the language root, located
//! positionally at print time (see the root `CLAUDE.md` §Comment Handling). Nothing in
//! that model forces a comment to actually be printed: a gap emitter that never runs, an
//! owned comment whose node reassembles instead of routing through the ownership seam, a
//! builder handed `&[]` for its comment slice — each silently *loses* the comment. The
//! inverse (one comment printed twice) is just as invisible.
//!
//! This module is the structural guard: an opt-in, per-format ledger of the comments a
//! document **parsed** against the comments its printers **emitted**, asserting the two
//! sets are equal. It is prettier's `ensureAllCommentsPrinted` (its `printedComments`
//! set, checked once the doc is built) in tsv's shape.
//!
//! It lives behind the **`comment_check` cargo feature** (off by default, like
//! [`crate::doc::swallow`]), so production builds — and default `tsv_debug` builds, whose
//! profiles must measure production-shaped code — compile it out entirely. Output is
//! byte-identical either way; this records, it never decides.
//!
//! ## Model
//!
//! - [`register_parsed`] — a **format entry point** declares the comment list it is about
//!   to print (`tsv_ts::format_in`, `tsv_css`'s `format_css*`, `tsv_svelte`'s
//!   `format_svelte*`). That list is the expectation.
//! - [`record_emitted`] — one comment printed. The **doc-based** printers (`tsv_ts`, and
//!   `tsv_svelte`'s four JS-comment doc builders) don't call this directly: they tag the
//!   comment's doc node ([`crate::doc::arena::DocArena::tag_comment_doc`]) and the
//!   *renderer* records it when it emits that node. That distinction is load-bearing — a
//!   builder may assemble the same subtree into two `conditional_group` candidates of
//!   which one renders, so build-time counting reads as a double-print (and a comment
//!   built only into a *losing* candidate would read as printed while being lost).
//!   `tsv_css`, whose printer writes comments straight to its output buffer
//!   (`print_css_comment`) or joins them to a string (`comment_blocks_in_range`), calls
//!   this at the write itself — the same instant.
//! - [`record_verbatim_range`] — a source range emitted **raw**, carrying whatever
//!   comments it contains straight to the output with no emitter involved: a
//!   `format-ignore` region, a raw at-rule prelude, a glued CSS compound selector, an
//!   unparseable selector. The range counts as one emit for every comment it covers, which
//!   is both why those don't read as dropped *and* why a comment somehow ALSO emitted
//!   normally still reads as double-printed. Keep every range tight — a too-wide carve-out
//!   silently re-opens the hole this exists to close. (Recorded at build, not render: a
//!   verbatim range is only ever built on a path with no layout candidates.)
//! - [`take_comment_ledger`] — finalize: compute the findings, drain, clear.
//!
//! ## Scope
//!
//! Only the **detached** comments a format entry registers are in scope. A comment
//! modeled as an *AST node* — a Svelte `<!-- … -->` (`FragmentNode::Comment`), a CSS
//! in-block `CssBlockChild::Comment` — is carried by the tree, not by the positional
//! model, and cannot be lost the same way; emits of those land in
//! [`CommentLedger::unregistered_emits`] rather than in a finding.
//!
//! TODO: extend the registered set to the AST-node comments (Svelte's
//! `FragmentNode::Comment`, CSS's `CssBlockChild::Comment`) so `unregistered_emits`
//! collapses to a genuine registration-gap signal instead of a mixed count. A CSS
//! declaration's *value* comments are a separate case — the parser never lexes them as
//! `Comment`s at all (they are re-derived from source), so they are outside the model by
//! construction, not merely unregistered.
//!
//! State is keyed on the **source text's identity** (address + length), not a push/pop
//! scope. A Svelte host and its embedded `<script>`/`<style>` islands all carry
//! host-absolute spans over one source, so they share a key and merge into one ledger; a
//! nested `<style>` *element* inside the template is re-parsed against its own extracted
//! content string (island-relative spans) and gets its own key, so its offsets can't
//! collide with the host's. Keys are valid only while the sources are alive —
//! [`take_comment_ledger`] drains after each document, so a later file's source reusing
//! an address can't inherit stale entries.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{Comment, Span};

static ENABLED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static DOCS: RefCell<Vec<DocLedger>> = const { RefCell::new(Vec::new()) };
    /// Emits for a span no [`register_parsed`] declared — an AST-node comment (out of
    /// scope, see the module docs) or a registration gap. Counted, never a finding.
    static UNREGISTERED: RefCell<usize> = const { RefCell::new(0) };
}

/// A document's identity: its source text's address + length.
///
/// The doc-node tags the renderer records through carry one of these, because the
/// renderer holds no `source` — see [`crate::doc::arena::DocArena::tag_comment_doc`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentKey(usize, usize);

/// The [`DocumentKey`] for `source`.
#[inline]
#[must_use]
pub fn document_key(source: &str) -> DocumentKey {
    DocumentKey(source.as_ptr() as usize, source.len())
}

/// One source text's ledger.
struct DocLedger {
    key: DocumentKey,
    /// The registered comments, sorted by `(start, end)`.
    entries: Vec<Entry>,
    /// Raw source ranges — every comment a range covers counts as emitted once.
    verbatim: Vec<(u32, u32)>,
}

struct Entry {
    span: Span,
    text: String,
    emitted: usize,
}

/// Why a comment is a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentFindingKind {
    /// Parsed but never emitted — silent content loss.
    Dropped,
    /// Emitted more than once.
    DoublePrinted,
}

/// A registered comment whose emit count is not exactly one.
#[derive(Debug, Clone)]
pub struct CommentFinding {
    pub kind: CommentFindingKind,
    /// The comment's full source text, delimiters included.
    pub text: String,
    /// The comment's span in the document it was parsed from.
    pub span: Span,
    /// How many times it was emitted — `0` for [`CommentFindingKind::Dropped`], `≥2` for
    /// [`CommentFindingKind::DoublePrinted`].
    pub emitted: usize,
}

/// The drained result of one format.
#[derive(Debug, Clone, Default)]
pub struct CommentLedger {
    pub findings: Vec<CommentFinding>,
    /// Comments registered across every document in the format (host + islands).
    pub parsed: usize,
    /// Emits for an unregistered span — see [`UNREGISTERED`].
    pub unregistered_emits: usize,
}

/// Enable or disable the comment ledger (process-global). Off by default. Set it
/// *before* formatting — the format entry points register their comment lists only while
/// enabled.
pub fn set_comment_check(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

/// Whether the comment ledger is enabled.
#[inline]
pub fn comment_check_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Declare the comment list a format entry point is about to print.
///
/// Idempotent per span: a document whose comments arrive from two lists (a Svelte
/// `Root.comments` plus its `<style>`'s stylesheet comments) merges them, and the same
/// list registered twice does not double the expectation — which is what makes the Svelte
/// `<script>` case work, where the island's `Program.comments` and the `Root.comments`
/// clones of them are two `Comment` values over one span.
pub fn register_parsed(source: &str, comments: &[Comment]) {
    if !comment_check_enabled() {
        return;
    }
    DOCS.with(|docs| {
        let mut docs = docs.borrow_mut();
        let doc = doc_for(&mut docs, source);
        for c in comments {
            if let Err(idx) = doc
                .entries
                .binary_search_by_key(&(c.span.start, c.span.end), |e| (e.span.start, e.span.end))
            {
                doc.entries.insert(
                    idx,
                    Entry {
                        span: c.span,
                        text: c.span.extract(source).to_string(),
                        emitted: 0,
                    },
                );
            }
        }
    });
}

/// Declare one comment printed.
pub fn record_emitted(source: &str, span: Span) {
    record_emitted_keyed(document_key(source), span);
}

/// [`record_emitted`] against an already-resolved [`DocumentKey`] — what the doc
/// renderer calls, having no `source` of its own.
pub fn record_emitted_keyed(key: DocumentKey, span: Span) {
    if !comment_check_enabled() {
        return;
    }
    DOCS.with(|docs| {
        let mut docs = docs.borrow_mut();
        let doc = doc_for_key(&mut docs, key);
        match doc
            .entries
            .binary_search_by_key(&(span.start, span.end), |e| (e.span.start, e.span.end))
        {
            Ok(idx) => doc.entries[idx].emitted += 1,
            Err(_) => UNREGISTERED.with(|u| *u.borrow_mut() += 1),
        }
    });
}

/// Declare `[start, end)` of `source` emitted **verbatim** — every comment it covers
/// counts as emitted once.
pub fn record_verbatim_range(source: &str, start: u32, end: u32) {
    if !comment_check_enabled() {
        return;
    }
    DOCS.with(|docs| {
        let mut docs = docs.borrow_mut();
        doc_for(&mut docs, source).verbatim.push((start, end));
    });
}

/// Finalize the format: compute the findings, drain, and clear.
pub fn take_comment_ledger() -> CommentLedger {
    let unregistered_emits = UNREGISTERED.with(|u| std::mem::take(&mut *u.borrow_mut()));
    let docs = DOCS.with(|docs| std::mem::take(&mut *docs.borrow_mut()));

    let mut ledger = CommentLedger {
        unregistered_emits,
        ..CommentLedger::default()
    };
    for doc in &docs {
        ledger.parsed += doc.entries.len();
        for entry in &doc.entries {
            let covered = doc
                .verbatim
                .iter()
                .any(|&(s, e)| entry.span.start >= s && entry.span.end <= e);
            let emitted = entry.emitted + usize::from(covered);
            let kind = match emitted {
                0 => CommentFindingKind::Dropped,
                1 => continue,
                _ => CommentFindingKind::DoublePrinted,
            };
            ledger.findings.push(CommentFinding {
                kind,
                text: entry.text.clone(),
                span: entry.span,
                emitted,
            });
        }
    }
    ledger
}

/// The ledger for `source`, created on first touch.
fn doc_for<'d>(docs: &'d mut Vec<DocLedger>, source: &str) -> &'d mut DocLedger {
    doc_for_key(docs, document_key(source))
}

/// The ledger for `key`, created on first touch.
fn doc_for_key(docs: &mut Vec<DocLedger>, key: DocumentKey) -> &mut DocLedger {
    let idx = if let Some(idx) = docs.iter().position(|d| d.key == key) {
        idx
    } else {
        docs.push(DocLedger {
            key,
            entries: Vec::new(),
            verbatim: Vec::new(),
        });
        docs.len() - 1
    };
    &mut docs[idx]
}

#[cfg(test)]
mod tests {
    use super::*;

    // The check is gated by a process-global flag; serialize the toggling so a parallel
    // test doesn't observe a half-set state. (State is thread-local, so only the flag
    // needs guarding.)
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// A line comment spanning `[start, end)` — the ledger only reads `span`.
    fn line_comment(start: u32, end: u32) -> Comment {
        Comment {
            content_span: Span::new(start + 2, end),
            is_block: false,
            multiline: false,
            span: Span::new(start, end),
            emit_character_field: false,
            bump_pattern_columns: false,
            owned_by_node: false,
        }
    }

    fn with_check<R>(f: impl FnOnce() -> R) -> (R, CommentLedger) {
        let _guard = LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        set_comment_check(true);
        let _ = take_comment_ledger(); // clear stragglers on this thread
        let r = f();
        let ledger = take_comment_ledger();
        set_comment_check(false);
        (r, ledger)
    }

    #[test]
    fn an_emitted_comment_is_clean() {
        let source = "// a\nx;\n";
        let comments = [line_comment(0, 4)];
        let ((), ledger) = with_check(|| {
            register_parsed(source, &comments);
            record_emitted(source, comments[0].span);
        });
        assert!(ledger.findings.is_empty(), "{:?}", ledger.findings);
        assert_eq!(ledger.parsed, 1);
    }

    #[test]
    fn an_unemitted_comment_is_dropped() {
        let source = "// a\nx;\n";
        let comments = [line_comment(0, 4)];
        let ((), ledger) = with_check(|| register_parsed(source, &comments));
        assert_eq!(ledger.findings.len(), 1);
        assert_eq!(ledger.findings[0].kind, CommentFindingKind::Dropped);
        assert_eq!(ledger.findings[0].text, "// a");
    }

    #[test]
    fn a_twice_emitted_comment_is_double_printed() {
        let source = "// a\nx;\n";
        let comments = [line_comment(0, 4)];
        let ((), ledger) = with_check(|| {
            register_parsed(source, &comments);
            record_emitted(source, comments[0].span);
            record_emitted(source, comments[0].span);
        });
        assert_eq!(ledger.findings.len(), 1);
        assert_eq!(ledger.findings[0].kind, CommentFindingKind::DoublePrinted);
        assert_eq!(ledger.findings[0].emitted, 2);
    }

    #[test]
    fn a_comment_in_a_verbatim_range_is_clean() {
        let source = "// a\nx;\n";
        let comments = [line_comment(0, 4)];
        let ((), ledger) = with_check(|| {
            register_parsed(source, &comments);
            record_verbatim_range(source, 0, 7);
        });
        assert!(ledger.findings.is_empty(), "{:?}", ledger.findings);
    }

    #[test]
    fn a_verbatim_comment_also_emitted_is_double_printed() {
        // The raw slice already carries the comment — an emitter running over the same
        // span prints it twice.
        let source = "// a\nx;\n";
        let comments = [line_comment(0, 4)];
        let ((), ledger) = with_check(|| {
            register_parsed(source, &comments);
            record_verbatim_range(source, 0, 7);
            record_emitted(source, comments[0].span);
        });
        assert_eq!(ledger.findings.len(), 1);
        assert_eq!(ledger.findings[0].kind, CommentFindingKind::DoublePrinted);
    }

    #[test]
    fn registering_twice_does_not_double_the_expectation() {
        let source = "// a\nx;\n";
        let comments = [line_comment(0, 4)];
        let ((), ledger) = with_check(|| {
            register_parsed(source, &comments);
            register_parsed(source, &comments);
            record_emitted(source, comments[0].span);
        });
        assert!(ledger.findings.is_empty(), "{:?}", ledger.findings);
        assert_eq!(ledger.parsed, 1);
    }

    #[test]
    fn an_unregistered_emit_is_counted_not_a_finding() {
        // An AST-node comment (Svelte `<!-- -->`, a CSS block child) is out of scope.
        let source = "// a\nx;\n";
        let ((), ledger) = with_check(|| record_emitted(source, Span::new(0, 4)));
        assert!(ledger.findings.is_empty());
        assert_eq!(ledger.unregistered_emits, 1);
    }

    #[test]
    fn two_sources_keep_separate_span_namespaces() {
        // A nested `<style>` element re-parses its content standalone: island-relative
        // spans that would collide with the host's if the ledger were one flat map.
        let host = String::from("// a\nx;\n");
        let island = String::from("// b\ny;\n");
        let host_comments = [line_comment(0, 4)];
        let island_comments = [line_comment(0, 4)];
        let ((), ledger) = with_check(|| {
            register_parsed(&host, &host_comments);
            register_parsed(&island, &island_comments);
            record_emitted(&host, host_comments[0].span);
            record_emitted(&island, island_comments[0].span);
        });
        assert!(ledger.findings.is_empty(), "{:?}", ledger.findings);
        assert_eq!(ledger.parsed, 2, "two documents, one comment each");
    }

    #[test]
    fn disabled_records_nothing() {
        let source = "// a\nx;\n";
        let comments = [line_comment(0, 4)];
        let _guard = LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        set_comment_check(false);
        let _ = take_comment_ledger();
        register_parsed(source, &comments);
        let ledger = take_comment_ledger();
        assert_eq!(ledger.parsed, 0);
        assert!(ledger.findings.is_empty());
    }
}
