//! Program pipeline assembly: parse (goal rule) -> bind -> check -> sort/dedup.
//!
//! **Two assembly modes, and this is the parity one.** The conformance harness
//! grades against tsgo's committed `.errors.txt` **baselines**, whose oracle path
//! is `harnessutil.go CompileFilesEx` (:634-645): it concatenates each unit's
//! syntactic + semantic diagnostics **unconditionally — no short-circuit**. So a
//! unit tsv parse-rejects contributes only its parse verdict (no AST to bind),
//! while every unit that parses contributes its bind/check diagnostics regardless
//! of a sibling's rejection. For the single-file tests this slice grades that
//! means a rejected file is simply ungradeable — its `CheckResult` has no
//! diagnostics because there is no AST, not because a program-wide guard fired.
//!
//! The **product path** (`tsv check`, mirroring the tsc CLI) is instead tsgo's
//! `GetDiagnosticsOfAnyProgram` (`program.go:1755`), which **short-circuits** at
//! :1770 — if syntactic diagnostics exist, semantic diagnostics are skipped
//! program-wide. Porting that short-circuit into this parity pipeline would
//! manufacture false `missing`s the moment multi-file grading starts, so it is a
//! deliberate mode distinction deferred to the product path, not modelled here.
//!
//! Each unit parses via the **goal rule**: `Goal::Module` first (correct for
//! ~all real TS), and on failure a `Goal::Script` retry (top-level `await` as an
//! identifier, no `import`/`export`). Both goals failing is a parse rejection.
//!
//! The caller owns the parse arena (`&'a Bump`) — the tsv_ts caller-owns-arena
//! contract scaled to a unit set. The returned [`CheckResult`] is fully owned
//! (diagnostics carry owned strings and `Copy` spans; nothing borrows the
//! arena), so the caller may reset and reuse the arena the moment this returns.
//
// tsgo: internal/testutil/harnessutil.go CompileFilesEx (:634-645) — the
//       baseline-oracle parity path (unconditional syntactic+semantic concat);
//       the bind-then-check concat is getBindAndCheckDiagnosticsWithChecker
//       (program.go:1337); final sort+dedup is caller-side (execute/tsc/emit.go:120).
//       The product-mode short-circuit lives at GetDiagnosticsOfAnyProgram
//       (program.go:1755, :1770) and is deliberately NOT ported here.

use crate::binder::flow::{FlowProduct, build_flow};
use crate::binder::{ModuleNess, bind_file, module_ness};
use crate::check::unreachable::{UnreachableCandidates, build_candidates};
use crate::diag::{Diagnostic, sort_and_deduplicate};
use crate::ids::FileId;
use crate::merge::{FileMerge, LibBase, LibFile, merge_program};
use crate::options::CheckOptions;
use bumpalo::Bump;
use tsv_ts::ast::Program;
use tsv_ts::{Goal, parse_with_goal};

/// One source unit to check — a file name (its diagnostic path) and its source.
pub struct SourceUnit<'a> {
    /// The unit's display name (the diagnostic path).
    pub name: &'a str,
    /// The unit's source text.
    pub source: &'a str,
}

impl<'a> SourceUnit<'a> {
    /// Build a source unit.
    #[must_use]
    pub fn new(name: &'a str, source: &'a str) -> SourceUnit<'a> {
        SourceUnit { name, source }
    }
}

/// The result of checking a program: its (sorted, deduped) diagnostics, its
/// suggestion-category diagnostics (a separate sink), a per-unit report, and
/// whether any unit parse-rejected.
pub struct CheckResult {
    /// Diagnostics in canonical sorted order — the unconditional concat of every
    /// unit that parsed (a rejected unit contributes none, having no AST).
    pub diagnostics: Vec<Diagnostic>,
    /// Suggestion-category diagnostics (the default-`Unknown` reachability
    /// shims), kept **out of** [`CheckResult::diagnostics`] so the conformance
    /// gate's error/expect-clean channel never sees them.
    pub suggestions: Vec<Diagnostic>,
    /// Per-unit parse/bind report, in input order.
    pub files: Vec<FileReport>,
    /// Whether any unit parse-rejected (a reported fact; it does **not** suppress
    /// the other units' diagnostics — see the module header's parity note).
    pub parse_rejected: bool,
}

/// The per-unit parse/bind report.
pub struct FileReport {
    /// The unit's file id.
    pub file: FileId,
    /// The unit's display name.
    pub name: String,
    /// The parse outcome and, when parsed, the bind facts.
    pub parse: ParseReport,
}

/// A unit's parse outcome.
#[derive(Clone)]
pub enum ParseReport {
    /// The unit parsed (possibly via the `Goal::Script` retry).
    Parsed(ParsedFacts),
    /// Both goals failed; `message` is the primary-goal (`Module`) error.
    Rejected {
        /// The `Goal::Module` parse error message.
        message: String,
    },
}

/// Facts recorded for a parsed unit.
#[derive(Clone)]
pub struct ParsedFacts {
    /// The goal the unit parsed under.
    pub goal: Goal,
    /// Whether the `Goal::Module` parse failed and the `Goal::Script` retry won.
    pub used_script_retry: bool,
    /// The unit's module-vs-script indicator (import/export presence).
    pub module_ness: ModuleNess,
    /// The bound node count (0 when the program short-circuited before binding).
    pub node_count: u32,
}

/// A parsed + bound program — variant-independent and fully owned
/// (arena-independent), so the caller may drop the parse arena the moment this
/// returns and merge it against any number of lib bases ([`check_bound`]). This is
/// the split that keeps parse+bind out of the per-variant loop: parse+bind once,
/// merge per resolved lib set.
pub struct BoundProgram {
    /// Whether any unit parse-rejected (a reported fact; it does **not** suppress
    /// the other units' diagnostics — the CompileFilesEx parity).
    pub parse_rejected: bool,
    units: Vec<BoundUnit>,
    total_nodes: u64,
}

/// One unit's owned bind product inside a [`BoundProgram`].
struct BoundUnit {
    file: FileId,
    name: String,
    parse: ParseReport,
    /// The bind (+ check) diagnostics — variant-independent, cloned into each
    /// [`check_bound`] result.
    bind_diagnostics: Vec<Diagnostic>,
    /// The merge product, `None` when the unit parse-rejected.
    merge: Option<FileMerge>,
    /// The per-file flow product, carried **dark** — `--dump-flow` renders it and
    /// F3's candidate table is built from it. `None` when the unit parse-rejected
    /// (no AST to walk).
    flow: Option<FlowProduct>,
    /// The variant-independent unreachable-code / unused-label candidate table
    /// (F3), built once at bind time and filtered per variant in [`check_bound`].
    /// `None` when the unit parse-rejected.
    candidates: Option<UnreachableCandidates>,
}

impl BoundProgram {
    /// Total bound nodes across parsed units (informational).
    #[must_use]
    pub fn total_node_count(&self) -> u64 {
        self.total_nodes
    }

    /// A unit's dark-carried flow product (`None` for a rejected unit or an
    /// out-of-range index). Nothing in the check pipeline reads it (F3 will);
    /// `--dump-flow` reaches it through this accessor.
    #[must_use]
    pub fn unit_flow(&self, index: usize) -> Option<&FlowProduct> {
        self.units.get(index).and_then(|u| u.flow.as_ref())
    }

    /// The per-unit parse reports, in input order (a read-only view for the caller
    /// that need not run [`check_bound`] to learn parse facts).
    #[must_use]
    pub fn parse_reports(&self) -> Vec<(&str, &ParseReport)> {
        self.units
            .iter()
            .map(|u| (u.name.as_str(), &u.parse))
            .collect()
    }
}

/// Parse every unit via the goal rule and bind each, returning the owned
/// [`BoundProgram`]. The merge is deferred to [`check_bound`] (it depends on the
/// resolved lib set), so this is variant-independent.
#[must_use]
pub fn bind_program<'a>(units: &[SourceUnit<'a>], arena: &'a Bump) -> BoundProgram {
    let mut bound_units: Vec<BoundUnit> = Vec::with_capacity(units.len());
    let mut parse_rejected = false;
    let mut total_nodes = 0u64;

    for (i, unit) in units.iter().enumerate() {
        let file = FileId(i as u32);
        match parse_unit(unit.source, arena) {
            Ok((program, goal, used_script_retry)) => {
                let module_ness = module_ness(&program);
                let bound = bind_file(&program, unit.source, file);
                total_nodes += u64::from(bound.node_count);
                // The third walk: the flow graph, built from the parsed program
                // and F0's node identity. Borrows `&bound`, so it runs before the
                // bind product's fields move out below. Carried dark in the unit.
                let flow = build_flow(&program, unit.source, &bound);
                // F3: the unreachable-code / unused-label candidate table, built
                // once here (the flag bit, run grouping, and const-enum/module
                // classification are all syntactic). Filtered per variant in
                // `check_bound`, keeping `BoundProgram` variant-independent.
                let candidates = build_candidates(&program, unit.source, &bound, &flow);
                // Per file: bind diagnostics then check diagnostics — the
                // getBindAndCheckDiagnostics concat. The check pass is a standalone
                // syntactic walk over the program (it needs no `BoundFile`); its
                // output folds in here, and the program-wide sort/dedup collapses any
                // diagnostic the bind and check both emit.
                let check_diags = crate::check::check_file_members(&program, unit.source, file);
                let mut bind_diagnostics = bound.diagnostics;
                bind_diagnostics.extend(check_diags);
                bound_units.push(BoundUnit {
                    file,
                    name: unit.name.to_string(),
                    parse: ParseReport::Parsed(ParsedFacts {
                        goal,
                        used_script_retry,
                        module_ness,
                        node_count: bound.node_count,
                    }),
                    bind_diagnostics,
                    merge: Some(bound.merge),
                    flow: Some(flow),
                    candidates: Some(candidates),
                });
            }
            Err(message) => {
                parse_rejected = true;
                bound_units.push(BoundUnit {
                    file,
                    name: unit.name.to_string(),
                    parse: ParseReport::Rejected { message },
                    bind_diagnostics: Vec::new(),
                    merge: None,
                    flow: None,
                    candidates: None,
                });
            }
        }
    }

    BoundProgram {
        parse_rejected,
        units: bound_units,
        total_nodes,
    }
}

/// Merge a [`BoundProgram`] against an optional [`LibBase`] under `options` and
/// return the final [`CheckResult`] (canonically sorted + deduped). The bind
/// diagnostics are the variant-independent concat (the CompileFilesEx parity
/// path); the merge phase consults the lib base, so the lib file names append
/// after the program units in the diagnostic path space. `options` drives the
/// per-variant reachability shims (TS7027/7028) — the only option-dependent
/// output, routed to `diagnostics` (error) or the separate `suggestions` sink.
// `options` is threaded by reference (uniform with `lib: Option<&LibBase>` and
// future-proof if `CheckOptions` grows) though it is currently `Copy`-small.
#[allow(clippy::trivially_copy_pass_by_ref)]
#[must_use]
pub fn check_bound(
    bound: &BoundProgram,
    lib: Option<&LibBase>,
    options: &CheckOptions,
) -> CheckResult {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let mut suggestions: Vec<Diagnostic> = Vec::new();
    for unit in &bound.units {
        diagnostics.extend(unit.bind_diagnostics.iter().cloned());
        // F3: filter the unit's variant-independent candidate table under
        // `options` — errors into `diagnostics`, suggestions into their own sink.
        if let Some(candidates) = &unit.candidates {
            candidates.emit(unit.file, options, &mut diagnostics, &mut suggestions);
        }
    }
    // Only test-unit merges are cloned here (lib globals live in the base, not in
    // `files`), so this stays cheap even run per-variant.
    let merges: Vec<FileMerge> = bound.units.iter().filter_map(|u| u.merge.clone()).collect();
    let lib_file_offset = bound.units.len() as u32;
    diagnostics.extend(merge_program(&merges, lib, lib_file_offset));

    // Path space: program units first, then the lib files (their FileIds are
    // `lib_file_offset + lib-local index`).
    let mut paths: Vec<String> = bound.units.iter().map(|u| u.name.clone()).collect();
    if let Some(base) = lib {
        paths.extend(base.lib_files.iter().cloned());
    }
    sort_and_deduplicate(&mut diagnostics, &paths);
    sort_and_deduplicate(&mut suggestions, &paths);

    let files = bound
        .units
        .iter()
        .map(|u| FileReport {
            file: u.file,
            name: u.name.clone(),
            parse: u.parse.clone(),
        })
        .collect();
    CheckResult {
        diagnostics,
        suggestions,
        files,
        parse_rejected: bound.parse_rejected,
    }
}

/// Check a program with no lib base — parse every unit via the goal rule, bind,
/// merge, and return canonically sorted diagnostics.
#[allow(clippy::trivially_copy_pass_by_ref)] // `&CheckOptions` — see `check_bound`
#[must_use]
pub fn check_program<'a>(
    units: &[SourceUnit<'a>],
    arena: &'a Bump,
    options: &CheckOptions,
) -> CheckResult {
    check_bound(&bind_program(units, arena), None, options)
}

/// Check a program against an optional lib base (the lib-aware entry point).
#[allow(clippy::trivially_copy_pass_by_ref)] // `&CheckOptions` — see `check_bound`
#[must_use]
pub fn check_program_with_lib<'a>(
    units: &[SourceUnit<'a>],
    lib: Option<&LibBase>,
    arena: &'a Bump,
    options: &CheckOptions,
) -> CheckResult {
    check_bound(&bind_program(units, arena), lib, options)
}

/// Parse + bind one lib `.d.ts` file, returning its owned global-eligible product
/// for folding into a [`LibBase`]. A lib is an ambient script; its globals are its
/// source-file locals (bound under FileId 0 — the fold re-keys by priority index).
///
/// # Errors
///
/// Returns the parse error message when the lib file does not parse under either
/// goal (expected never for the bundled libs; the caller counts it as a carve-out).
pub fn bind_lib(name: &str, source: &str) -> Result<LibFile, String> {
    let arena = Bump::new();
    let (program, _goal, _retry) = parse_unit(source, &arena)?;
    let bound = bind_file(&program, source, FileId::ROOT);
    // A lib contributes its globals through the merge either as an ambient script
    // (globals in `source_locals`) or, when the lib file is itself a module — e.g.
    // `lib.es2025.iterator.d.ts`, which carries a top-level `export {}` and so binds
    // external — through a `declare global {}` block (`global_augmentations`). A lib
    // that bound external with NEITHER would silently fold to nothing. This
    // `debug_assert!` is the fast local guard (dev builds only); the conformance
    // harness's lib channel counts any such lib and fails its run on a nonzero count,
    // so release/corpus builds catch the same no-op this compiles out of.
    debug_assert!(
        !bound.merge.is_external || !bound.merge.global_augmentations.is_empty(),
        "lib {name} bound as an external module with no `declare global` block — its globals would be silently dropped",
    );
    Ok(LibFile {
        name: name.to_string(),
        merge: bound.merge,
    })
}

/// Parse a unit via the goal rule: `Module` first, `Script` on failure. Returns
/// the program, the goal it parsed under, and whether the `Script` retry won; on
/// double failure returns the `Module`-goal error message.
fn parse_unit<'a>(source: &'a str, arena: &'a Bump) -> Result<(Program<'a>, Goal, bool), String> {
    match parse_with_goal(source, Goal::Module, arena) {
        Ok(program) => Ok((program, Goal::Module, false)),
        Err(module_err) => match parse_with_goal(source, Goal::Script, arena) {
            Ok(program) => Ok((program, Goal::Script, true)),
            // Both goals failed: report the primary (Module) goal's error.
            Err(_script_err) => Err(module_err.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(source: &str) -> CheckResult {
        let arena = Bump::new();
        check_program(
            &[SourceUnit::new("test.ts", source)],
            &arena,
            &CheckOptions::default(),
        )
    }

    #[test]
    fn clean_program_binds_and_grades_empty() {
        let result = check("const x: number = 1;");
        assert!(!result.parse_rejected);
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.files.len(), 1);
        match &result.files[0].parse {
            ParseReport::Parsed(facts) => {
                assert_eq!(facts.goal, Goal::Module);
                assert!(!facts.used_script_retry);
                assert!(facts.node_count >= 3); // Program + decl + declarator (+ id)
            }
            ParseReport::Rejected { .. } => panic!("expected a clean parse"),
        }
    }

    #[test]
    fn parse_rejection_yields_no_diagnostics() {
        // A hard syntax error both goals reject: no AST to bind, so no diagnostics
        // (the single-file "ungradeable" case).
        let result = check("const = = = ;");
        assert!(result.parse_rejected);
        assert!(result.diagnostics.is_empty());
        assert!(matches!(
            result.files[0].parse,
            ParseReport::Rejected { .. }
        ));
    }

    #[test]
    fn script_retry_on_top_level_import_free_await_ident() {
        // `await` as a plain binding is a Module-goal error (reserved) but valid
        // at Script goal — the retry should win.
        let result = check("var await = 1;");
        match &result.files[0].parse {
            ParseReport::Parsed(facts) => {
                assert_eq!(facts.goal, Goal::Script);
                assert!(facts.used_script_retry);
            }
            ParseReport::Rejected { .. } => panic!("expected the Script retry to win"),
        }
    }

    #[test]
    fn sibling_rejection_does_not_suppress_a_parsed_unit() {
        // The CompileFilesEx parity: a rejected sibling does NOT short-circuit the
        // program — the unit that parsed still contributes its bind diagnostics.
        let arena = Bump::new();
        let result = check_program(
            &[
                SourceUnit::new("a.ts", "let x; let x;"),
                SourceUnit::new("b.ts", "const = ;"),
            ],
            &arena,
            &CheckOptions::default(),
        );
        assert!(result.parse_rejected);
        // a.ts's two TS2451 survive despite b.ts rejecting.
        assert_eq!(result.diagnostics.len(), 2);
        assert!(result.diagnostics.iter().all(|d| d.code == 2451));
        assert!(matches!(result.files[0].parse, ParseReport::Parsed(_)));
        assert!(matches!(
            result.files[1].parse,
            ParseReport::Rejected { .. }
        ));
    }

    #[test]
    fn computed_literal_key_bind_and_check_spans_collapse() {
        // A computed-literal key that conflicts at BOTH bind (method vs property) and
        // check (property vs property) once produced two differently-spanned
        // diagnostics per declaration — the bind side spanned the bare literal, the
        // check side the whole `[ … ]` node — so the sort/dedup couldn't collapse
        // them (six TS2300 for three declarations). Both phases now span the
        // bracket-inclusive name node with the raw `[0]` message arg, so identical
        // diagnostics collapse: three declarations -> three TS2300.
        let result = check("class C { [0]() {} [0] = 1; [0] = 2; }");
        let ts2300: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.code == 2300)
            .collect();
        assert_eq!(ts2300.len(), 3);
        assert!(ts2300.iter().all(|d| d.args == vec!["[0]".to_string()]));
        assert_eq!(
            ts2300
                .iter()
                .map(|d| (d.span.start, d.span.end))
                .collect::<Vec<_>>(),
            vec![(10, 13), (19, 22), (28, 31)]
        );
    }

    #[test]
    fn private_name_bind_and_check_display_collapse() {
        // A duplicate private member (`#x`) is reported by BOTH the bind cascade and
        // the check pass. Both point at the same `#name` span with code 2300, but the
        // bind side once built its message arg WITHOUT the leading `#` (bare `x`) while
        // the check side built it WITH (`#x`), so the differing args blocked sort/dedup
        // — a latent extra (six TS2300 for three declarations). Both phases now carry
        // the `#x` form (matching tsgo's `Duplicate identifier '#foo'.`), so identical
        // diagnostics collapse: three declarations -> three TS2300.
        let result = check("class C { #x = 1; get #x() { return 1; } #x() {} }");
        let ts2300: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.code == 2300)
            .collect();
        let summary: Vec<_> = ts2300
            .iter()
            .map(|d| (d.args.clone(), d.span.start, d.span.end))
            .collect();
        // Every private-name TS2300 carries the `#x` arg (with the `#`), not bare `x`.
        assert!(
            ts2300.iter().all(|d| d.args == vec!["#x".to_string()]),
            "expected all private-name TS2300 args to be `#x`, got {summary:?}",
        );
        // No duplicated (code, span) pair survives dedup — the latent extra is closed.
        let mut spans: Vec<_> = ts2300.iter().map(|d| (d.span.start, d.span.end)).collect();
        let n = spans.len();
        spans.sort_unstable();
        spans.dedup();
        assert_eq!(
            spans.len(),
            n,
            "a duplicated (code, span) private-name TS2300 remains: {summary:?}",
        );
        assert_eq!(
            n, 3,
            "expected three deduped private-name TS2300: {summary:?}"
        );
    }

    #[test]
    fn nested_type_literal_method_property_conflict_binds() {
        // A nested type literal's method-vs-property conflict is bind-time; it was
        // missed at depth >= 1 because a property signature's own type annotation
        // never descended. The property/property nested dup was always caught (the
        // check pass recurses at any depth) — this closes only the bind-time family
        // gap. Depth-0 control and the nested case both fire two TS2300.
        let ts2300 = |source: &str| {
            check(source)
                .diagnostics
                .iter()
                .filter(|d| d.code == 2300)
                .count()
        };
        assert_eq!(ts2300("var a: { m(): void; m: number };"), 2);
        assert_eq!(ts2300("var a: { outer: { m(): void; m: number } };"), 2);
    }
}
