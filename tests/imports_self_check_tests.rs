//! The import-record self-check over the main fixture tree — pure Rust, sidecar-free, so it
//! runs in every `cargo test --workspace`.
//!
//! Every TypeScript and Svelte fixture under `tests/fixtures/` whose `expected.json` is the
//! canonical parser's tree of the input (acorn-typescript's, or Svelte's `parse`, span-only)
//! is free oracle input: the import fixtures' ESTree walk (`imports_fixtures::oracle_json`,
//! which reads that tree alone, never tsv's AST) computes the records from it, and
//! `tsv_ts::parse_imports_with_goal` (at the fixture's `goal`) or `tsv_svelte::parse_imports`
//! must return the same, through the bindings' JSON writer. An `expected_<stem>.json`
//! variant pin is the canonical tree of its sibling `<stem>.*` and is graded the same way.
//!
//! Skipped, and counted: CSS fixtures; a fixture with `expected_ours.json` (a parser
//! divergence — the canonical tree is not of tsv's parse, so neither is its variant pins');
//! a `tsv_rejects.txt` fixture (tsv does not parse the input). A record-free tree still
//! grades `hasModuleSyntax`, so it counts as compared.
//!
//! The walk fails loudly on a tree it cannot read (a `TS*` node reached through a key it
//! does not know holds a type, an `assert` clause, a field missing): that is a failure here,
//! never a skip. The floors below keep the check from passing over nothing.

use std::path::Path;
use tsv_debug::fixtures::{Fixture, FixtureFiles, InputType, walk_fixtures};
use tsv_debug::imports_fixtures::{ImportsLanguage, oracle_json, ours_json_at};

/// The fewest trees the check must compare — far below the tree's count, so it trips only
/// when discovery or a skip rule has gone wrong.
const COMPARED_MIN: usize = 4500;

/// The fewest records the compared trees must hold between them.
const RECORDS_MIN: usize = 500;

#[derive(Default)]
struct Tally {
    compared: usize,
    records: usize,
    variant_pins: usize,
    skipped_css: usize,
    skipped_divergence: usize,
    skipped_tsv_rejects: usize,
    failures: Vec<String>,
}

#[test]
fn test_imports_match_the_canonical_fixture_trees() {
    let tally = match tsv_lang::run_with_stack(run).expect("spawning the sized-stack thread") {
        Ok(tally) => tally,
        Err(e) => panic!("the fixture tree is malformed: {e}"),
    };
    let Tally {
        compared,
        records,
        variant_pins,
        skipped_css,
        skipped_divergence,
        skipped_tsv_rejects,
        failures,
    } = tally;
    eprintln!(
        "imports self-check: {compared} trees compared ({variant_pins} variant pins), \
         {records} records; skipped {skipped_css} CSS, {skipped_divergence} parser \
         divergence, {skipped_tsv_rejects} tsv_rejects"
    );
    assert!(
        failures.is_empty(),
        "{} failure(s) over {compared} compared tree(s):\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
    assert!(
        compared >= COMPARED_MIN,
        "only {compared} trees compared (floor {COMPARED_MIN}): discovery or a skip rule broke"
    );
    assert!(
        records >= RECORDS_MIN,
        "only {records} records compared (floor {RECORDS_MIN}): the walk or the trees changed"
    );
}

fn run() -> Result<Tally, String> {
    let fixtures = walk_fixtures(Path::new("tests/fixtures"))?;
    let mut tally = Tally::default();
    for fixture in &fixtures {
        let language = match fixture.input_type() {
            InputType::Css => {
                tally.skipped_css += 1;
                continue;
            }
            InputType::Svelte => ImportsLanguage::Svelte,
            InputType::TypeScript | InputType::SvelteTs => ImportsLanguage::TypeScript,
        };
        if fixture.has_expected_ours() {
            tally.skipped_divergence += 1;
            continue;
        }
        let files = FixtureFiles::scan(fixture);
        if files.tsv_rejects {
            tally.skipped_tsv_rejects += 1;
            continue;
        }
        grade(
            &mut tally,
            fixture,
            language,
            &fixture.input_file,
            "expected.json",
        );
        for pin in &files.expected_variant {
            tally.variant_pins += 1;
            grade(&mut tally, fixture, language, &pin.variant, &pin.pin);
        }
    }
    Ok(tally)
}

/// Grade the source `input` against the canonical tree `tree`, both in `fixture`'s
/// directory.
fn grade(tally: &mut Tally, fixture: &Fixture, language: ImportsLanguage, input: &str, tree: &str) {
    let name = format!("{}/{input}", fixture.relative_path);
    let read = |file: &str| {
        let path = fixture.path.join(file);
        std::fs::read_to_string(&path).map_err(|e| format!("reading {}: {e}", path.display()))
    };
    let goal = match language {
        ImportsLanguage::TypeScript => Some(fixture.goal()),
        ImportsLanguage::Svelte => None,
    };
    let outcome = (|| {
        let source = read(input)?;
        let wire = tsv_debug::json::from_str(&read(tree)?)
            .map_err(|e| format!("{tree} does not parse: {e}"))?;
        let oracle = oracle_json(language, &source, &wire).map_err(|e| format!("oracle: {e}"))?;
        let ours = ours_json_at(language, &source, goal).map_err(|e| format!("tsv: {e}"))?;
        Ok::<_, String>((oracle, ours))
    })();
    match outcome {
        Ok((oracle, ours)) => {
            tally.compared += 1;
            tally.records += oracle["imports"].as_array().map_or(0, Vec::len);
            if oracle != ours {
                tally.failures.push(format!(
                    "{name}: records differ from {tree}'s\n  oracle: {oracle}\n  tsv:    {ours}"
                ));
            }
        }
        Err(e) => tally.failures.push(format!("{name}: {e}")),
    }
}
