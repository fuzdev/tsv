//! The checker's `Diagnostic` representation and the sort/dedup kernel.
//!
//! `Diagnostic` is the shape a future checker emits; at this slice the checker
//! emits none, so the value here is the **pure kernel** ports — the canonical
//! sort ([`compare_diagnostics`], tsgo's `CompareDiagnostics`) and the
//! dedup-with-related-info-merge ([`sort_and_deduplicate`], tsgo's
//! `compactAndMergeRelatedInfos`) — unit-tested against every comparator leg.
//! The canonical *sorted* order is a must-match property (baseline order is
//! canonical regardless of production order), so these are semantic-core.
//!
//! A message chain and each related-info entry are modelled as nested
//! `Diagnostic`s exactly as tsgo models them (`messageChain []*Diagnostic`,
//! related information `[]*Diagnostic`) — the comparator reads only the fields
//! each leg touches (chain nodes: code/args/chain; related nodes: the full
//! diagnostic recursively). Message text is kept as an owned string here (the
//! template-catalog codegen is a later phase); the comparator keys on `args`,
//! never the rendered text, so this simplification is faithful.
//
// tsgo: internal/ast/diagnostic.go CompareDiagnostics (:329),
//       EqualDiagnosticsNoRelatedInfo (:265), equalMessageChain,
//       compareMessageChainSize/Content, compareRelatedInfo
// tsgo: internal/compiler/program.go SortAndDeduplicateDiagnostics (:1436),
//       compactAndMergeRelatedInfos (:1444)

use crate::ids::FileId;
use std::cmp::Ordering;
use tsv_lang::Span;

/// The diagnostic category (tsc's `DiagnosticCategory`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    /// A warning (tsc emits these rarely).
    Warning,
    /// An error — the conformance-visible category.
    Error,
    /// A suggestion (opt-in, `@captureSuggestions`).
    Suggestion,
    /// An informational message.
    Message,
}

/// One diagnostic — plus, when it is a message-chain or related-info node, its
/// nested diagnostics.
///
/// The comparator/dedup read a node differently by role:
/// - as a top-level diagnostic: path, span, code, args, chain, related info;
/// - as a **chain** node: only code, args, and its own chain;
/// - as a **related-info** node: the full diagnostic, recursively.
#[derive(Clone, Debug)]
pub struct Diagnostic {
    /// The file this diagnostic points at; `None` for a global (fileless) one,
    /// which sorts first (its path resolves to the empty string).
    pub file: Option<FileId>,
    /// The source range — `Pos` (start) and `End` in the tsgo comparer.
    pub span: Span,
    /// The tsc numeric code (the checker emits positive TS codes).
    pub code: u32,
    /// The diagnostic category.
    pub category: Category,
    /// The rendered head message (display only — the comparer keys on `args`).
    pub message: String,
    /// Message-substitution arguments — tsgo's `MessageArgs` compared leg.
    pub args: Vec<String>,
    /// The elaboration chain, each node a nested `Diagnostic`.
    pub chain: Vec<Diagnostic>,
    /// Related information, each entry a full nested `Diagnostic`.
    pub related: Vec<Diagnostic>,
}

impl Diagnostic {
    /// Build a bare error diagnostic (no chain, no related info) — the shape
    /// the first family checks will emit.
    #[must_use]
    pub fn error(file: FileId, span: Span, code: u32, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            file: Some(file),
            span,
            code,
            category: Category::Error,
            message: message.into(),
            args: Vec::new(),
            chain: Vec::new(),
            related: Vec::new(),
        }
    }
}

/// The diagnostic's sort path: the file's name, or `""` for a global one.
fn diag_path<'a>(d: &Diagnostic, paths: &'a [String]) -> &'a str {
    match d.file {
        Some(f) => paths.get(f.index()).map_or("", String::as_str),
        None => "",
    }
}

/// Compare two diagnostics by tsgo's `CompareDiagnostics` total order:
/// path -> start -> end -> code -> args -> chain size -> chain content ->
/// related info. `paths` maps `FileId` -> file name.
///
/// `args` compares as `slices.Compare` (element-wise then length) — Rust's
/// `Vec<String>` ordering is exactly that.
// tsgo: internal/ast/diagnostic.go CompareDiagnostics (:329)
#[must_use]
pub fn compare_diagnostics(a: &Diagnostic, b: &Diagnostic, paths: &[String]) -> Ordering {
    diag_path(a, paths)
        .cmp(diag_path(b, paths))
        .then_with(|| a.span.start.cmp(&b.span.start))
        .then_with(|| a.span.end.cmp(&b.span.end))
        .then_with(|| a.code.cmp(&b.code))
        .then_with(|| a.args.cmp(&b.args))
        .then_with(|| compare_chain_size(&a.chain, &b.chain))
        .then_with(|| compare_chain_content(&a.chain, &b.chain))
        .then_with(|| compare_related_info(&a.related, &b.related, paths))
}

/// Compare message-chain size: **more elaboration sorts first**, then recurse
/// pairwise (tsgo `compareMessageChainSize`, `len(c2) - len(c1)`).
fn compare_chain_size(a: &[Diagnostic], b: &[Diagnostic]) -> Ordering {
    b.len().cmp(&a.len()).then_with(|| {
        for (ca, cb) in a.iter().zip(b) {
            let c = compare_chain_size(&ca.chain, &cb.chain);
            if c != Ordering::Equal {
                return c;
            }
        }
        Ordering::Equal
    })
}

/// Compare message-chain content: per element, compare `args`, then recurse
/// (tsgo `compareMessageChainContent`). Sizes are already equal when this runs.
fn compare_chain_content(a: &[Diagnostic], b: &[Diagnostic]) -> Ordering {
    for (ca, cb) in a.iter().zip(b) {
        let c = ca.args.cmp(&cb.args);
        if c != Ordering::Equal {
            return c;
        }
        let c = compare_chain_content(&ca.chain, &cb.chain);
        if c != Ordering::Equal {
            return c;
        }
    }
    Ordering::Equal
}

/// Compare related-info lists: **more related info sorts first**, then compare
/// each entry as a full diagnostic (tsgo `compareRelatedInfo`).
fn compare_related_info(a: &[Diagnostic], b: &[Diagnostic], paths: &[String]) -> Ordering {
    b.len().cmp(&a.len()).then_with(|| {
        for (ra, rb) in a.iter().zip(b) {
            let c = compare_diagnostics(ra, rb, paths);
            if c != Ordering::Equal {
                return c;
            }
        }
        Ordering::Equal
    })
}

/// Equality excluding related information (tsgo `EqualDiagnosticsNoRelatedInfo`):
/// path, loc (start+end), code, args, and the full message chain.
#[must_use]
pub fn equal_no_related_info(a: &Diagnostic, b: &Diagnostic, paths: &[String]) -> bool {
    diag_path(a, paths) == diag_path(b, paths)
        && a.span == b.span
        && a.code == b.code
        && a.args == b.args
        && equal_chain(&a.chain, &b.chain)
}

/// Message-chain equality (tsgo `equalMessageChain`): code, args, chain —
/// **not** path or loc.
fn equal_chain(a: &[Diagnostic], b: &[Diagnostic]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| {
            x.code == y.code && x.args == y.args && equal_chain(&x.chain, &y.chain)
        })
}

/// Full diagnostic equality (tsgo `EqualDiagnostics`): equal-no-related-info and
/// related info equal recursively.
#[must_use]
pub fn equal_diagnostics(a: &Diagnostic, b: &Diagnostic, paths: &[String]) -> bool {
    equal_no_related_info(a, b, paths)
        && a.related.len() == b.related.len()
        && a.related.iter().zip(&b.related).all(|(x, y)| equal_diagnostics(x, y, paths))
}

/// Sort `diags` into canonical order and merge duplicates, faithful to tsgo's
/// `SortAndDeduplicateDiagnostics` -> `compactAndMergeRelatedInfos`: a run of
/// diagnostics equal except for related information collapses to the first, with
/// their related infos concatenated, sorted, and deduped.
// tsgo: internal/compiler/program.go SortAndDeduplicateDiagnostics (:1436)
pub fn sort_and_deduplicate(diags: &mut Vec<Diagnostic>, paths: &[String]) {
    diags.sort_by(|a, b| compare_diagnostics(a, b, paths));
    compact_and_merge_related_infos(diags, paths);
}

/// Collapse runs of `equal_no_related_info` diagnostics, merging related info
/// (tsgo `compactAndMergeRelatedInfos`). Keeps the first of each run.
// The inner `while let` can't become a `for`: the iterator is shared with the
// outer run loop (a non-equal candidate becomes the next run's head), so it must
// outlive the inner loop.
#[allow(clippy::while_let_on_iterator)]
fn compact_and_merge_related_infos(diags: &mut Vec<Diagnostic>, paths: &[String]) {
    if diags.len() < 2 {
        return;
    }
    let mut out: Vec<Diagnostic> = Vec::with_capacity(diags.len());
    let mut iter = std::mem::take(diags).into_iter();
    let mut current = iter.next();
    while let Some(mut head) = current.take() {
        // Related infos across the whole run (the head's included, per tsgo).
        let mut run_related: Vec<Diagnostic> = std::mem::take(&mut head.related);
        let mut had_dupes = false;
        // `current` stays `None` if the iterator empties — the run ends the list.
        while let Some(candidate) = iter.next() {
            if equal_no_related_info(&head, &candidate, paths) {
                had_dupes = true;
                run_related.extend(candidate.related);
            } else {
                current = Some(candidate);
                break;
            }
        }
        if had_dupes {
            // tsgo sets merged related only when the run produced any; an
            // all-empty run leaves the head's (empty) related untouched.
            if !run_related.is_empty() {
                run_related.sort_by(|a, b| compare_diagnostics(a, b, paths));
                dedup_by_equal(&mut run_related, paths);
                head.related = run_related;
            }
        } else {
            // A singleton run keeps its related info verbatim.
            head.related = run_related;
        }
        out.push(head);
    }
    *diags = out;
}

/// Remove adjacent `equal_diagnostics` duplicates, keeping the first (tsgo
/// `slices.CompactFunc(_, EqualDiagnostics)` over the sorted related list).
fn dedup_by_equal(diags: &mut Vec<Diagnostic>, paths: &[String]) {
    diags.dedup_by(|a, b| equal_diagnostics(a, b, paths));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> Vec<String> {
        vec!["a.ts".to_string(), "b.ts".to_string()]
    }

    fn diag(file: Option<u32>, start: u32, end: u32, code: u32) -> Diagnostic {
        Diagnostic {
            file: file.map(FileId),
            span: Span::new(start, end),
            code,
            category: Category::Error,
            message: String::new(),
            args: Vec::new(),
            chain: Vec::new(),
            related: Vec::new(),
        }
    }

    fn with_args(mut d: Diagnostic, args: &[&str]) -> Diagnostic {
        d.args = args.iter().map(|s| (*s).to_string()).collect();
        d
    }

    fn with_chain(mut d: Diagnostic, chain: Vec<Diagnostic>) -> Diagnostic {
        d.chain = chain;
        d
    }

    fn with_related(mut d: Diagnostic, related: Vec<Diagnostic>) -> Diagnostic {
        d.related = related;
        d
    }

    /// Assert `a` sorts strictly before `b` under the two-file `paths()`.
    fn assert_lt(a: &Diagnostic, b: &Diagnostic) {
        assert_eq!(compare_diagnostics(a, b, &paths()), Ordering::Less);
        assert_eq!(compare_diagnostics(b, a, &paths()), Ordering::Greater);
    }

    #[test]
    fn path_leg() {
        // b.ts (FileId 1) sorts after a.ts (FileId 0); a global (None) first.
        assert_lt(&diag(Some(0), 0, 0, 1), &diag(Some(1), 0, 0, 1));
        assert_lt(&diag(None, 5, 5, 1), &diag(Some(0), 0, 0, 1));
    }

    #[test]
    fn start_then_end_then_code_legs() {
        assert_lt(&diag(Some(0), 1, 9, 1), &diag(Some(0), 2, 3, 1)); // start
        assert_lt(&diag(Some(0), 2, 3, 1), &diag(Some(0), 2, 5, 1)); // same start, end tiebreak
        assert_lt(&diag(Some(0), 2, 3, 1), &diag(Some(0), 2, 3, 9)); // same span, code tiebreak
    }

    #[test]
    fn args_leg_is_slices_compare() {
        let p = paths();
        let a = with_args(diag(Some(0), 0, 0, 1), &["x"]);
        let b = with_args(diag(Some(0), 0, 0, 1), &["y"]);
        assert_eq!(compare_diagnostics(&a, &b, &p), Ordering::Less);
        // shorter prefix sorts first
        let short = with_args(diag(Some(0), 0, 0, 1), &["x"]);
        let long = with_args(diag(Some(0), 0, 0, 1), &["x", "y"]);
        assert_eq!(compare_diagnostics(&short, &long, &p), Ordering::Less);
    }

    #[test]
    fn chain_size_leg_more_elaboration_first() {
        let p = paths();
        let child = diag(Some(0), 0, 0, 2);
        let more = with_chain(diag(Some(0), 0, 0, 1), vec![child]);
        let less = diag(Some(0), 0, 0, 1);
        // more elaboration (non-empty chain) sorts first
        assert_eq!(compare_diagnostics(&more, &less, &p), Ordering::Less);
        assert_eq!(compare_diagnostics(&less, &more, &p), Ordering::Greater);
    }

    #[test]
    fn chain_content_leg() {
        let p = paths();
        let a = with_chain(diag(Some(0), 0, 0, 1), vec![with_args(diag(Some(0), 0, 0, 2), &["a"])]);
        let b = with_chain(diag(Some(0), 0, 0, 1), vec![with_args(diag(Some(0), 0, 0, 2), &["b"])]);
        // same size, chain content (args) breaks the tie
        assert_eq!(compare_diagnostics(&a, &b, &p), Ordering::Less);
    }

    #[test]
    fn related_info_leg_count_then_recursive() {
        let p = paths();
        let r = diag(Some(0), 3, 4, 9);
        let more = with_related(diag(Some(0), 0, 0, 1), vec![r]);
        let less = diag(Some(0), 0, 0, 1);
        // more related info sorts first
        assert_eq!(compare_diagnostics(&more, &less, &p), Ordering::Less);
        // same count, recursive compare of the related entries
        let a = with_related(diag(Some(0), 0, 0, 1), vec![diag(Some(0), 1, 2, 9)]);
        let b = with_related(diag(Some(0), 0, 0, 1), vec![diag(Some(0), 5, 6, 9)]);
        assert_eq!(compare_diagnostics(&a, &b, &p), Ordering::Less);
    }

    #[test]
    fn equal_ignores_related_but_reads_chain() {
        let p = paths();
        let base = diag(Some(0), 0, 0, 1);
        let with_r = with_related(diag(Some(0), 0, 0, 1), vec![diag(Some(0), 9, 9, 2)]);
        assert!(equal_no_related_info(&base, &with_r, &p));
        assert!(!equal_diagnostics(&base, &with_r, &p));
        // a differing chain breaks even no-related equality
        let with_c = with_chain(diag(Some(0), 0, 0, 1), vec![diag(Some(0), 0, 0, 2)]);
        assert!(!equal_no_related_info(&base, &with_c, &p));
    }

    #[test]
    fn dedup_collapses_identical() {
        let p = paths();
        let mut v = vec![diag(Some(0), 0, 0, 1), diag(Some(0), 0, 0, 1), diag(Some(0), 1, 1, 2)];
        sort_and_deduplicate(&mut v, &p);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].code, 1);
        assert_eq!(v[1].code, 2);
    }

    #[test]
    fn dedup_merges_related_info_sorted_and_deduped() {
        let p = paths();
        // Two diagnostics equal-except-related-info; their related infos merge,
        // sort, and dedup into the survivor.
        let d1 = with_related(diag(Some(0), 0, 0, 1), vec![diag(Some(0), 5, 6, 9)]);
        let d2 = with_related(
            diag(Some(0), 0, 0, 1),
            vec![diag(Some(0), 1, 2, 9), diag(Some(0), 5, 6, 9)],
        );
        let mut v = vec![d1, d2];
        sort_and_deduplicate(&mut v, &p);
        assert_eq!(v.len(), 1);
        // (1,2) and (5,6) survive once each, sorted by position.
        assert_eq!(v[0].related.len(), 2);
        assert_eq!(v[0].related[0].span, Span::new(1, 2));
        assert_eq!(v[0].related[1].span, Span::new(5, 6));
    }

    #[test]
    fn sort_is_stable_total_order() {
        let p = paths();
        let mut v = vec![
            diag(Some(1), 0, 0, 1),
            diag(Some(0), 4, 5, 1),
            diag(None, 0, 0, 7),
            diag(Some(0), 4, 5, 1),
            diag(Some(0), 1, 2, 1),
        ];
        sort_and_deduplicate(&mut v, &p);
        // global first, then a.ts by position, then b.ts; the duplicate collapsed.
        assert_eq!(v.len(), 4);
        assert_eq!(v[0].file, None);
        assert_eq!(v[1].file, Some(FileId(0)));
        assert_eq!(v[1].span, Span::new(1, 2));
        assert_eq!(v[2].file, Some(FileId(0)));
        assert_eq!(v[2].span, Span::new(4, 5));
        assert_eq!(v[3].file, Some(FileId(1)));
    }
}
