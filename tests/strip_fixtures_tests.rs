//! The strip fixture gate (`tests/fixtures_strip/`) — pure Rust, so it runs in every
//! `cargo test --workspace`.
//!
//! Per fixture, `tsv_ts::strip` must produce the committed `expected.*` (or the refusal in
//! `refusal.txt`), keep every surviving character's line and UTF-16 column and every line
//! terminator, and be idempotent (stripping its output returns it borrowed). Known failures
//! are pinned in the ratchet snapshot
//! `crates/tsv_debug/src/cli/commands/strip_fixtures_known.txt`: a new failure fails this
//! test, and so does a pinned one that stopped failing (re-pin with
//! `deno task strip:fixtures:validate:update`). A broken fixture or a panic always fails.
//!
//! The committed files' agreement with the oracles (ts-blank-space, Node's
//! `module.stripTypeScriptTypes`) is `deno task strip:fixtures:oracle`'s, which needs
//! node_modules and so stays out of this test.

use tsv_debug::strip_fixtures::{REPIN_HINT, run_gate};

#[test]
fn test_all_strip_fixtures() {
    let report = match run_gate(&[]) {
        Ok(report) => report,
        Err(e) => panic!("strip fixture gate could not run: {e}"),
    };
    let mut problems = Vec::new();
    for finding in &report.findings {
        if !finding.key.check.is_pinnable() || report.new.contains(&finding.key) {
            problems.push(format!("{}: {}", finding.key, finding.detail));
        }
    }
    for key in &report.stale {
        problems.push(format!("STALE pin (no longer fails): {key}"));
    }
    assert!(
        report.holds(),
        "{} strip fixture(s), {} known failure(s); the ratchet does not hold — re-pin with \
         `{REPIN_HINT}` once the change is deliberate:\n{}",
        report.fixtures,
        report.known,
        problems.join("\n")
    );
}
