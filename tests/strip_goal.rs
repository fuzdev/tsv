// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::panic)]

//! The type strip's parse goal: `tsv_ts::strip_with_goal(source, goal, arena)`.
//!
//! The strip reads the goal axis as the format path does (`tsv_ts::parse_with_goal_or_fallback`):
//! a named goal is exact, and `None` — what `tsv_ts::strip` passes — parses at `Module` and
//! retries at `Script` only if that fails. The retry's verdicts and error attribution are
//! the unit tests' (`strip/mod.rs`); this file pins the named goals, which no fixture can
//! (the strip fixtures carry no goal marker, and their JS-validity guard parses a module).

use bumpalo::Bump;
use tsv_ts::{Goal, StripError, strip, strip_with_goal};

fn stripped(source: &str, goal: Option<Goal>) -> String {
    let arena = Bump::new();
    match strip_with_goal(source, goal, &arena) {
        Ok(stripped) => stripped.code.into_owned(),
        Err(error) => panic!(
            "expected {source:?} to strip at {goal:?}: {}",
            error.message(source)
        ),
    }
}

/// The parse error the strip reports at `goal`, beside the one the parse at `parsed_at`
/// reports, which it must be.
fn parse_errors(source: &str, goal: Option<Goal>, parsed_at: Goal) -> (String, String) {
    let arena = Bump::new();
    let error = match strip_with_goal(source, goal, &arena) {
        Err(StripError::Parse(error)) => error,
        Err(error) => panic!("expected a parse error for {source:?}: {error}"),
        Ok(stripped) => panic!(
            "expected {source:?} to fail, stripped to {:?}",
            stripped.code
        ),
    };
    let parse_arena = Bump::new();
    let Err(parsed) = tsv_ts::parse_with_goal(source, parsed_at, &parse_arena) else {
        panic!("expected {source:?} to fail at {parsed_at:?}");
    };
    (error.to_string(), parsed.to_string())
}

/// `Script` reads `await` as a name and admits the sloppy-mode productions, with no
/// module attempt first.
#[test]
fn a_named_script_goal_strips_a_sloppy_script() {
    for (source, code) in [
        ("var await: number = 1;\n", "var await         = 1;\n"),
        ("with (o as any) { x; }\n", "with (o       ) { x; }\n"),
        (
            "function await<T>(x: T) { return x }\nawait<number>(1);\n",
            "function await   (x   ) { return x }\nawait        (1);\n",
        ),
    ] {
        assert_eq!(stripped(source, Some(Goal::Script)), code, "{source:?}");
    }
}

/// A named goal is exact: `Module` refuses a script-only source rather than retrying, and
/// `Script` refuses a module-only one — a `.cts` file's `import type` included, which is
/// why the unset goal tries the module first.
#[test]
fn a_named_goal_takes_no_retry() {
    let (stripped, module) = parse_errors("with (a) {}\n", Some(Goal::Module), Goal::Module);
    assert_eq!(stripped, module);
    for source in [
        "import type { A } from 'a';\n",
        "let x: number = 1;\nexport { x };\n",
        "await f() as T;\n",
    ] {
        let (stripped, script) = parse_errors(source, Some(Goal::Script), Goal::Script);
        assert_eq!(stripped, script, "{source:?}");
    }
}

/// `strip` is the unset goal: the same answer as `strip_with_goal(…, None, …)` on a
/// module, a sloppy script, and a source neither grammar takes.
#[test]
fn strip_is_the_unset_goal() {
    for source in [
        "let x: number = 1;\nexport { x };\n",
        "with (o as any) { x; }\n",
        "with (o) {}\nlet = ;\n",
        "enum E { A }\n",
    ] {
        let arena = Bump::new();
        let unset = strip_with_goal(source, None, &arena).map(|s| s.code.into_owned());
        let arena = Bump::new();
        let bare = strip(source, &arena).map(|s| s.code.into_owned());
        assert_eq!(unset, bare, "{source:?}");
    }
}
