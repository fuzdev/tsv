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

use crate::binder::{bind_file, module_ness, ModuleNess};
use crate::diag::{sort_and_deduplicate, Diagnostic};
use crate::ids::FileId;
use crate::merge::{merge_program, FileMerge};
use bumpalo::Bump;
use tsv_ts::ast::Program;
use tsv_ts::{parse_with_goal, Goal};

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

/// The result of checking a program: its (sorted, deduped) diagnostics, a
/// per-unit report, and whether any unit parse-rejected.
pub struct CheckResult {
    /// Diagnostics in canonical sorted order — the unconditional concat of every
    /// unit that parsed (a rejected unit contributes none, having no AST).
    pub diagnostics: Vec<Diagnostic>,
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

/// Check a program: parse every unit via the goal rule, and — unless any unit
/// parse-rejects — bind and check each, concatenating diagnostics and returning
/// them in canonical sorted order.
#[must_use]
pub fn check_program<'a>(units: &[SourceUnit<'a>], arena: &'a Bump) -> CheckResult {
    let mut attempts: Vec<Attempt<'a>> = Vec::with_capacity(units.len());
    let mut parse_rejected = false;

    for (i, unit) in units.iter().enumerate() {
        let file = FileId(i as u32);
        match parse_unit(unit.source, arena) {
            Ok((program, goal, used_script_retry)) => attempts.push(Attempt {
                file,
                name: unit.name,
                goal,
                used_script_retry,
                module_ness: module_ness(&program),
                program: Some(program),
                message: None,
                node_count: 0,
            }),
            Err(message) => {
                parse_rejected = true;
                attempts.push(Attempt {
                    file,
                    name: unit.name,
                    goal: Goal::Module,
                    used_script_retry: false,
                    module_ness: ModuleNess::Script,
                    program: None,
                    message: Some(message),
                    node_count: 0,
                });
            }
        }
    }

    // Unconditional concat (the CompileFilesEx parity path): every unit that
    // parsed contributes its bind/check diagnostics, independent of a sibling's
    // rejection. A rejected unit has no AST, so it contributes none.
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let mut merges: Vec<FileMerge> = Vec::new();
    for attempt in &mut attempts {
        if let Some(program) = &attempt.program {
            let source = units[attempt.file.index()].source;
            let bound = bind_file(program, source, attempt.file);
            attempt.node_count = bound.node_count;
            // Per file: bind diagnostics then check diagnostics (check is a no-op
            // this slice) — the getBindAndCheckDiagnostics concat.
            let check_diags = check_file(&bound);
            diagnostics.extend(bound.diagnostics);
            diagnostics.extend(check_diags);
            merges.push(bound.merge);
        }
    }
    // The single-threaded global merge (checker-init phase) over every parsed
    // file's bind product — cross-declaration-space conflicts, the
    // globalThis/undefined checks, and module augmentations. Its diagnostics join
    // the pool before the canonical sort (order-independent).
    diagnostics.extend(merge_program(&merges));

    // Final caller-side sort + dedup over the whole program's diagnostics.
    let paths: Vec<String> = units.iter().map(|u| u.name.to_string()).collect();
    sort_and_deduplicate(&mut diagnostics, &paths);

    let files = attempts.into_iter().map(Attempt::into_report).collect();
    CheckResult { diagnostics, files, parse_rejected }
}

/// Check one bound file — a no-op skeleton (no semantic diagnostics yet).
fn check_file(bound: &crate::binder::BoundFile) -> Vec<Diagnostic> {
    // The checker is not built yet; the seam exists so the pipeline is proven
    // end-to-end. The bound columns are available here for the future checker.
    let _ = bound;
    Vec::new()
}

/// Parse a unit via the goal rule: `Module` first, `Script` on failure. Returns
/// the program, the goal it parsed under, and whether the `Script` retry won; on
/// double failure returns the `Module`-goal error message.
fn parse_unit<'a>(
    source: &'a str,
    arena: &'a Bump,
) -> Result<(Program<'a>, Goal, bool), String> {
    match parse_with_goal(source, Goal::Module, arena) {
        Ok(program) => Ok((program, Goal::Module, false)),
        Err(module_err) => match parse_with_goal(source, Goal::Script, arena) {
            Ok(program) => Ok((program, Goal::Script, true)),
            // Both goals failed: report the primary (Module) goal's error.
            Err(_script_err) => Err(module_err.to_string()),
        },
    }
}

/// The mutable per-unit state carried from parse through bind into the report.
struct Attempt<'a> {
    file: FileId,
    name: &'a str,
    goal: Goal,
    used_script_retry: bool,
    module_ness: ModuleNess,
    program: Option<Program<'a>>,
    message: Option<String>,
    node_count: u32,
}

impl Attempt<'_> {
    fn into_report(self) -> FileReport {
        let parse = match self.message {
            Some(message) => ParseReport::Rejected { message },
            None => ParseReport::Parsed(ParsedFacts {
                goal: self.goal,
                used_script_retry: self.used_script_retry,
                module_ness: self.module_ness,
                node_count: self.node_count,
            }),
        };
        FileReport { file: self.file, name: self.name.to_string(), parse }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(source: &str) -> CheckResult {
        let arena = Bump::new();
        check_program(&[SourceUnit::new("test.ts", source)], &arena)
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
        assert!(matches!(result.files[0].parse, ParseReport::Rejected { .. }));
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
            &[SourceUnit::new("a.ts", "let x; let x;"), SourceUnit::new("b.ts", "const = ;")],
            &arena,
        );
        assert!(result.parse_rejected);
        // a.ts's two TS2451 survive despite b.ts rejecting.
        assert_eq!(result.diagnostics.len(), 2);
        assert!(result.diagnostics.iter().all(|d| d.code == 2451));
        assert!(matches!(result.files[0].parse, ParseReport::Parsed(_)));
        assert!(matches!(result.files[1].parse, ParseReport::Rejected { .. }));
    }
}
