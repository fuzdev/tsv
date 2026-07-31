use argh::FromArgs;
use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::audit::ratchet::{Ratchet, SnapshotKey, refuse_narrowed_update};
use crate::audit::sweep::{PristineSweep, sweep_pristine};
use crate::cli::CliError;

use super::profile::resolve_files;

/// Audit for blank lines the formatter INVENTS.
///
/// The pristine counterpart to `blank_audit`. That audit MUTATES a seed — it injects a blank
/// into a code gap and grades the response — so its subject is how the formatter reacts to a
/// blank the author *did* write. This one never mutates: it formats the seed as authored and
/// asks whether the output holds a blank run the input did not. A fabricated blank is
/// indistinguishable from an authored one once written, so it is silent content the author
/// never approved.
///
/// **No other gate covers this.** F1 is structurally blind — the fabricated blank is authored
/// as far as pass 2 is concerned, so pass 2 preserves it and the file is a fixed point. The
/// injection ratchets are blind by construction: they grade a MUTATED seed, and both exempt
/// format-ignore regions outright.
///
/// Graded as a RATCHET over `fabrication_audit_known.txt`, keyed by the *shape* of the two
/// lines bracketing the invented blank (so it is corpus-portable — no paths). Every line is a
/// bug; the file shrinking is the goal. The two legitimate fabrications — the hoisted-section
/// seam and an empty block body — are structural carve-outs ([`is_sanctioned`]) rather than
/// snapshot lines, so the snapshot never mixes sanctioned rules with debt.
///
/// Pure Rust — no Deno. Defaults to `tests/fixtures` when no paths are given.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "fabrication_audit")]
pub struct FabricationAuditCommand {
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

/// REGRESSION PIN (minimum, at the exact measured value): files formatted on a default
/// (`tests/fixtures`) run — with an empty or all-parse-failing corpus the audit would pass
/// vacuously ("0 fabrications across 0 files"). A minimum, not a two-sided pin, because the
/// fixtures tree is COMMITTED and grows with ordinary fixture PRs (`deno task check` must not
/// fail per added fixture); shrinkage/collapse fails. Same ritual as `swallow_audit`'s
/// `FORMATTED_MIN` and `benches/js/lib/gate_counts.ts`.
const FORMATTED_MIN: usize = 7_032;

const SNAPSHOT_HEADER: &str = "\
# Blank-fabrication ratchet — every line is a BUG, the file shrinking is the goal.
#
# One line per SHAPE of invented blank line: the syntactic shape of the output line
# BEFORE the blank run, a TAB, then the shape of the line AFTER it. Shapes rather than
# paths, so the snapshot is corpus-portable.
#
# A shape found but not pinned FAILS (a new fabrication). A pinned shape that no longer
# fires FAILS (fix landed — re-pin). The two sanctioned layout rules — the hoisted-section
# seam and an empty block body — are NOT here: they are carved out structurally in
# `fabrication_audit.rs`.
#
# Regenerate with `deno task fabrication:audit:update`.
";

const REPIN_HINT: &str = "deno task fabrication:audit:update";

/// One shape of invented blank: what brackets it, normalized.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct FabricationShape {
    /// Shape of the last non-blank output line before the invented run.
    before: String,
    /// Shape of the first non-blank output line after it.
    after: String,
}

impl SnapshotKey for FabricationShape {
    fn to_line(&self) -> String {
        format!("{}\t{}", self.before, self.after)
    }

    fn from_line(line: &str) -> Option<Self> {
        let (before, after) = line.split_once('\t')?;
        Some(Self {
            before: before.to_string(),
            after: after.to_string(),
        })
    }

    /// Every fabrication shape is pinnable — unlike gap/blank's PANIC class there is no
    /// absolute sub-invariant here. Fabricating a blank is always a bug; the ratchet's job
    /// is to stop NEW ones while the known ones are burned down.
    fn is_pinnable(&self) -> bool {
        true
    }
}

/// A file whose format invented blank runs, with the counts that say so.
struct Fabrication {
    path: PathBuf,
    /// Blank runs in the seed as authored.
    input_runs: usize,
    /// Blank runs in the output excused by a carve-out (reported for transparency).
    sanctioned: usize,
    /// Every blank run in the output that no carve-out excuses, in output order.
    ///
    /// **Non-empty by construction** — the one push site guards on
    /// `invented.len() > input_runs`, and `input_runs` is unsigned. Held as the runs
    /// themselves rather than a count + a separate first-example field: those could
    /// disagree, and "the count says 3, the example says none" has no meaning.
    invented: Vec<Invented>,
}

/// One invented blank run in a file's output.
struct Invented {
    /// 1-based output line the blank run opens on.
    line: usize,
    /// The trimmed line the run precedes — what the blank was inserted before.
    anchor: String,
    /// What brackets the run, normalized — this run's contribution to the ratchet.
    shape: FabricationShape,
}

impl FabricationAuditCommand {
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
            "the fabrication shapes over tests/fixtures",
            "SUBSET",
        )?;
        let paths = if default_paths {
            vec!["tests/fixtures".to_string()]
        } else {
            self.paths
        };
        let files = match resolve_files(&paths) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error: {e}");
                return Err(CliError::Failed);
            }
        };

        let sweep = sweep_files(&files);

        if self.json {
            print_json(&sweep);
        } else {
            print_report(&sweep);
        }

        if default_paths && sweep.pristine.formatted < FORMATTED_MIN {
            eprintln!(
                "Error: pinned minimum — formatted {} files < pinned {FORMATTED_MIN}. \
                 The fixtures walk shrank (or parsing collapsed); if deliberate, re-pin FORMATTED_MIN.",
                sweep.pristine.formatted
            );
            return Err(CliError::Failed);
        }

        let ratchet = ratchet();
        if self.update {
            ratchet.write_pinned(&sweep.shapes, "shape")?;
            return Ok(());
        }
        // Off the default corpus the snapshot doesn't apply — it pins the full default run, so
        // grading a narrowed one would call every unreached shape stale. Every fabrication is
        // news instead.
        if !default_paths {
            return if sweep.fabrications.is_empty() {
                Ok(())
            } else {
                Err(CliError::Failed)
            };
        }

        let diff = ratchet.grade(&sweep.shapes)?;
        if diff.holds() {
            println!(
                "\n✓ ratchet holds — {} known fabrication shape(s), no new ones ({} files)",
                diff.known, sweep.pristine.formatted
            );
            return Ok(());
        }
        for shape in &diff.new {
            eprintln!(
                "✗ NEW fabrication shape (not pinned): {:?} ⇢ blank ⇢ {:?}",
                shape.before, shape.after
            );
        }
        for shape in &diff.stale {
            eprintln!(
                "✗ STALE pin (no longer fires — fix landed, re-pin): {:?} ⇢ blank ⇢ {:?}",
                shape.before, shape.after
            );
        }
        eprintln!("\nRe-pin with `{REPIN_HINT}` once the change is deliberate.");
        Err(CliError::Failed)
    }
}

/// What one corpus walk produced.
struct Sweep {
    /// Files that fabricated, in walk order — the human report.
    fabrications: Vec<Fabrication>,
    /// Every fabrication shape seen, deduped — what the ratchet grades.
    shapes: BTreeSet<FabricationShape>,
    /// The shared skip/format bookkeeping (the `FORMATTED_MIN` vacuity guard
    /// reads `formatted`; panics are counted there, not gated here — the panic
    /// gates own that class).
    pristine: PristineSweep,
}

/// Format every file (via the shared pristine sweep) and collect the fabrications.
///
/// Split out of `run` so that function reads as scope → sweep → report → gate, rather than
/// interleaving the corpus walk with the ratchet's argument handling.
fn sweep_files(files: &[PathBuf]) -> Sweep {
    let mut fabrications = Vec::new();
    let mut shapes = BTreeSet::new();
    let pristine = sweep_pristine(files, |path, _parser, source, output| {
        let input_runs = count_blank_runs(source);
        let scan = scan_output(output);
        if scan.invented.len() > input_runs {
            shapes.extend(scan.invented.iter().map(|i| i.shape.clone()));
            fabrications.push(Fabrication {
                path: path.to_path_buf(),
                input_runs,
                sanctioned: scan.sanctioned,
                invented: scan.invented,
            });
        }
    });
    Sweep {
        fabrications,
        shapes,
        pristine,
    }
}

/// The ratchet snapshot, colocated with this module and read at runtime by the [`Ratchet`]
/// (see that module for why not `include_str!`). Anchored on `CARGO_MANIFEST_DIR` rather than
/// the cwd, so the audit finds it from any working directory.
fn known_shapes_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/commands/fabrication_audit_known.txt")
}

/// The ratchet over [`known_shapes_path`], carrying this audit's header + re-pin hint.
fn ratchet() -> Ratchet {
    Ratchet::new(known_shapes_path(), SNAPSHOT_HEADER, REPIN_HINT)
}

/// Whether `line` is blank — empty or horizontal whitespace only.
///
/// The class is `[ \t\r]`, matching the printer's own horizontal-whitespace class
/// (`tsv_svelte`'s `is_horizontal_ws`). A form feed is rendered content, so a line holding one
/// is NOT blank — the same narrowing the render-key oracle and `Text`'s collapse class make,
/// and for the same reason: a class one character too wide would let the audit vouch for a
/// formatter that fabricated a line containing content.
fn is_blank_line(line: &str) -> bool {
    line.bytes().all(|b| matches!(b, b' ' | b'\t' | b'\r'))
}

/// One maximal run of blank lines, with the non-blank lines that bracket it.
struct BlankRun<'a> {
    /// 1-based line number the run opens on.
    line: usize,
    /// Last non-blank line before the run; `""` at the start of the document.
    prev: &'a str,
    /// First non-blank line after the run; `""` at the end of the document.
    next: &'a str,
}

/// Visit each maximal run of blank lines in `s`, in order.
///
/// **The single definition of "a blank run"**, shared by the input count and the output scan.
/// The metric is a comparison between those two, so two copies of this walk that drifted — one
/// counting runs, the other lines — would silently compare different things and the audit would
/// report nonsense while looking healthy.
///
/// Runs, not lines: collapsing `a⏎⏎⏎⏎b` to `a⏎⏎b` removes blank *lines* but not the blank
/// *run*, and the author's signal is "there is a break here", not how many newlines spell it.
fn for_each_blank_run(s: &str, mut visit: impl FnMut(BlankRun<'_>)) {
    let lines: Vec<&str> = s.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if !is_blank_line(lines[i]) {
            i += 1;
            continue;
        }
        let prev = if i == 0 { "" } else { lines[i - 1] };
        let next = lines[i..]
            .iter()
            .find(|l| !is_blank_line(l))
            .copied()
            .unwrap_or("");
        visit(BlankRun {
            line: i + 1,
            prev,
            next,
        });
        while i < lines.len() && is_blank_line(lines[i]) {
            i += 1;
        }
    }
}

/// The number of maximal runs of consecutive blank lines in `s`.
fn count_blank_runs(s: &str) -> usize {
    let mut runs = 0;
    for_each_blank_run(s, |_| runs += 1);
    runs
}

/// Whether a blank run with this bracketing [`FabricationShape`] is one of tsv's SANCTIONED
/// fabrications — a layout rule of the output rather than a claim about the input.
///
/// Two rules, each stated on its own below. They are carve-outs in code rather than lines in
/// the ratchet because the snapshot means "every line is a bug"; pinning a sanctioned rule
/// there would make that false and invite someone to "fix" it.
///
/// Takes the shape, not the raw lines — the shapes are the run's one normalization, shared
/// with the ratchet key ([`scan_output`] computes them once). A shape carries the exact tag
/// name, so `<style-guide>` (a custom element) cannot slip through a `starts_with("<style")`
/// and silently widen a carve-out past what it says.
fn is_sanctioned(shape: &FabricationShape) -> bool {
    is_section_seam(&shape.before, &shape.after) || is_empty_block_body(&shape.before, &shape.after)
}

/// Whether the run sits at a hoisted-section boundary.
///
/// tsv moves `<script>` / `<style>` / `<svelte:options>` to canonical positions and separates
/// each from its neighbours with a blank line. The carve-out is **two-sided** because the seam
/// is: a glued `</script><div>` puts the section's closing tag before the run, while a glued
/// `</div><style>` puts the section's opening tag after it, and excusing only one side leaves
/// the other reported as a fabrication. This is the narrowest statable form — it excuses a run
/// that touches a root section boundary, and nothing else.
///
/// `<svelte:options` appears on both sides because that section is a single self-closing line,
/// so it opens and closes on the same shape.
///
/// ⚠️ **Known over-report.** Comments travel with their section, so a glued
/// `<div>block1</div>⏎<!-- comment -->⏎<style>` puts the section's *leading comment* where the
/// tag would be — the run reads `</div` ⇢ `<!--`, nothing here fires, and the audit reports a
/// blank prettier emits too. Not widened: the only shape visible here is "a blank before some
/// comment", and excusing that would blind the audit to a whole class of real fabrications.
/// Stating it narrowly needs the AST, not line shapes. Latent in practice — an already-formatted
/// corpus carries the blank in its input, so the counts match.
fn is_section_seam(prev: &str, next: &str) -> bool {
    let closes_section = matches!(prev, "</script" | "</style" | "<svelte:options");
    let opens_section = matches!(next, "<script" | "<style" | "<svelte:options");
    closes_section || opens_section
}

/// Whether the run IS an empty block body.
///
/// A kept-but-empty block section prints in block form — its opener and its terminator each keep
/// their own line, and the empty body between them is a blank line, so `{:catch error}{/await}`
/// becomes `{:catch error}⏎⏎{/await}`. That blank is the body, not a separator the author wrote.
/// Sanctioned by the `empty_branch_collapse` and `empty_catch_multiline` fixtures, whose READMEs
/// state it.
///
/// Stated as "a block opener or branch on the left, a branch or terminator on the right", which
/// is exactly the bracketing an empty body has and nothing else: real content between them would
/// put a non-blank line there and the run would not be adjacent to both. `prev` / `next` are
/// [`line_shape`]s, which keep the sigil the test reads.
fn is_empty_block_body(prev: &str, next: &str) -> bool {
    let opens_body = prev.starts_with("{#") || prev.starts_with("{:");
    let closes_body = next.starts_with("{:") || next.starts_with("{/");
    opens_body && closes_body
}

/// The syntactic shape of `line` — a stable token standing for what kind of construct opens it.
///
/// The ratchet keys on shapes rather than file paths so the snapshot survives a corpus change
/// and states the BUG (`{:catch` gains a blank before `{/await`) instead of a location. The
/// alphabet is deliberately coarse: a tag name without its attributes, a block keyword without
/// its expression, `text` for prose, `^` for the document edge.
fn line_shape(line: &str) -> String {
    let t = line.trim();
    if t.is_empty() {
        return "^".to_string();
    }
    if t.starts_with("<!--") {
        return "<!--".to_string();
    }
    if let Some(rest) = t.strip_prefix("</") {
        return format!("</{}", head_name(rest));
    }
    if let Some(rest) = t.strip_prefix('<') {
        return format!("<{}", head_name(rest));
    }
    if let Some(rest) = t.strip_prefix('{') {
        return match rest.chars().next() {
            Some(sigil @ ('#' | ':' | '/' | '@')) => {
                format!("{{{sigil}{}", head_name(&rest[sigil.len_utf8()..]))
            }
            // A plain `{expr}` interpolation — the expression itself is not part of the shape.
            _ => "{".to_string(),
        };
    }
    "text".to_string()
}

/// The leading name token of `s` — letters, digits, `:`, `-`, `_`.
fn head_name(s: &str) -> String {
    s.chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '-' | '_'))
        .collect()
}

/// Per-output blank-run tally, split by whether a carve-out excuses the run.
struct OutputScan {
    /// Runs a carve-out excuses (counted for transparency, never graded).
    sanctioned: usize,
    /// Runs it does not, in output order.
    invented: Vec<Invented>,
}

fn scan_output(out: &str) -> OutputScan {
    let mut scan = OutputScan {
        sanctioned: 0,
        invented: Vec::new(),
    };
    for_each_blank_run(out, |run| {
        let shape = FabricationShape {
            before: line_shape(run.prev),
            after: line_shape(run.next),
        };
        if is_sanctioned(&shape) {
            scan.sanctioned += 1;
            return;
        }
        scan.invented.push(Invented {
            line: run.line,
            // Anchor on the line the run precedes — what the blank was inserted before.
            anchor: run.next.trim().to_string(),
            shape,
        });
    });
    scan
}

fn print_report(sweep: &Sweep) {
    let Sweep {
        fabrications,
        pristine,
        ..
    } = sweep;
    let formatted = pristine.formatted;
    let skipped = pristine.skipped_note();
    if fabrications.is_empty() {
        println!("✓ no fabricated blank lines across {formatted} files ({skipped})");
        return;
    }

    println!(
        "✗ {} file(s) gained a blank line the author did not write ({formatted} formatted, {skipped})\n",
        fabrications.len(),
    );

    for f in fabrications {
        println!("  {}", f.path.display());
        println!(
            "    blank runs: input {} → output {} unsanctioned (+{} sanctioned)",
            f.input_runs,
            f.invented.len(),
            f.sanctioned
        );
        // The first is enough to find the file's problem without re-running; the rest are a
        // count, so one pathological file can't bury the others.
        for e in f.invented.iter().take(1) {
            println!(
                "    first invented blank: output line {}, before {:?}",
                e.line, e.anchor
            );
        }
        if f.invented.len() > 1 {
            println!("    (+{} more in this file)", f.invented.len() - 1);
        }
        println!();
    }
}

fn print_json(sweep: &Sweep) {
    let Sweep {
        fabrications,
        pristine,
        ..
    } = sweep;
    let items: Vec<serde_json::Value> = fabrications
        .iter()
        .map(|f| {
            let invented: Vec<serde_json::Value> = f
                .invented
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "line": e.line,
                        "anchor": e.anchor,
                        "shape_before": e.shape.before,
                        "shape_after": e.shape.after,
                    })
                })
                .collect();
            serde_json::json!({
                "path": f.path.to_string_lossy(),
                "input_blank_runs": f.input_runs,
                "output_blank_runs_unsanctioned": f.invented.len(),
                "output_blank_runs_sanctioned": f.sanctioned,
                "invented": invented,
            })
        })
        .collect();
    let output = serde_json::json!({
        "formatted": pristine.formatted,
        "parse_skipped": pristine.parse_errors,
        "read_skipped": pristine.read_errors,
        "panicked": pristine.panics,
        "fabrications": fabrications.len(),
        "files": items,
    });
    #[allow(clippy::unwrap_used)]
    let s = serde_json::to_string_pretty(&output).unwrap();
    println!("{s}");
}

#[cfg(test)]
mod tests {
    use super::{
        FabricationShape, count_blank_runs, is_blank_line, is_sanctioned, line_shape, scan_output,
    };

    /// The raw-line → shape → carve-out pipeline exactly as [`scan_output`] runs it, so the
    /// carve-out tests keep exercising [`line_shape`]'s normalization (whitespace, attributes)
    /// rather than pre-shaped tokens.
    fn sanctioned(prev: &str, next: &str) -> bool {
        is_sanctioned(&FabricationShape {
            before: line_shape(prev),
            after: line_shape(next),
        })
    }

    #[test]
    fn blank_runs_count_runs_not_lines() {
        assert_eq!(count_blank_runs("a\nb"), 0);
        assert_eq!(count_blank_runs("a\n\nb"), 1);
        // Four newlines are still ONE break in the author's signal.
        assert_eq!(count_blank_runs("a\n\n\n\nb"), 1);
        assert_eq!(count_blank_runs("a\n\nb\n\nc"), 2);
        // Horizontal whitespace does not make a line non-blank; a form feed does.
        assert!(is_blank_line(" \t\r"));
        assert!(!is_blank_line("\u{c}"));
        assert_eq!(count_blank_runs("a\n \t\nb"), 1);
        assert_eq!(count_blank_runs("a\n\u{c}\nb"), 0);
    }

    #[test]
    fn section_seam_is_two_sided() {
        assert!(sanctioned("</script>", "<div>a</div>"));
        assert!(sanctioned("\t</style>  ", "text"));
        assert!(sanctioned("<svelte:options runes />", "text"));
        // The other side of the seam: a glued `</div><style>`.
        assert!(sanctioned("<div>a</div>", "<style>"));
        assert!(sanctioned("text", "<script lang=\"ts\">"));
        assert!(!sanctioned("<!-- format-ignore-end -->", "text"));
        assert!(!sanctioned("</div>", "<div>a</div>"));
        // The carve-out is keyed on the exact tag name, so a custom element that merely
        // starts with a section name does not widen it.
        assert!(!sanctioned("text", "<style-guide>"));
        assert!(!sanctioned("</script-host>", "text"));
    }

    #[test]
    fn shapes_drop_attributes_and_expressions() {
        assert_eq!(line_shape("<div class=\"a\" data-attr=\"v\">"), "<div");
        assert_eq!(line_shape("</div>"), "</div");
        assert_eq!(line_shape("<!-- anything at all -->"), "<!--");
        assert_eq!(line_shape("{:catch error}"), "{:catch");
        assert_eq!(line_shape("{/await}"), "{/await");
        assert_eq!(line_shape("{#if cond}"), "{#if");
        assert_eq!(line_shape("{expr}"), "{");
        assert_eq!(line_shape("some prose"), "text");
        assert_eq!(line_shape("   "), "^");
        assert_eq!(line_shape("<svelte:options runes />"), "<svelte:options");
    }

    #[test]
    fn scan_splits_sanctioned_from_invented() {
        let out = "<script>\n\tlet a = 1;\n</script>\n\n<div>a</div>\n\ntext\n";
        let scan = scan_output(out);
        assert_eq!(scan.sanctioned, 1);
        assert_eq!(scan.invented.len(), 1);
        let invented = &scan.invented[0];
        assert_eq!(invented.line, 6);
        assert_eq!(invented.anchor, "text");
        // The shape keys on how a line OPENS, so a one-line `<div>a</div>` is `<div` —
        // `</div` is the shape of a *closing* line in a multi-line element.
        assert_eq!(invented.shape.before, "<div");
        assert_eq!(invented.shape.after, "text");
    }
}
