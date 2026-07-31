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
//! Two comment carriers are in scope, both registered by the format entry points:
//!
//! - the **detached** comments — the flat `Vec<Comment>` on each language root, registered
//!   via [`register_parsed`].
//! - the **AST-node** comments — a Svelte `<!-- … -->` (`FragmentNode::Comment`) and a CSS
//!   in-block `CssBlockChild::Comment`. These are carried by the tree rather than by the
//!   positional model, but a printer can still drop one (a walk that misses a fragment, a
//!   builder that reassembles) or double-print it, so each format entry walks its tree and
//!   registers their spans via [`register_parsed_spans`].
//!
//! With both registered, [`CommentLedger::unregistered_emits`] is a pure registration-gap
//! signal — an emit for a span no entry declared — which over a clean corpus is zero.
//!
//! A CSS declaration's *value* comments remain out of scope **by construction**: the parser
//! never lexes them as `Comment`s at all (they are re-derived from source), so there is
//! nothing to register and nothing to record.
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
    /// Emits for a span no [`register_parsed`] / [`register_parsed_spans`] declared — a
    /// genuine registration gap (an emitter running over a comment the walk missed). Counted,
    /// never a finding; over a clean corpus it is zero.
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
    /// Line comments a `BlockOnly`-filtered builder passed over, with the call site that
    /// held the licence — see [`record_filtered_skip`]. Joined against the DROPPED
    /// findings at drain; never a finding by itself.
    filtered_skips: Vec<FilteredSkip>,
}

/// One recorded `BlockOnly` skip: the line comment's span and the builder call site that
/// skipped it (captured via `#[track_caller]` at the printers' filtered comment builder).
struct FilteredSkip {
    span: Span,
    site: &'static std::panic::Location<'static>,
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
    /// The `BlockOnly`-filtered builder call sites that skipped this comment during the
    /// format (`file:line:column`, sorted + deduped), populated only for
    /// [`CommentFindingKind::Dropped`] — the skip-∧-dropped join that names the builder
    /// whose gate broke the line-comment routing promise. Empty when no filtered builder
    /// saw the comment (the drop happened elsewhere), and always empty for
    /// [`CommentFindingKind::DoublePrinted`] (a skip cannot cause a double print).
    pub skip_sites: Vec<String>,
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
    register_parsed_spans(source, comments.iter().map(|c| c.span));
}

/// Declare a set of comment **spans** a format entry point is about to print — the
/// span-based twin of [`register_parsed`], for the AST-node comment kinds whose carrier is
/// not a [`Comment`]. A Svelte `<!-- … -->` (`HtmlComment`) holds only spans; a CSS in-block
/// `CssBlockChild::Comment` holds a real [`Comment`] and may use either. The ledger reads
/// only the span — it keys on it and stores `span.extract(source)` as the text — so a caller
/// with just the spans loses nothing by not manufacturing a `Comment` with fields the ledger
/// never reads.
///
/// Idempotent per span, exactly like [`register_parsed`]: a span already registered (by
/// another list or a second call) does not double the expectation.
pub fn register_parsed_spans(source: &str, spans: impl IntoIterator<Item = Span>) {
    if !comment_check_enabled() {
        return;
    }
    DOCS.with(|docs| {
        let mut docs = docs.borrow_mut();
        let doc = doc_for(&mut docs, source);
        for span in spans {
            if let Err(idx) = doc
                .entries
                .binary_search_by_key(&(span.start, span.end), |e| (e.span.start, e.span.end))
            {
                doc.entries.insert(
                    idx,
                    Entry {
                        span,
                        text: span.extract(source).to_string(),
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

/// Record a line comment a `CommentFilter::BlockOnly` builder **skipped**, with the call
/// site that held the licence (the filtered builder's own caller, captured via
/// `#[track_caller]`).
///
/// The `BlockOnly` licence is a promise: the caller's gate routed line comments to an
/// expansion builder first, so the filtered builder never meets one. Nothing enforces
/// that the gate is the exact complement of what the filtered builder can express — this
/// records the evidence when the promise breaks. A skip is an **annotation, never a
/// finding**: it is legitimate wherever the routing worked but the doc was built anyway
/// (a losing `conditional_group` candidate) or another emitter prints the comment (the
/// `line_suffix` family), and in both cases the comment ends the format emitted and the
/// annotation stays invisible. Only when the skipped comment ends the format **DROPPED**
/// does [`take_comment_ledger`] surface the recorded site(s) on the finding
/// ([`CommentFinding::skip_sites`]) — naming the responsible builder at the site, instead
/// of leaving the drop to be found by injection wherever a fixture happens to reach it.
pub fn record_filtered_skip(
    source: &str,
    span: Span,
    site: &'static std::panic::Location<'static>,
) {
    if !comment_check_enabled() {
        return;
    }
    DOCS.with(|docs| {
        let mut docs = docs.borrow_mut();
        doc_for(&mut docs, source)
            .filtered_skips
            .push(FilteredSkip { span, site });
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

/// The spans of every comment registered on this thread's ledger **against `source`**, read
/// **without draining** it.
///
/// The ledger holds the parsed-comment spans (the [`register_parsed`] expectation) but
/// otherwise surfaces only aggregate counts. The gap-injection audit needs the spans
/// themselves — to exclude an injection site that falls *inside* an existing comment, which
/// mutilates the author's comment rather than probing a gap — so this exposes them. Read
/// before [`take_comment_ledger`], which discards them; the per-injection hot path (which
/// only ever drains) never pays to collect them.
///
/// Spans are byte offsets over `source`. Scoping to `document_key(source)` is what keeps them
/// in one coordinate space **by construction**: the host and its top-level
/// `<script>`/`<style>` islands all register host-absolute spans under the host's key and are
/// returned; a nested `<style>` *element* re-parses island-relative under its own key (see the
/// module docs) and is structurally excluded — so an island-relative span can never be
/// mistaken for a host offset. Pass the exact binding the format entry registered against, so
/// the key matches by pointer identity.
#[must_use]
pub fn parsed_comment_spans(source: &str) -> Vec<Span> {
    if !comment_check_enabled() {
        return Vec::new();
    }
    let key = document_key(source);
    DOCS.with(|docs| {
        docs.borrow()
            .iter()
            .filter(|d| d.key == key)
            .flat_map(|d| d.entries.iter().map(|e| e.span))
            .collect()
    })
}

/// The full source text of every comment registered on this thread's ledger, read **without
/// draining** it — the content twin of [`parsed_comment_spans`].
///
/// Unlike the span accessor this is deliberately **not** scoped to a document key: a text
/// carries no offset, so the collision the span scoping guards against (an island-relative
/// offset mistaken for a host one) cannot arise, and returning every document's comments is
/// what lets a caller compare the whole comment *content* of one format against another — a
/// drop or a mangle in *any* island (the host `<script>`, a nested `<style>` element) then
/// shows up in the difference. The gap-injection audit's self-verify uses exactly that: the
/// multiset of contents in a format's input vs its output decides whether a ledger finding is
/// a real loss or an instrument gap. Read before [`take_comment_ledger`], which discards the
/// entries.
///
/// A comment registered under two lists over one span (the Svelte `<script>` island's
/// `Program.comments` and the `Root.comments` clone of it) is deduped within its document by
/// [`register_parsed_spans`], so it is returned once, not twice.
#[must_use]
pub fn parsed_comment_texts() -> Vec<String> {
    if !comment_check_enabled() {
        return Vec::new();
    }
    DOCS.with(|docs| {
        docs.borrow()
            .iter()
            .flat_map(|d| d.entries.iter().map(|e| e.text.clone()))
            .collect()
    })
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
            // The skip-∧-dropped join: a DROPPED comment some `BlockOnly` builder passed
            // over names that builder's call site on the finding. Dropped-only — an
            // emitted comment clears its annotations (the skip was legitimate), and a
            // skip cannot cause a double print.
            let skip_sites = if kind == CommentFindingKind::Dropped {
                let mut sites: Vec<String> = doc
                    .filtered_skips
                    .iter()
                    .filter(|s| s.span == entry.span)
                    .map(|s| s.site.to_string())
                    .collect();
                sites.sort();
                sites.dedup();
                sites
            } else {
                Vec::new()
            };
            ledger.findings.push(CommentFinding {
                kind,
                text: entry.text.clone(),
                span: entry.span,
                emitted,
                skip_sites,
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
            filtered_skips: Vec::new(),
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
    fn an_emit_for_an_unregistered_span_is_a_registration_gap() {
        // No `register_parsed` / `register_parsed_spans` ever declared this span, yet an
        // emitter ran over it — a genuine registration gap (the walk missed a comment). It is
        // counted, never a finding: the ledger can only assert print-once for spans it was
        // told to expect.
        let source = "// a\nx;\n";
        let ((), ledger) = with_check(|| record_emitted(source, Span::new(0, 4)));
        assert!(ledger.findings.is_empty());
        assert_eq!(ledger.unregistered_emits, 1);
    }

    #[test]
    fn a_registered_span_emitted_once_is_clean() {
        // The AST-node registration path (`register_parsed_spans`): a Svelte `<!-- -->` or a
        // CSS in-block comment registers by span, then its printer records the single emit.
        let source = "<!-- a -->\n";
        let ((), ledger) = with_check(|| {
            register_parsed_spans(source, [Span::new(0, 10)]);
            record_emitted(source, Span::new(0, 10));
        });
        assert!(ledger.findings.is_empty(), "{:?}", ledger.findings);
        assert_eq!(ledger.parsed, 1);
        assert_eq!(
            ledger.unregistered_emits, 0,
            "the emit matched a registered span"
        );
    }

    #[test]
    fn a_registered_span_never_emitted_is_dropped() {
        // Registered by span but the printer dropped it — the AST-node analog of a dropped
        // detached comment, now a real Dropped finding rather than a silent, out-of-scope loss.
        let source = "<!-- a -->\n";
        let ((), ledger) = with_check(|| register_parsed_spans(source, [Span::new(0, 10)]));
        assert_eq!(ledger.findings.len(), 1);
        assert_eq!(ledger.findings[0].kind, CommentFindingKind::Dropped);
        assert_eq!(ledger.findings[0].text, "<!-- a -->");
    }

    /// A skip whose comment ends the format DROPPED surfaces the recorded call site on the
    /// finding — the skip-∧-dropped join. Two distinct sites both skipping it are both
    /// named (sorted), and the same site skipping twice (a rebuilt candidate) dedupes.
    #[test]
    fn a_skipped_and_dropped_comment_names_the_skip_sites() {
        let source = "// a\nx;\n";
        let comments = [line_comment(0, 4)];
        let site_a = std::panic::Location::caller();
        let ((), ledger) = with_check(|| {
            register_parsed(source, &comments);
            record_filtered_skip(source, comments[0].span, site_a);
            record_filtered_skip(source, comments[0].span, site_a); // rebuilt candidate: dedupes
        });
        assert_eq!(ledger.findings.len(), 1);
        assert_eq!(ledger.findings[0].kind, CommentFindingKind::Dropped);
        assert_eq!(
            ledger.findings[0].skip_sites,
            vec![site_a.to_string()],
            "the responsible site is named once, deduped"
        );
        assert!(ledger.findings[0].skip_sites[0].contains("comment_ledger.rs"));
    }

    /// A skip whose comment IS emitted (the routed expansion path printed it, or the
    /// skipping doc was a losing `conditional_group` candidate) is invisible — a bare skip
    /// is an annotation, never a finding.
    #[test]
    fn a_skipped_but_emitted_comment_is_clean() {
        let source = "// a\nx;\n";
        let comments = [line_comment(0, 4)];
        let ((), ledger) = with_check(|| {
            register_parsed(source, &comments);
            record_filtered_skip(source, comments[0].span, std::panic::Location::caller());
            record_emitted(source, comments[0].span);
        });
        assert!(ledger.findings.is_empty(), "{:?}", ledger.findings);
    }

    /// A dropped comment nothing skipped carries no sites — the drop happened elsewhere,
    /// and the finding must not point at an innocent builder.
    #[test]
    fn a_dropped_comment_without_skips_names_no_site() {
        let source = "// a\nx;\n";
        let comments = [line_comment(0, 4)];
        let ((), ledger) = with_check(|| register_parsed(source, &comments));
        assert_eq!(ledger.findings.len(), 1);
        assert!(ledger.findings[0].skip_sites.is_empty());
    }

    /// A double-printed comment never carries skip sites — a skip cannot cause a double
    /// print, so annotating one would misdirect the triage.
    #[test]
    fn a_double_printed_comment_carries_no_skip_sites() {
        let source = "// a\nx;\n";
        let comments = [line_comment(0, 4)];
        let ((), ledger) = with_check(|| {
            register_parsed(source, &comments);
            record_filtered_skip(source, comments[0].span, std::panic::Location::caller());
            record_emitted(source, comments[0].span);
            record_emitted(source, comments[0].span);
        });
        assert_eq!(ledger.findings.len(), 1);
        assert_eq!(ledger.findings[0].kind, CommentFindingKind::DoublePrinted);
        assert!(ledger.findings[0].skip_sites.is_empty());
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
    fn parsed_comment_spans_reads_registered_spans_without_draining() {
        let source = "// a\n// b\nx;\n";
        let comments = [line_comment(0, 4), line_comment(5, 9)];
        let (spans, ledger) = with_check(|| {
            register_parsed(source, &comments);
            // Read before drain: the spans survive; the ledger below still sees the entries.
            parsed_comment_spans(source)
        });
        assert_eq!(
            spans,
            vec![Span::new(0, 4), Span::new(5, 9)],
            "both registered spans, byte offsets over the source"
        );
        // The read did not drain — `take_comment_ledger` (inside `with_check`) still counted
        // the two entries, reporting them as dropped (nothing was emitted).
        assert_eq!(ledger.parsed, 2, "the peek left the ledger intact");
    }

    #[test]
    fn parsed_comment_texts_reads_all_documents_without_draining() {
        // The text accessor is NOT key-scoped (unlike the span one): a comment in the host
        // AND a comment registered under a separate island key both come back, so a content
        // compare can see a drop in any island. Read before drain leaves the ledger intact.
        let host = String::from("// a\nx;\n");
        let island = String::from("// b\ny;\n");
        let host_comments = [line_comment(0, 4)];
        let island_comments = [line_comment(0, 4)];
        let (texts, ledger) = with_check(|| {
            register_parsed(&host, &host_comments);
            register_parsed(&island, &island_comments);
            let mut t = parsed_comment_texts();
            t.sort();
            t
        });
        assert_eq!(
            texts,
            vec!["// a".to_string(), "// b".to_string()],
            "both documents' comment texts, across keys"
        );
        assert_eq!(ledger.parsed, 2, "the peek left the ledger intact");
    }

    #[test]
    fn parsed_comment_texts_is_empty_when_disabled() {
        let _guard = LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        set_comment_check(false);
        let _ = take_comment_ledger();
        assert!(parsed_comment_texts().is_empty());
    }

    #[test]
    fn parsed_comment_spans_scopes_to_the_host_key() {
        // A nested `<style>` element re-parses island-relative under its own key; its spans
        // must NOT leak into the host's — an island-relative offset could otherwise coincide
        // with a host code offset and drop a legit injection site. Both sources register a
        // comment at the SAME-valued span (0, 4) under DIFFERENT keys, so returning one span
        // proves the accessor filters by key, not by value.
        let host = String::from("// h\nx;\n");
        let island = String::from("// i\ny;\n");
        let host_comments = [line_comment(0, 4)];
        let island_comments = [line_comment(0, 4)];
        let (spans, _ledger) = with_check(|| {
            register_parsed(&host, &host_comments);
            register_parsed(&island, &island_comments);
            parsed_comment_spans(&host)
        });
        assert_eq!(
            spans,
            vec![Span::new(0, 4)],
            "only the host's comment, not the island's registered under a different key"
        );
    }

    #[test]
    fn parsed_comment_spans_is_empty_when_disabled() {
        let _guard = LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        set_comment_check(false);
        let _ = take_comment_ledger();
        assert!(parsed_comment_spans("// a\nx;\n").is_empty());
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
