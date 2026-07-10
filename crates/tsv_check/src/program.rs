//! Program pipeline assembly: parse (goal rule) -> bind -> check -> sort/dedup.
//!
//! Ports the shape of tsgo's `GetDiagnosticsOfAnyProgram`, in particular the
//! **parse-error short-circuit**: when any unit fails to parse, the program
//! emits no bind/check diagnostics at all (tsgo `program.go:1770` — the
//! syntactic-append-added-nothing guard). This matters because tsv's parser is
//! deliberately permissive: on a program tsgo parse-rejects, tsv can parse
//! clean and would otherwise emit family diagnostics the baseline lacks. tsv's
//! suppression mirrors the short-circuit exactly — a parse rejection anywhere
//! yields zero semantic output for the program.
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
// tsgo: internal/compiler/program.go GetDiagnosticsOfAnyProgram (:1755),
//       the syntactic short-circuit at :1770; the bind-then-check concat at
//       getBindAndCheckDiagnosticsWithChecker (:1337); final sort+dedup is
//       caller-side (execute/tsc/emit.go:120).

use crate::binder::{bind_file, module_ness, ModuleNess};
use crate::diag::{sort_and_deduplicate, Diagnostic};
use crate::ids::FileId;
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
/// per-unit report, and whether the parse-error short-circuit fired.
pub struct CheckResult {
    /// Diagnostics in canonical sorted order — always empty at this slice (the
    /// checker is a no-op), and empty by construction whenever `parse_rejected`.
    pub diagnostics: Vec<Diagnostic>,
    /// Per-unit parse/bind report, in input order.
    pub files: Vec<FileReport>,
    /// Whether any unit parse-rejected (the program short-circuit fired).
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

    // The parse-error short-circuit: any rejection -> no bind/check diagnostics.
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    if !parse_rejected {
        for attempt in &mut attempts {
            if let Some(program) = &attempt.program {
                let bound = bind_file(program, attempt.file);
                attempt.node_count = bound.node_count;
                // Per file: bind diagnostics then check diagnostics (both empty
                // at this slice) — the getBindAndCheckDiagnostics concat.
                let check_diags = check_file(&bound);
                diagnostics.extend(bound.diagnostics);
                diagnostics.extend(check_diags);
            }
        }
        // Final caller-side sort + dedup over the whole program's diagnostics.
        let paths: Vec<String> = units.iter().map(|u| u.name.to_string()).collect();
        sort_and_deduplicate(&mut diagnostics, &paths);
    }

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
    fn parse_rejection_short_circuits() {
        // A hard syntax error both goals reject.
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
    fn multi_unit_short_circuit_is_program_wide() {
        // One clean unit, one rejected unit -> the whole program short-circuits.
        let arena = Bump::new();
        let result = check_program(
            &[SourceUnit::new("a.ts", "const x = 1;"), SourceUnit::new("b.ts", "const = ;")],
            &arena,
        );
        assert!(result.parse_rejected);
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.files.len(), 2);
        assert!(matches!(result.files[0].parse, ParseReport::Parsed(_)));
        assert!(matches!(result.files[1].parse, ParseReport::Rejected { .. }));
    }
}
