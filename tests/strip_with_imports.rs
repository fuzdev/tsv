//! The combined entry, `tsv_ts::strip_with_imports`, against its two halves: over every
//! TypeScript input of the import and strip fixture trees, at the unset goal and at each
//! named one, the pair is exactly `(strip(source), parse_imports(source))` wherever the
//! strip accepts the source, and the strip's own error wherever it does not. The warm
//! primitives beneath the two halves (`__private::strip_into` and
//! `__private::parse_imports_into`, one `WarmStores` rewound between sources, as the bindings
//! keep theirs) return the same.

use std::path::{Path, PathBuf};

use tsv_ts::__private::{WarmStores, parse_imports_into, strip_into};
use tsv_ts::{
    Goal, parse_imports, parse_imports_with_goal, strip, strip_with_goal, strip_with_imports,
    strip_with_imports_with_goal,
};

const ROOTS: [&str; 2] = ["tests/fixtures_imports", "tests/fixtures_strip"];

/// Every TypeScript fixture input under `dir` (`.ts`, `.mts` and `.svelte.ts`; a Svelte
/// component is no input of the TypeScript strip), sorted.
fn typescript_inputs(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            typescript_inputs(&path, out)?;
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.starts_with("input") && (name.ends_with(".ts") || name.ends_with(".mts")) {
            out.push(path);
        }
    }
    Ok(())
}

#[test]
fn the_combined_entry_is_its_two_halves() {
    tsv_ts::run_with_stack(|| {
        let mut inputs = Vec::new();
        for root in ROOTS {
            if let Err(error) = typescript_inputs(Path::new(root), &mut inputs) {
                panic!("reading {root}: {error}");
            }
        }
        inputs.sort();
        assert!(!inputs.is_empty(), "no TypeScript fixture inputs");
        let mut stores = WarmStores::default();
        let (mut accepted, mut refused) = (0, 0);
        for path in &inputs {
            let source = match std::fs::read_to_string(path) {
                Ok(source) => source,
                Err(error) => panic!("reading {}: {error}", path.display()),
            };
            let name = path.display();
            let combined = strip_with_imports(&source);
            match strip(&source) {
                Ok(stripped) => {
                    accepted += 1;
                    let imports = match parse_imports(&source) {
                        Ok(imports) => imports,
                        Err(error) => panic!("{name}: strips but does not parse: {error:#}"),
                    };
                    assert_eq!(combined, Ok((stripped, imports)), "{name}");
                }
                Err(error) => {
                    refused += 1;
                    assert_eq!(combined, Err(error), "{name}");
                }
            }
            assert_eq!(
                stores.with(|stores| strip_into(&source, None, stores)),
                strip(&source),
                "{name}: the warm strip"
            );
            assert_eq!(
                stores.with(|stores| parse_imports_into(&source, None, stores)),
                parse_imports(&source),
                "{name}: the warm imports"
            );
            for goal in [Goal::Module, Goal::Script] {
                let expected = strip_with_goal(&source, goal).map(|stripped| {
                    let imports = match parse_imports_with_goal(&source, goal) {
                        Ok(imports) => imports,
                        Err(error) => {
                            panic!("{name}: strips at {goal:?} but does not parse: {error:#}")
                        }
                    };
                    (stripped, imports)
                });
                assert_eq!(
                    strip_with_imports_with_goal(&source, goal),
                    expected,
                    "{name} at {goal:?}"
                );
            }
        }
        // both verdicts are graded: the trees hold sources the strip refuses
        assert!(
            accepted > 0 && refused > 0,
            "{accepted} accepted, {refused} refused"
        );
    })
    .expect("the test thread spawns");
}
