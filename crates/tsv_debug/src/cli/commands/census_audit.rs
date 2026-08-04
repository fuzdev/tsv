use argh::FromArgs;
use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::audit::census::{CensusEntry, CensusMultiset, comment_census};
use crate::audit::ratchet::{
    Ratchet, SnapshotKey, grade_narrowed_strictly, refuse_narrowed_update,
};
use crate::audit::sweep::{
    FIXTURES_FORMATTED_MIN, PristineSweep, check_formatted_min, sweep_pristine,
};
use crate::cli::CliError;

use super::profile::resolve_seed_files;

/// The comment CENSUS: does every comment the author wrote survive formatting?
///
/// Per file, lex the comment trivia off the raw INPUT and the raw formatted OUTPUT with the
/// census scanners (`audit::census` — never `parse().comments`, which inherits exactly the
/// registration holes this audit exists to check) and compare the per-line-trimmed interior
/// MULTISETS, per language bucket (`ts` / `css` / `template`, Svelte islands lexed with their
/// own language). A comment interior missing from the output is a DROPPED comment no matter
/// which internal layer lost it — parse-time consumption included, the class the print-once
/// ledger is structurally blind to (a comment the parser never registered never existed as far
/// as the ledger knows). An interior the output holds that the input did not is a duplicated
/// or fabricated one.
///
/// Whole-comment drops are sanctioned in exactly ONE place — the CSS CDO/CDC `<!-- ... -->`
/// span, which tsv (matching `parseCss`) discards wholesale — and that carve-out lives in the
/// scanner itself, so a finding here is always a bug. Rejected inputs make no format claim and
/// are skipped.
///
/// Graded as a RATCHET over `census_audit_known.txt`, keyed `(path, bucket, direction)` —
/// file-level, like the compile validation ratchet. Every line is a known bug; the file
/// shrinking is the goal. Born EMPTY over `tests/fixtures` (the CSS parse-time-drop class it
/// was argued from was fixed by hand before it landed); its standing role there is the
/// tripwire that keeps whole-comment conservation closed, and its discovery yield is external
/// corpora — its first sweep over the prettier suites found a live `as const` code swallow
/// and four line-comment merges no other gate could see.
///
/// Pure Rust — no Deno. Defaults to `tests/fixtures` when no paths are given.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "census_audit")]
pub struct CensusAuditCommand {
    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// regenerate the ratchet snapshot (refused on a narrowed run)
    #[argh(switch)]
    update: bool,

    /// file paths, directories, or glob patterns (default: tests/fixtures)
    #[argh(positional)]
    paths: Vec<String>,
}

const SNAPSHOT_HEADER: &str = "\
# Comment-census ratchet — every line is a KNOWN BUG, the file shrinking is the goal.
#
# One line per (file, language bucket, direction): a comment interior present in the raw
# INPUT lex but not the raw OUTPUT lex (MISSING — a dropped comment, whichever internal
# layer lost it), or present in the output but not the input (EXTRA — a duplicated or
# fabricated one). Interiors compare as per-line-trimmed multisets, so a re-indented
# multi-line block matches; everything else is byte-exact. The one sanctioned drop (the
# CSS CDO/CDC `<!-- ... -->` span, discarded wholesale to match parseCss) is carved out
# in the scanner and can never appear here.
#
# A key found but not pinned FAILS (a new loss site). A pinned key that no longer fires
# FAILS (fix landed — re-pin).
#
# Regenerate with `deno task census:audit:update`.
";

const REPIN_HINT: &str = "deno task census:audit:update";

/// Which way a file's multiset moved.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Direction {
    /// In the input, not the output: a dropped comment.
    Missing,
    /// In the output, not the input: a duplicated or fabricated comment.
    Extra,
}

impl Direction {
    const fn name(self) -> &'static str {
        match self {
            Direction::Missing => "MISSING",
            Direction::Extra => "EXTRA",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "MISSING" => Some(Direction::Missing),
            "EXTRA" => Some(Direction::Extra),
            _ => None,
        }
    }
}

/// One ratchet line: a file × language bucket × direction with a known delta.
///
/// File-level by design — the census's reproducer IS the file (re-run the audit on it), and a
/// content-bearing key would churn on every edit to a pinned fixture. Coarser than one line
/// per lost comment: a second drop in an already-pinned (file, bucket, direction) is invisible
/// until the first is fixed, the same trade every ratchet key makes (the key is the shape,
/// not a count).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct CensusKey {
    path: String,
    bucket: String,
    direction: Direction,
}

impl SnapshotKey for CensusKey {
    fn to_line(&self) -> String {
        format!("{}\t{}\t{}", self.path, self.bucket, self.direction.name())
    }

    fn from_line(line: &str) -> Option<Self> {
        let mut cols = line.split('\t');
        let path = cols.next()?.to_string();
        let bucket = cols.next()?.to_string();
        let direction = Direction::parse(cols.next()?)?;
        cols.next().is_none().then_some(Self {
            path,
            bucket,
            direction,
        })
    }

    /// Every census key is pinnable — a lost comment is always a bug, never an absolute
    /// invariant like gap's PANIC. (A format that PANICS is counted separately and not
    /// gated here; the panic gates own that class.)
    fn is_pinnable(&self) -> bool {
        true
    }
}

/// One multiset imbalance: an interior class whose input and output counts disagree.
struct Delta {
    entry: CensusEntry,
    input_count: usize,
    output_count: usize,
}

impl Delta {
    const fn direction(&self) -> Direction {
        if self.output_count < self.input_count {
            Direction::Missing
        } else {
            Direction::Extra
        }
    }
}

/// A file whose input and output comment multisets disagree.
struct CensusFinding {
    path: PathBuf,
    /// The imbalances, in `CensusEntry` order. Non-empty by construction — the one push
    /// site guards on it.
    deltas: Vec<Delta>,
}

impl CensusAuditCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let default_paths = self.paths.is_empty();
        let narrowed: &[&'static str] = if default_paths {
            &[]
        } else {
            &["explicit paths"]
        };
        refuse_narrowed_update(
            self.update,
            narrowed,
            "the census keys over tests/fixtures",
            "SUBSET",
        )?;
        let files = resolve_seed_files(&self.paths, 0)?;
        let sweep = sweep_files(&files);

        if self.json {
            print_json(&sweep);
        } else {
            print_report(&sweep);
        }
        sweep.pristine.print_panic_sample();

        if default_paths {
            check_formatted_min(sweep.pristine.formatted, FIXTURES_FORMATTED_MIN)?;
        }

        let ratchet = ratchet();
        if self.update {
            ratchet.write_pinned(&sweep.keys, "key")?;
            return Ok(());
        }
        // Off the default corpus the snapshot doesn't apply — it pins the full default run,
        // so grading a narrowed one would call every unreached key stale. Every finding is
        // news instead.
        if !default_paths {
            return grade_narrowed_strictly(narrowed, "census delta", sweep.findings.len());
        }

        ratchet.grade_and_report(
            &sweep.keys,
            "census key",
            &format!("{} files", sweep.pristine.formatted),
            |key| format!("{} [{}] {}", key.path, key.bucket, key.direction.name()),
        )
    }
}

/// What one corpus walk produced.
struct Sweep {
    /// Files with a multiset imbalance, in walk order — the human report.
    findings: Vec<CensusFinding>,
    /// Every `(path, bucket, direction)` seen — what the ratchet grades.
    keys: BTreeSet<CensusKey>,
    /// The shared skip/format bookkeeping (the [`check_formatted_min`] vacuity
    /// guard reads `formatted`; panics are counted there, not gated here).
    pristine: PristineSweep,
}

/// Format every file (via the shared pristine sweep) and compare the two censuses.
fn sweep_files(files: &[PathBuf]) -> Sweep {
    let mut findings = Vec::new();
    let mut keys = BTreeSet::new();
    let pristine = sweep_pristine(files, |path, parser, source, output| {
        let input_census = comment_census(source, parser);
        let output_census = comment_census(output, parser);
        let deltas = diff_censuses(&input_census, &output_census);
        if deltas.is_empty() {
            return;
        }
        let path_key = path.display().to_string();
        for delta in &deltas {
            keys.insert(CensusKey {
                path: path_key.clone(),
                bucket: delta.entry.bucket.name().to_string(),
                direction: delta.direction(),
            });
        }
        findings.push(CensusFinding {
            path: path.to_path_buf(),
            deltas,
        });
    });
    Sweep {
        findings,
        keys,
        pristine,
    }
}

/// The multiset comparison: every interior class whose counts disagree, in entry order.
fn diff_censuses(input: &CensusMultiset, output: &CensusMultiset) -> Vec<Delta> {
    let mut deltas = Vec::new();
    for (entry, &input_count) in input {
        let output_count = output.get(entry).copied().unwrap_or(0);
        if output_count != input_count {
            deltas.push(Delta {
                entry: entry.clone(),
                input_count,
                output_count,
            });
        }
    }
    for (entry, &output_count) in output {
        if !input.contains_key(entry) {
            deltas.push(Delta {
                entry: entry.clone(),
                input_count: 0,
                output_count,
            });
        }
    }
    deltas.sort_by(|a, b| a.entry.cmp(&b.entry));
    deltas
}

/// The ratchet over this audit's colocated snapshot, carrying its header + re-pin hint.
fn ratchet() -> Ratchet {
    Ratchet::colocated("census_audit_known.txt", SNAPSHOT_HEADER, REPIN_HINT)
}

/// A one-line, escape-rendered preview of a comment interior, truncated on a char boundary.
fn preview(content: &str) -> String {
    let mut out = String::new();
    for (count, c) in content.chars().enumerate() {
        if count >= 48 {
            out.push('…');
            break;
        }
        out.extend(c.escape_debug());
    }
    out
}

fn print_report(sweep: &Sweep) {
    let Sweep {
        findings, pristine, ..
    } = sweep;
    let formatted = pristine.formatted;
    let skipped = pristine.skipped_note();
    if findings.is_empty() {
        println!("✓ comment censuses balance across {formatted} files ({skipped})");
        return;
    }

    println!(
        "✗ {} file(s) with a comment-census imbalance ({formatted} formatted, {skipped})\n",
        findings.len(),
    );

    for f in findings {
        println!("  {}", f.path.display());
        // The first few localize the loss without re-running; the rest are a count, so one
        // pathological file can't bury the others.
        for d in f.deltas.iter().take(3) {
            println!(
                "    {} [{} {}] \"{}\" (input ×{} → output ×{})",
                d.direction().name(),
                d.entry.bucket.name(),
                d.entry.kind.name(),
                preview(&d.entry.content),
                d.input_count,
                d.output_count
            );
        }
        if f.deltas.len() > 3 {
            println!("    (+{} more in this file)", f.deltas.len() - 3);
        }
        println!();
    }
}

fn print_json(sweep: &Sweep) {
    let Sweep {
        findings, pristine, ..
    } = sweep;
    let items: Vec<serde_json::Value> = findings
        .iter()
        .map(|f| {
            let deltas: Vec<serde_json::Value> = f
                .deltas
                .iter()
                .map(|d| {
                    serde_json::json!({
                        "direction": d.direction().name(),
                        "bucket": d.entry.bucket.name(),
                        "kind": d.entry.kind.name(),
                        "content": d.entry.content,
                        "input_count": d.input_count,
                        "output_count": d.output_count,
                    })
                })
                .collect();
            serde_json::json!({
                "path": f.path.to_string_lossy(),
                "deltas": deltas,
            })
        })
        .collect();
    let output = serde_json::json!({
        "formatted": pristine.formatted,
        "parse_skipped": pristine.parse_errors,
        "read_skipped": pristine.read_errors,
        "panicked": pristine.panics.count(),
        "panicked_sample": pristine.panics.sample(),
        "findings": findings.len(),
        "files": items,
    });
    #[allow(clippy::unwrap_used)]
    let s = serde_json::to_string_pretty(&output).unwrap();
    println!("{s}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::census::comment_census;
    use tsv_cli::cli::input::ParserType;

    #[test]
    fn census_key_round_trips() {
        let key = CensusKey {
            path: "tests/fixtures/css/x/input.svelte".to_string(),
            bucket: "css".to_string(),
            direction: Direction::Missing,
        };
        let line = key.to_line();
        assert_eq!(line, "tests/fixtures/css/x/input.svelte\tcss\tMISSING");
        assert_eq!(CensusKey::from_line(&line), Some(key));
        assert_eq!(CensusKey::from_line("a\tb"), None, "two columns");
        assert_eq!(CensusKey::from_line("a\tb\tDROPPED"), None, "bad direction");
        assert_eq!(
            CensusKey::from_line("a\tb\tMISSING\tc"),
            None,
            "extra column"
        );
    }

    #[test]
    fn diff_reports_missing_and_extra_with_counts() {
        let input = comment_census("// a\n// a\n// b\n", ParserType::TypeScript);
        let output = comment_census("// a\n// c\n", ParserType::TypeScript);
        let deltas = diff_censuses(&input, &output);
        let rendered: Vec<(String, &str, usize, usize)> = deltas
            .iter()
            .map(|d| {
                (
                    d.entry.content.clone(),
                    d.direction().name(),
                    d.input_count,
                    d.output_count,
                )
            })
            .collect();
        assert_eq!(
            rendered,
            vec![
                ("a".to_string(), "MISSING", 2, 1),
                ("b".to_string(), "MISSING", 1, 0),
                ("c".to_string(), "EXTRA", 0, 1),
            ]
        );
        assert!(diff_censuses(&input, &input).is_empty());
    }
}
