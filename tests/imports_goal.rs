// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::panic)]

//! `tsv_ts::parse_imports`'s parse goal: the unset goal is the strip's
//! (`tsv_ts::__private::parse_with_goal_or_fallback` — a module, retried as a script only
//! if that fails), and `parse_imports_with_goal` names one exactly. The import fixtures
//! read every input at the unset goal, so they cannot pin a named one.

use bumpalo::Bump;
use tsv_ts::__private::parse_imports_in;
use tsv_ts::{Goal, ImportRecordKind, Imports, parse_imports, parse_imports_with_goal};

fn imports(source: &str, goal: Option<Goal>) -> Imports {
    let result = match goal {
        Some(goal) => parse_imports_with_goal(source, goal),
        None => parse_imports(source),
    };
    match result {
        Ok(imports) => imports,
        Err(error) => panic!("expected {source:?} to parse at {goal:?}: {error:#}"),
    }
}

/// A script-only source fails at `Module` and parses at `Script`; the unset goal reaches
/// it through the retry, with the same records.
#[test]
fn the_unset_goal_retries_a_script() {
    let source = "with (o) { import('./w.js'); }\n";
    assert!(parse_imports_with_goal(source, Goal::Module).is_err());
    let script = imports(source, Some(Goal::Script));
    assert_eq!(imports(source, None), script);
    let [record] = &script.records[..] else {
        panic!("one record: {script:?}");
    };
    assert_eq!(record.kind, ImportRecordKind::Dynamic);
    assert!(!script.has_module_syntax);
}

/// A module-only source parses at `Module` and fails at `Script`, with no retry from a
/// named goal.
#[test]
fn a_named_goal_takes_no_retry() {
    for source in [
        "import a from './a.js';\n",
        "export * from './a.js';\n",
        "console.log(import.meta.url);\n",
        "await import('./a.js');\n",
    ] {
        assert!(
            parse_imports_with_goal(source, Goal::Script).is_err(),
            "{source:?} parsed at Script"
        );
        let module = imports(source, Some(Goal::Module));
        assert!(module.has_module_syntax, "{source:?}");
        assert_eq!(imports(source, None), module, "{source:?}");
    }
}

/// At `Script` a top-level `await` is an identifier, never module syntax.
#[test]
fn a_script_await_is_a_name() {
    let script = imports("await(import('./a.js'));\n", Some(Goal::Script));
    assert!(!script.has_module_syntax);
    assert_eq!(script.records.len(), 1);
}

/// The warm-arena primitive is the one-shot functions' body.
#[test]
fn the_arena_primitive_matches() {
    let source = "import a from './a.js';\nwith (o) {}\n";
    let arena = Bump::new();
    for goal in [None, Some(Goal::Module), Some(Goal::Script)] {
        let warm = parse_imports_in(source, goal, &arena).ok();
        let one_shot = match goal {
            Some(goal) => parse_imports_with_goal(source, goal).ok(),
            None => parse_imports(source).ok(),
        };
        assert_eq!(warm, one_shot, "{goal:?}");
    }
}
