//! Every committed prettier-chain signature is byte-for-byte its canonical serialization —
//! pure Rust, sidecar-free, so it runs in every `cargo test --workspace`.
//!
//! `AuditSignature::parse` skips the `#` header and ignores `%%PASS=N%%` numbers, and the
//! validator's chain check compares parsed passes, so a change to what
//! `AuditSignature::serialize` writes (the header text, the numbering, the section spacing)
//! would otherwise leave every committed `audit_signature*.txt` stale with no gate failing.
//! Each file is parsed and re-serialized under the anchor its filename names
//! (`ChainAnchor::for_filename`), and must come back unchanged. The repair is
//! `npm run fixtures:update:formatted -- <pattern>`, which rewrites any byte-different file.
//!
//! The walk is a plain directory walk, independent of fixture discovery, so a signature in a
//! directory discovery would skip is still graded. The floors keep it from passing over
//! nothing.

use std::fs;
use std::path::Path;
use tsv_debug::fixtures::{AuditSignature, ChainAnchor};

/// The fewest `audit_signature.txt` files the walk must grade — far below the tree's count,
/// so it trips only when the walk or the filename rule has gone wrong.
const OUTPUT_PRETTIER_MIN: usize = 100;

/// The fewest `audit_signature_<suffix>.txt` files the walk must grade.
const UNFORMATTED_OURS_MIN: usize = 50;

#[derive(Default)]
struct Tally {
    output_prettier: usize,
    unformatted_ours: usize,
    failures: Vec<String>,
}

#[test]
fn test_audit_signatures_are_canonical() {
    let mut tally = Tally::default();
    if let Err(e) = walk(Path::new("tests/fixtures"), &mut tally) {
        tally.failures.push(e);
    }
    let Tally {
        output_prettier,
        unformatted_ours,
        failures,
    } = tally;
    assert!(
        failures.is_empty(),
        "{} signature file(s) failed (a non-canonical file regenerates with \
         npm run fixtures:update:formatted -- <pattern>; a misnamed or unparseable one needs a \
         look):\n\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(
        output_prettier >= OUTPUT_PRETTIER_MIN,
        "only {output_prettier} audit_signature.txt graded (floor {OUTPUT_PRETTIER_MIN})"
    );
    assert!(
        unformatted_ours >= UNFORMATTED_OURS_MIN,
        "only {unformatted_ours} audit_signature_<suffix>.txt graded (floor {UNFORMATTED_OURS_MIN})"
    );
}

/// Grade every signature under `dir`; `Err` when a directory can't be read.
fn walk(dir: &Path, tally: &mut Tally) -> Result<(), String> {
    let read_error = |e: std::io::Error| format!("reading {}: {e}", dir.display());
    for entry in fs::read_dir(dir).map_err(read_error)? {
        let path = entry.map_err(read_error)?.path();
        if path.is_dir() {
            walk(&path, tally)?;
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with("audit_signature") {
            continue;
        }
        let shown = path.display();
        let Some(anchor) = ChainAnchor::for_filename(name) else {
            tally
                .failures
                .push(format!("{shown}: names no signature anchor"));
            continue;
        };
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(e) => {
                tally.failures.push(format!("{shown}: {e}"));
                continue;
            }
        };
        let signature = match AuditSignature::parse(&raw) {
            Ok(signature) => signature,
            Err(e) => {
                tally.failures.push(format!("{shown}: {e}"));
                continue;
            }
        };
        match anchor {
            ChainAnchor::OutputPrettier => tally.output_prettier += 1,
            ChainAnchor::UnformattedOurs => tally.unformatted_ours += 1,
        }
        if let Some(canonical) = signature.non_canonical(&raw, anchor) {
            tally.failures.push(format!(
                "{shown}: first differing line {}",
                first_differing_line(&raw, &canonical)
            ));
        }
    }
    Ok(())
}

/// The 1-based line where `recorded` first departs from `canonical`, with both lines shown.
fn first_differing_line(recorded: &str, canonical: &str) -> String {
    let mut recorded_lines = recorded.lines();
    let mut canonical_lines = canonical.lines();
    let mut line = 1;
    loop {
        match (recorded_lines.next(), canonical_lines.next()) {
            (Some(r), Some(c)) if r == c => line += 1,
            (r, c) => {
                return format!(
                    "{line}\n  recorded:  {}\n  canonical: {}",
                    r.map_or_else(|| "<end of file>".to_string(), |r| format!("{r:?}")),
                    c.map_or_else(|| "<end of file>".to_string(), |c| format!("{c:?}")),
                );
            }
        }
    }
}
