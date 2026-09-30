use argh::FromArgs;
use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::audit::census::{CensusEntry, CensusMultiset, comment_census};
use crate::audit::ratchet::{
    Ratchet, SnapshotKey, grade_narrowed_strictly, refuse_narrowed_update,
};
use crate::audit::sweep::{PristineSweep, sort_findings_by_path, sweep_pristine};
use crate::cli::CliError;

use super::profile::resolve_seed_files;

/// The comment CENSUS: does every comment the author wrote survive formatting? And, over a
/// Svelte template, every open tag and every `{#…}` / `{@…}` head?
///
/// Per file, lex the comment trivia off the raw INPUT and the raw formatted OUTPUT with the
/// census scanners (`audit::census` — never `parse().comments`, which inherits exactly the
/// registration holes this audit exists to check) and compare the per-line-trimmed interior
/// MULTISETS, per language bucket (`ts` / `css` / `template`, Svelte islands lexed with their
/// own language). A comment interior missing from the output is a DROPPED comment no matter
/// which internal layer lost it — parse-time consumption included, the class the print-once
/// ledger is structurally blind to (a comment the parser never registered never existed as far
/// as the ledger knows). An interior the output holds that the input did not is a duplicated
/// or fabricated one. The Svelte scanner counts the template's open-tag names and block / tag
/// heads (`tag_head` / `block_head` kinds) into the same multiset, so a template node the
/// PARSER consumed — invisible to the wire-vs-wire node census, which only sees what the parser
/// built — is a plain MISSING here.
///
/// Whole-comment drops are sanctioned in exactly ONE place — the CSS CDO/CDC `<!-- ... -->`
/// span, which tsv (matching `parseCss`) discards wholesale — and that carve-out lives in the
/// scanner itself, so a finding here is always a bug. Rejected inputs make no format claim and
/// are skipped.
///
/// Graded as a RATCHET over `census_audit_known.txt`, keyed `(path, bucket, direction)` —
/// file-level, like the compile validation ratchet. Every line is a known bug; the file
/// shrinking is the goal. Born EMPTY over `tests/fixtures`; its standing role there is the
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

    /// worker threads (default: available parallelism). Results are `--jobs`-invariant —
    /// see `audit::sweep`, which restores the serial walk's order exactly
    #[argh(option)]
    jobs: Option<usize>,

    /// file paths, directories, or glob patterns (default: tests/fixtures)
    #[argh(positional)]
    paths: Vec<String>,
}

const SNAPSHOT_HEADER: &str = "\
# Comment-census ratchet — every line is a KNOWN BUG, the file shrinking is the goal.
#
# One line per (file, language bucket, direction): a comment interior — or, in the
# template bucket, an open-tag name or a `{#…}` / `{@…}` head — present in the raw
# INPUT lex but not the raw OUTPUT lex (MISSING — a dropped comment or node, whichever
# internal layer lost it), or present in the output but not the input (EXTRA — a
# duplicated or fabricated one). Interiors compare as per-line-trimmed multisets, so
# a re-indented multi-line block matches; everything else is byte-exact. The one
# sanctioned drop (the CSS CDO/CDC `<!-- ... -->` span, discarded wholesale to match
# parseCss) is carved out in the scanner and can never appear here.
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
        let sweep = sweep_files(&files, self.jobs)?;

        if self.json {
            print_json(&sweep);
        } else {
            print_report(&sweep);
        }
        sweep.pristine.finish(default_paths)?;

        let ratchet = ratchet();
        if self.update {
            ratchet.write_pinned(&sweep.keys, "key", self.json)?;
            return Ok(());
        }
        // Off the default corpus the snapshot doesn't apply — it pins the full default run,
        // so grading a narrowed one would call every unreached key stale. Every finding is
        // news instead.
        if !default_paths {
            return grade_narrowed_strictly(
                narrowed,
                "census delta",
                sweep.findings.len(),
                self.json,
            );
        }

        ratchet.grade_and_report(
            &sweep.keys,
            "census key",
            &format!("{} files", sweep.pristine.formatted),
            self.json,
            |key| format!("{} [{}] {}", key.path, key.bucket, key.direction.name()),
        )
    }
}

/// What one corpus walk produced.
struct Sweep {
    /// Files with a multiset imbalance, in walk order (restored after the per-worker
    /// merge by `sort_findings_by_path`) — the human report.
    findings: Vec<CensusFinding>,
    /// Every `(path, bucket, direction)` seen — what the ratchet grades.
    keys: BTreeSet<CensusKey>,
    /// The shared skip/format bookkeeping (the [`check_formatted_min`](crate::audit::vacuity::check_formatted_min) vacuity
    /// guard reads `formatted`; panics are counted there, not gated here).
    pristine: PristineSweep,
}

/// One worker's share of the walk. `keys` is a `BTreeSet`, so the ratchet's product is
/// order-free by construction; `findings` is the human report's list and gets its order back
/// from the stable path sort in [`sweep_files`].
#[derive(Default)]
struct Tally {
    findings: Vec<CensusFinding>,
    keys: BTreeSet<CensusKey>,
}

/// Format every file (via the shared pristine sweep) and compare the two censuses.
///
/// # Errors
///
/// Returns [`CliError::Failed`] when a sweep worker panics outside the per-file catch.
fn sweep_files(files: &[PathBuf], jobs: Option<usize>) -> Result<Sweep, CliError> {
    let (pristine, mut tally) = sweep_pristine(
        files,
        jobs,
        |path, parser, source, output, tally: &mut Tally| {
            let input_census = comment_census(source, parser);
            let output_census = comment_census(output, parser);
            let deltas = diff_censuses(&input_census, &output_census);
            if deltas.is_empty() {
                return;
            }
            let path_key = path.display().to_string();
            for delta in &deltas {
                tally.keys.insert(CensusKey {
                    path: path_key.clone(),
                    bucket: delta.entry.bucket.name().to_string(),
                    direction: delta.direction(),
                });
            }
            tally.findings.push(CensusFinding {
                path: path.to_path_buf(),
                deltas,
            });
        },
        |dst, src| {
            dst.findings.extend(src.findings);
            dst.keys.extend(src.keys);
        },
    )?;
    sort_findings_by_path(&mut tally.findings, |f| &f.path);
    Ok(Sweep {
        findings: tally.findings,
        keys: tally.keys,
        pristine,
    })
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
    let output = pristine.json_report(
        serde_json::json!({}),
        serde_json::json!({
            "findings": findings.len(),
            "files": items,
        }),
    );
    super::print_json_pretty(&output);
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
                (" a".to_string(), "MISSING", 2, 1),
                (" b".to_string(), "MISSING", 1, 0),
                (" c".to_string(), "EXTRA", 0, 1),
            ]
        );
        assert!(diff_censuses(&input, &input).is_empty());
    }

    /// The `(content, direction)` of every delta between an input and a hand-written output.
    fn deltas(input: &str, output: &str, parser: ParserType) -> Vec<(String, &'static str)> {
        let input = comment_census(input, parser);
        let output = comment_census(output, parser);
        diff_censuses(&input, &output)
            .iter()
            .map(|d| (d.entry.content.clone(), d.direction().name()))
            .collect()
    }

    #[test]
    fn a_dropped_comment_after_an_earlier_statements_comparison_is_missing() {
        // An earlier statement's comparison `<` must not wait to be "closed" by a later
        // comparison `>`: the regex after that `>` would read as division, a quote in its
        // body would open a string, and the comment after it would vanish from both sides —
        // a drop the census could not see.
        for (input, output, dropped) in [
            (
                "for (let i = 0; i < n; i++) {}\nconst ok = x > /'/.test(s); // DROPME1\nconst y = 'z';\n",
                "for (let i = 0; i < n; i++) {}\nconst ok = x > /'/.test(s);\nconst y = 'z';\n",
                " DROPME1",
            ),
            (
                "if (a < b) {\n\tc = d > /\"/.test(e); /* DROPME5 */ f = \"g\";\n}\n",
                "if (a < b) {\n\tc = d > /\"/.test(e);\n\tf = \"g\";\n}\n",
                " DROPME5 ",
            ),
            (
                "let k = a<b;\nlet m = c > /`/.test(d); // DROPME7\nlet n = `z`;\n",
                "let k = a < b;\nlet m = c > /`/.test(d);\nlet n = `z`;\n",
                " DROPME7",
            ),
            // A conditional expression's `?`/`:` between a comparison `<` and `>`.
            (
                "x = a < b ? c : d > /'/.test(e); // DROPME12\ny = 'z';\n",
                "x = a < b ? c : d > /'/.test(e);\ny = 'z';\n",
                " DROPME12",
            ),
            // A logical operator or a call between a comparison's `<` and `>`.
            (
                "if (i < n && j > /'/.test(s)) {} // DROPME13\nz = 'z';\n",
                "if (i < n && j > /'/.test(s)) {\n}\nz = 'z';\n",
                " DROPME13",
            ),
            (
                "x = a < b || c > /'/.test(d); // DROPME14\nz = 'z';\n",
                "x = a < b || c > /'/.test(d);\nz = 'z';\n",
                " DROPME14",
            ),
            (
                "x = a < b.c(d) > /'/.test(e); // DROPME15\nz = 'z';\n",
                "x = a < b.c(d) > /'/.test(e);\nz = 'z';\n",
                " DROPME15",
            ),
            (
                "x = a < b[c()] > /'/.test(e); // DROPME16\nz = 'z';\n",
                "x = a < b[c()] > /'/.test(e);\nz = 'z';\n",
                " DROPME16",
            ),
            // A comparison whose `<`…`>` run holds a type keyword, a call or an object
            // literal's method — spaced, as the printer spells every comparison.
            (
                "x = a < typeof (b) > /'/.test(d); // DROPME17\nz = 'z';\n",
                "x = a < typeof b > /'/.test(d);\nz = 'z';\n",
                " DROPME17",
            ),
            (
                "x = a < infer(b) > /'/.test(d); // DROPME18\nz = 'z';\n",
                "x = a < infer(b) > /'/.test(d);\nz = 'z';\n",
                " DROPME18",
            ),
            (
                "x = a < asserts(b) > /'/.test(d); // DROPME19\nz = 'z';\n",
                "x = a < asserts(b) > /'/.test(d);\nz = 'z';\n",
                " DROPME19",
            ),
            (
                "x = a < abstract(b) > /'/.test(d); // DROPME20\nz = 'z';\n",
                "x = a < abstract(b) > /'/.test(d);\nz = 'z';\n",
                " DROPME20",
            ),
            (
                "x = a < is(b) > /'/.test(d); // DROPME21\nz = 'z';\n",
                "x = a < is(b) > /'/.test(d);\nz = 'z';\n",
                " DROPME21",
            ),
            (
                "x = a < { m(b) {} }.m > /'/.test(d); // DROPME22\nz = 'z';\n",
                "x = a < { m(b) {} }.m > /'/.test(d);\nz = 'z';\n",
                " DROPME22",
            ),
            (
                "x = a < new (b) > /'/.test(d); // DROPME23\nz = 'z';\n",
                "x = a < new b() > /'/.test(d);\nz = 'z';\n",
                " DROPME23",
            ),
            // The same with no `;` to end the first statement (ASI), which the printer adds.
            (
                "let k = a < b\nlet m = c > /`/.test(d) // DROPME11\nlet n = `z`\n",
                "let k = a < b;\nlet m = c > /`/.test(d);\nlet n = `z`;\n",
                " DROPME11",
            ),
        ] {
            assert_eq!(
                deltas(input, output, ParserType::TypeScript),
                vec![(dropped.to_string(), "MISSING")],
                "{input:?}"
            );
        }
    }

    #[test]
    fn a_stripped_pair_around_a_type_argument_list_balances() {
        // The printer strips the pair around an instantiation before a `/`, so the census
        // must read `f<…> / 2` as the division the authored `(f<…>) / 2` is — a kept
        // comment balances, and a dropped one is still MISSING.
        for (authored, printed) in [
            ("(f<'a'>) / 2", "f<'a'> / 2"),
            ("(f<{ a: T }>) / 2", "f<{ a: T }> / 2"),
            ("(f<() => void>) / 2", "f<() => void> / 2"),
            ("(f<A extends B ? C : D>) / 2", "f<A extends B ? C : D> / 2"),
            ("(f<-1>) / 2", "f<-1> / 2"),
            (
                "(f<typeof import('./a')>) / 2",
                "f<typeof import('./a')> / 2",
            ),
            ("(f<{ m(): void }['m']>) / 2", "f<{ m(): void }['m']> / 2"),
        ] {
            let kept = deltas(
                &format!("x = {authored}; // c\n"),
                &format!("x = {printed}; // c\n"),
                ParserType::TypeScript,
            );
            assert!(kept.is_empty(), "{authored:?}: {kept:?}");
            let dropped = deltas(
                &format!("x = {authored}; // DROPME\n"),
                &format!("x = {printed};\n"),
                ParserType::TypeScript,
            );
            assert_eq!(
                dropped,
                vec![(" DROPME".to_string(), "MISSING")],
                "{authored:?}"
            );
        }
        // The same in a Svelte document's template expression.
        assert_eq!(
            deltas(
                "{(f<() => void>) / 2 /* DROPME */}\n",
                "{f<() => void> / 2}\n",
                ParserType::Svelte
            ),
            vec![(" DROPME ".to_string(), "MISSING")]
        );
    }

    #[test]
    fn a_non_canonical_angle_spelling_is_the_documented_residue() {
        // The census reads a type-argument list off the printer's spelling (glued) and a
        // comparison off its spacing, so an author's own spelling of either the other way
        // is misread on the INPUT side alone (docs/audits.md §census): the kept comment
        // reads EXTRA and a dropped one goes unseen.
        for (authored, printed) in [
            // A fully glued comparison, read as a list.
            ("x = a<b()>/'/.test(d);", "x = a < b() > /'/.test(d);"),
            // Spaced type arguments, read as a comparison.
            ("x = f < T > / 2;", "x = f<T> / 2;"),
        ] {
            assert_eq!(
                deltas(
                    &format!("{authored} // c\nz = 'z';\n"),
                    &format!("{printed} // c\nz = 'z';\n"),
                    ParserType::TypeScript
                ),
                vec![(" c".to_string(), "EXTRA")],
                "{authored:?}"
            );
            assert!(
                deltas(
                    &format!("{authored} // DROPME\nz = 'z';\n"),
                    &format!("{printed}\nz = 'z';\n"),
                    ParserType::TypeScript
                )
                .is_empty(),
                "{authored:?}"
            );
        }
    }
}
