//! Whitespace-**direction** audit for the `_compact` / `_spaces` normalization variants.
//!
//! The two standard variant names are a matched pair of opposite claims about the
//! same input, and the pair is the point: `_compact` removes whitespace the
//! formatter normalizes away, `_spaces` adds whitespace the formatter normalizes
//! away, so between them a fixture is squeezed from both sides. The N rules prove
//! only that each variant *lands on* input; nothing has ever checked that it
//! travelled there from the direction its name advertises, and the corpus drifted
//! accordingly — `_compact` files padding with double spaces
//! (`interface Empty {  }`), `_spaces` files stripping them (`h1 + p` → `h1+p`).
//! A variant pointing the wrong way is not a weaker test, it is a *duplicate* of
//! its sibling wearing the other one's name, and the direction it was supposed to
//! cover goes untested with nothing to say so.
//!
//! ## The instrument: gap alignment
//!
//! A whitespace-only variant has the same non-whitespace character sequence as its
//! reference (§Scope below), so the two align on that shared stream: at every position
//! the reference holds a whitespace run and the variant holds the run that replaced it.
//! That alignment is exact — no lexer, no removability heuristic, no formatter run —
//! and it makes the naming claim a per-gap question.
//!
//! Two events are graded, both unambiguous:
//!
//! - **EMPTIED** — the input's run is non-empty and the variant's is empty. A true
//!   removal.
//! - **WIDENED** — *neither* run contains a newline and the variant's is strictly
//!   longer. A true horizontal addition.
//!
//! `_compact` must never WIDEN; `_spaces` must never EMPTY.
//!
//! ⚠️ **The newline exclusion on WIDENED is load-bearing, not laziness.** Raw length
//! over-flags: a `_spaces` variant that joins a deeply-indented input onto one line
//! replaces `\n\t\t\t` (4 chars) with `  ` (2) — *shorter*, yet the variant is
//! plainly adding horizontal space. Comparing lengths only where both sides are
//! newline-free asks the horizontal question in isolation, and the vertical axis is
//! left to EMPTIED, which no re-indentation can trip. Grading the vertical axis on
//! `_compact` as well was measured and rejected: it flags 27 further variants, and
//! they are almost all *correct content* — `blank_line_before_first_only`'s variant
//! is compact everywhere except the one blank line the fixture exists to prove
//! collapses, which it can only prove by adding it.
//!
//! ⚠️ **The whitespace class is [`is_collapsible_ws_char`], never Rust's
//! `is_ascii_whitespace`.** Here the class decides how the two files ALIGN, so a
//! character wrongly counted as whitespace shifts one stream against the other and
//! every gap after it is graded against the wrong partner. The narrow class is also
//! the conservative one: a real whitespace character treated as content merely
//! splits one gap into two, and both halves are still graded.
//!
//! ## Scope: the exact suffix only
//!
//! Graded names are `unformatted_`, `unformatted_ours_` and `unformatted_prettier_`
//! followed by exactly `compact` or `spaces`. Each prefix also names the **reference**
//! the gaps are aligned against — the form that variant is required to normalize to,
//! which is `input.*` for the first two and `output_prettier.*` for
//! `unformatted_prettier_*` (N8). Aligning every prefix against `input` would grade
//! that one against a form it makes no claim about, and because the mismatch surfaces
//! as an alignment failure it would land in the blind-spot bucket below rather than as
//! an error — a silent hole, which is why the prefix carries its reference rather than
//! being dropped from the list. A *qualified* name
//! (`unformatted_unicode_spaces`, `unformatted_then_spaces`,
//! `unformatted_spaces_before_colon`, `unformatted_compactish`) makes a narrower,
//! specific claim rather than the bare directional one, and `unicode_spaces` in
//! particular is a character *substitution* — it swaps ASCII space for U+00A0 and
//! friends — which is not the whitespace-volume axis this audit grades at all.
//!
//! The oracle-generated forms are out of scope by construction:
//! `prettier_variant_compact`, `variant_compact`, `divergent_variant_compact` and
//! the `prettier_intermediate*` kinds are **prettier's own output**, so their suffix
//! names the form prettier preserved rather than an authoring instruction. Grading
//! them would put the audit in an argument with the oracle.
//!
//! ## Blind spot: the unalignable variants
//!
//! A variant whose non-whitespace stream differs from its reference cannot be
//! aligned, so it cannot be graded. They are counted, and listed by `--list` /
//! `--json`, rather than dropped — a silently-excluded class reads as a covered one.
//!
//! The composition is measured, not assumed: a clear majority differ by exactly a
//! **trailing comma** before a closer (`}` / `)` / `]` / `>`), then quote flips
//! (`'x'` → `"x"`), then an added paren or a leading `|`. Only that tail is a
//! *naming* problem (a whitespace name over a genuinely mixed edit) — the
//! trailing-comma class is a whitespace variant plus one mechanical token
//! `trailingComma: 'none'` deletes, and its direction is exactly what one wants
//! graded. Normalizing a trailing comma before a closer would recover about half the
//! blind spot; both that and re-homing the tail are deliberately out of scope here.
//!
//! ## Direction only, never extent
//!
//! A `_compact` variant that empties one gap and leaves twenty removable ones passes.
//! Extent is decidable (empty a gap, re-format, see whether it still lands on the
//! reference) but deliberately ungated: most non-empty gaps are mandatory separators,
//! and forcing maximality would delete injection sites `gaps:audit` / `blanks:audit` /
//! `ignore:audit` enumerate from the seed text. One subclass is held at zero by
//! convention instead — a HORIZONTAL gap touching a block comment, since
//! `owned_by_node` is defined by glue and a space-padded comment in a compact variant
//! tests nothing its own input does not. A gap carrying a **newline** is left alone
//! either way: own-line-ness is authorship, not volume. See `docs/audits.md`.
//!
//! Pure Rust — no Deno, no formatter run. Part of `deno task check` (via
//! `deno task variants:audit`).

use crate::audit::vacuity::check_graded_nonzero;
use crate::cli::CliError;
use crate::fixtures::{self, FixtureFiles};
use argh::FromArgs;
use tsv_svelte::ast::internal::is_collapsible_ws_char;

/// Audit that `_compact` / `_spaces` variants move whitespace the way their names claim.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "variant_audit")]
pub struct VariantAuditCommand {
    /// list every graded variant with its emptied/widened counts instead of only failures
    #[argh(switch)]
    list: bool,

    /// emit a machine-readable JSON report
    #[argh(switch)]
    json: bool,
}

/// REGRESSION PIN (minimum, at the exact measured value): `_compact`/`_spaces`
/// variants a full run grades.
///
/// The [`check_graded_nonzero`] floor underneath catches a total collapse; this
/// catches the partial one it cannot see — a walk that still finds fixtures but
/// stops recognizing most variants (a bucket rename, a prefix typo, an extension
/// mismatch silently routing them to `unknown`). A **minimum** rather than an exact
/// pin because the fixtures tree is committed and grows with ordinary fixture PRs.
///
/// Private rather than shared: this audit's denominator is *variants*, not the
/// formatted-file count [`crate::audit::vacuity::FIXTURES_FORMATTED_MIN`] pins, so
/// sharing that constant would compare two different populations.
const VARIANTS_GRADED_MIN: usize = 1_951;

/// The whitespace-direction claim a variant's name makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    /// `_compact` — removes whitespace that normalizes away. Must never widen a gap.
    Compact,
    /// `_spaces` — adds whitespace that normalizes away. Must never empty a gap.
    Spaces,
}

impl Direction {
    /// The bare suffix that names this direction.
    const fn suffix(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Spaces => "spaces",
        }
    }

    /// How a violation of this direction reads in a report.
    const fn violation_label(self) -> &'static str {
        match self {
            Self::Compact => "widened a gap",
            Self::Spaces => "emptied a gap",
        }
    }
}

/// The file a variant's gaps are aligned against — the form it is required to
/// normalize to, and therefore the form its whitespace was added to or removed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reference {
    /// `input.*` — what `unformatted_*` (N4) and `unformatted_ours_*` (N5) normalize to.
    Input,
    /// `output_prettier.*` — what `unformatted_prettier_*` normalizes to (N8).
    PrettierOutput,
}

/// The three authored-variant prefixes, each with the reference it normalizes to.
/// `prettier_variant_` and its siblings are deliberately absent — see the module docs.
const GRADED_PREFIXES: &[(&str, Reference)] = &[
    ("unformatted_ours_", Reference::Input),
    ("unformatted_prettier_", Reference::PrettierOutput),
    ("unformatted_", Reference::Input),
];

/// The reference and direction a variant filename claims, or `None` when the name is
/// not a bare `compact`/`spaces` one.
///
/// `unformatted_ours_` is tested before `unformatted_` so an `_ours_` name is not
/// mis-read as an `unformatted_` one with the suffix `ours_compact` (which would
/// then be rejected as qualified, silently dropping 268 variants from the gate).
fn claimed_direction(filename: &str, ext: &str) -> Option<(Reference, Direction)> {
    let stem = filename.strip_suffix(ext)?;
    let (reference, suffix) = GRADED_PREFIXES
        .iter()
        .find_map(|(p, r)| Some((*r, stem.strip_prefix(p)?)))?;
    match suffix {
        s if s == Direction::Compact.suffix() => Some((reference, Direction::Compact)),
        s if s == Direction::Spaces.suffix() => Some((reference, Direction::Spaces)),
        _ => None,
    }
}

/// One whitespace run, with the byte offset it starts at.
type Gap<'a> = (usize, &'a str);

/// Split `text` into its non-whitespace stream and the whitespace run preceding each
/// of that stream's characters, plus one trailing run.
///
/// The returned run vector always has `stream.chars().count() + 1` entries, so two
/// texts with equal streams zip index-for-index.
fn split_gaps(text: &str) -> (String, Vec<Gap<'_>>) {
    let mut stream = String::with_capacity(text.len());
    let mut runs = Vec::new();
    // The open run is always `[run_start, run_end)`: `run_start` is one past the
    // previous stream character (0 before the first), `run_end` walks with the
    // whitespace, so an empty run reports the offset where whitespace would go.
    let mut run_start = 0;
    let mut run_end = 0;
    for (offset, ch) in text.char_indices() {
        if is_collapsible_ws_char(ch) {
            run_end = offset + ch.len_utf8();
        } else {
            runs.push((run_start, &text[run_start..run_end]));
            stream.push(ch);
            run_start = offset + ch.len_utf8();
            run_end = run_start;
        }
    }
    runs.push((run_start, &text[run_start..run_end]));
    (stream, runs)
}

/// A single gap the variant moved the wrong way.
#[derive(Debug)]
struct Violation {
    /// 1-based line in the variant file.
    line: usize,
    /// What happened, e.g. `"1 → 3"` or `"1 → 0"`.
    detail: String,
}

/// What the gap alignment found, before it is attributed to a file.
#[derive(Debug)]
struct Verdict {
    emptied: usize,
    widened: usize,
    violations: Vec<Violation>,
}

/// The verdict on one variant file.
#[derive(Debug)]
struct Graded {
    fixture: String,
    file: String,
    direction: Direction,
    verdict: Verdict,
}

/// A variant whose non-whitespace stream differs from its input's, so no alignment
/// exists to grade.
#[derive(Debug)]
struct Unalignable {
    fixture: String,
    file: String,
    direction: Direction,
}

/// Grade one aligned pair. `ref_runs` is the [`Reference`] form's runs and
/// `var_runs` the variant's; the two must already be known stream-equal, so they zip
/// index-for-index. `variant` is the variant's text, for line numbers.
fn grade(
    direction: Direction,
    ref_runs: &[Gap<'_>],
    var_runs: &[Gap<'_>],
    variant: &str,
) -> Verdict {
    let mut verdict = Verdict {
        emptied: 0,
        widened: 0,
        violations: Vec::new(),
    };
    for ((_, before), (offset, after)) in ref_runs.iter().zip(var_runs) {
        let emptied = !before.is_empty() && after.is_empty();
        // Both sides newline-free, so this asks the HORIZONTAL question alone — a
        // re-indentation that trades `\n\t\t` for `  ` is not a removal (see module docs).
        let widened = !before.contains('\n') && !after.contains('\n') && after.len() > before.len();
        verdict.emptied += usize::from(emptied);
        verdict.widened += usize::from(widened);
        let offends = match direction {
            Direction::Compact => widened,
            Direction::Spaces => emptied,
        };
        if offends {
            verdict.violations.push(Violation {
                line: line_of(variant, *offset),
                detail: format!("{} → {}", before.len(), after.len()),
            });
        }
    }
    verdict
}

/// The 1-based line number `offset` falls on.
fn line_of(text: &str, offset: usize) -> usize {
    text.get(..offset)
        .map_or(1, |s| s.matches('\n').count() + 1)
}

/// Read a fixture file, or fail the run. Every file this audit reads is one a
/// committed fixture declares, so an unreadable one is a broken tree, not a skip.
fn read_or_fail(path: &std::path::Path) -> Result<String, CliError> {
    fixtures::read_file(path).map_err(|e| {
        eprintln!("Error: {e}");
        CliError::Failed
    })
}

impl VariantAuditCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let all = super::walk_fixtures_or_fail()?;
        let mut graded: Vec<Graded> = Vec::new();
        let mut unalignable: Vec<Unalignable> = Vec::new();

        for fixture in &all {
            let files = FixtureFiles::scan(fixture);
            let ext = fixture.input_type().extension();
            let input = read_or_fail(&fixture.input_path())?;
            let input_gaps = split_gaps(&input);

            // Read `output_prettier.*` only when an `unformatted_prettier_*` variant
            // actually claims it, so an ordinary fixture pays nothing for the arm.
            let needs_prettier = files
                .unformatted_prettier
                .iter()
                .any(|f| claimed_direction(f, ext).is_some());
            let prettier_output = if needs_prettier {
                Some(read_or_fail(&fixture.output_prettier_path())?)
            } else {
                None
            };
            let prettier_gaps = prettier_output.as_deref().map(split_gaps);

            let candidates = files
                .unformatted
                .iter()
                .chain(&files.unformatted_ours)
                .chain(&files.unformatted_prettier);
            for filename in candidates {
                let Some((reference, direction)) = claimed_direction(filename, ext) else {
                    continue;
                };
                let (ref_stream, ref_runs) = match reference {
                    Reference::Input => &input_gaps,
                    // Some by construction — the scan above asks `claimed_direction` the
                    // same question. Loud rather than falling back to `input`, which
                    // would grade against a form the variant makes no claim about.
                    Reference::PrettierOutput => match prettier_gaps.as_ref() {
                        Some(gaps) => gaps,
                        None => {
                            eprintln!(
                                "Error: {}/{filename} claims output_prettier.* as its \
                                 reference but none was read",
                                fixture.relative_path
                            );
                            return Err(CliError::Failed);
                        }
                    },
                };
                let variant = read_or_fail(&fixture.path.join(filename))?;
                let (var_stream, var_runs) = split_gaps(&variant);
                if &var_stream != ref_stream {
                    unalignable.push(Unalignable {
                        fixture: fixture.relative_path.clone(),
                        file: filename.clone(),
                        direction,
                    });
                    continue;
                }
                graded.push(Graded {
                    fixture: fixture.relative_path.clone(),
                    file: filename.clone(),
                    direction,
                    verdict: grade(direction, ref_runs, &var_runs, &variant),
                });
            }
        }

        check_graded_nonzero(graded.len(), "`_compact`/`_spaces` variants graded")?;
        if graded.len() < VARIANTS_GRADED_MIN {
            eprintln!(
                "Error: pinned minimum — graded {} variants < pinned {VARIANTS_GRADED_MIN}. \
                 The fixtures walk shrank, or variant recognition broke (a bucket rename, a \
                 prefix typo, an extension mismatch); if deliberate, re-pin VARIANTS_GRADED_MIN.",
                graded.len()
            );
            return Err(CliError::Failed);
        }

        let failures: Vec<&Graded> = graded
            .iter()
            .filter(|g| !g.verdict.violations.is_empty())
            .collect();
        if self.json {
            print_json(&graded, &failures, &unalignable);
        } else {
            print_human(&graded, &failures, &unalignable, self.list);
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(CliError::Failed)
        }
    }
}

/// Per-file violation lines shown before truncating; the rest are summarized.
const MAX_SHOWN_VIOLATIONS: usize = 6;

fn print_human(graded: &[Graded], failures: &[&Graded], unalignable: &[Unalignable], list: bool) {
    let compact = graded
        .iter()
        .filter(|g| g.direction == Direction::Compact)
        .count();
    let spaces = graded.len() - compact;
    println!("Variant whitespace-direction audit");
    println!(
        "  graded: {} ({compact} compact, {spaces} spaces)",
        graded.len()
    );
    if !unalignable.is_empty() {
        let uc = unalignable
            .iter()
            .filter(|u| u.direction == Direction::Compact)
            .count();
        println!(
            "  not whitespace-only (ungraded): {} ({uc} compact, {} spaces)",
            unalignable.len(),
            unalignable.len() - uc
        );
    }

    if list {
        for g in graded {
            println!(
                "  [{}] emptied={:3} widened={:3}  {}/{}",
                g.direction.suffix(),
                g.verdict.emptied,
                g.verdict.widened,
                g.fixture,
                g.file
            );
        }
        // The blind spot is this gate's work list, so `--list` shows it too — a class
        // visible only under `--json` reads as a covered one.
        for u in unalignable {
            println!(
                "  [{}] UNGRADED (not whitespace-only)  {}/{}",
                u.direction.suffix(),
                u.fixture,
                u.file
            );
        }
    }

    if failures.is_empty() {
        println!("✓ every `_compact`/`_spaces` variant moves whitespace the way its name claims");
        return;
    }

    println!();
    println!(
        "{} variant(s) move whitespace the WRONG WAY:",
        failures.len()
    );
    for g in failures {
        println!(
            "\n  {}/{} — a `_{}` variant {} ({} site(s))",
            g.fixture,
            g.file,
            g.direction.suffix(),
            g.direction.violation_label(),
            g.verdict.violations.len()
        );
        for v in g.verdict.violations.iter().take(MAX_SHOWN_VIOLATIONS) {
            println!("      line {}: whitespace {}", v.line, v.detail);
        }
        if g.verdict.violations.len() > MAX_SHOWN_VIOLATIONS {
            println!(
                "      … {} more",
                g.verdict.violations.len() - MAX_SHOWN_VIOLATIONS
            );
        }
    }
    println!();
    println!(
        "A `_compact` variant must never widen a newline-free gap; a `_spaces` variant must \
         never empty one.\nFix the whitespace, or — when the mix is deliberate — rename the file \
         to `unformatted_mixed_spacing`.\nSee docs/fixture_naming.md §Standard Variant Names."
    );
}

fn print_json(graded: &[Graded], failures: &[&Graded], unalignable: &[Unalignable]) {
    let report = serde_json::json!({
        "graded_count": graded.len(),
        "failure_count": failures.len(),
        "failures": failures.iter().map(|g| serde_json::json!({
            "fixture": g.fixture,
            "file": g.file,
            "direction": g.direction.suffix(),
            "emptied": g.verdict.emptied,
            "widened": g.verdict.widened,
            "sites": g.verdict.violations.iter().map(|v| serde_json::json!({
                "line": v.line,
                "detail": v.detail,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "unalignable_count": unalignable.len(),
        "unalignable": unalignable.iter().map(|u| serde_json::json!({
            "fixture": u.fixture,
            "file": u.file,
            "direction": u.direction.suffix(),
        })).collect::<Vec<_>>(),
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report).unwrap_or_default()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_gaps_aligns_texts_that_differ_only_in_whitespace() {
        let (a_stream, a_runs) = split_gaps("if (cond) {\n\texpr;\n}");
        let (b_stream, b_runs) = split_gaps("if(cond){expr;}");
        assert_eq!(a_stream, b_stream);
        assert_eq!(a_runs.len(), b_runs.len());
        assert_eq!(a_runs.len(), a_stream.chars().count() + 1);
    }

    #[test]
    fn split_gaps_records_leading_and_trailing_runs() {
        let (stream, runs) = split_gaps("  a  ");
        assert_eq!(stream, "a");
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].1, "  ");
        assert_eq!(runs[1].1, "  ");
    }

    #[test]
    fn compact_that_only_removes_is_clean() {
        let (_, input) = split_gaps("if (cond) {\n\texpr;\n}");
        let variant = "if(cond){expr;}";
        let (_, var) = split_gaps(variant);
        let g = grade(Direction::Compact, &input, &var, variant);
        assert!(g.violations.is_empty());
        assert!(g.emptied > 0);
    }

    #[test]
    fn compact_that_pads_is_a_violation() {
        // `interface Empty {}` → `interface Empty {  }`: the brace gap widened.
        let (_, input) = split_gaps("interface Empty {}");
        let variant = "interface Empty {  }";
        let (_, var) = split_gaps(variant);
        let g = grade(Direction::Compact, &input, &var, variant);
        assert_eq!(g.widened, 1);
        assert_eq!(g.violations.len(), 1);
    }

    #[test]
    fn spaces_that_strips_is_a_violation() {
        // `h1 + p` → `h1+p`: two gaps emptied, the exact adjacent-sibling defect.
        let (_, input) = split_gaps("h1 + p {}");
        let variant = "h1+p {}";
        let (_, var) = split_gaps(variant);
        let g = grade(Direction::Spaces, &input, &var, variant);
        assert_eq!(g.emptied, 2);
        assert_eq!(g.violations.len(), 2);
    }

    #[test]
    fn reindentation_alone_never_trips_either_direction() {
        // A `_spaces` variant joining an indented input onto one line replaces
        // `\n\t\t` with `  ` — SHORTER, yet plainly the spaces direction. The
        // newline exclusion on WIDENED is what keeps this from reading as a
        // removal, and nothing was emptied.
        let (_, input) = split_gaps("f(\n\t\ta,\n\t\tb\n);");
        let variant = "f(  a,  b  );";
        let (_, var) = split_gaps(variant);
        let g = grade(Direction::Spaces, &input, &var, variant);
        assert_eq!(g.emptied, 0);
        assert!(g.violations.is_empty());
    }

    #[test]
    fn widened_ignores_gaps_that_carry_a_newline() {
        // Input's gap has a newline, variant's is horizontal-only and longer:
        // that is a compaction (the line was joined), not a widening.
        let (_, input) = split_gaps("a\nb");
        let variant = "a  b";
        let (_, var) = split_gaps(variant);
        let g = grade(Direction::Compact, &input, &var, variant);
        assert_eq!(g.widened, 0);
        assert!(g.violations.is_empty());
    }

    #[test]
    fn claimed_direction_takes_bare_suffixes_only() {
        assert_eq!(
            claimed_direction("unformatted_compact.svelte", ".svelte"),
            Some((Reference::Input, Direction::Compact))
        );
        assert_eq!(
            claimed_direction("unformatted_spaces.svelte", ".svelte"),
            Some((Reference::Input, Direction::Spaces))
        );
        // `_ours_` must be recognized as its own prefix, not as a qualified suffix.
        assert_eq!(
            claimed_direction("unformatted_ours_compact.svelte", ".svelte"),
            Some((Reference::Input, Direction::Compact))
        );
    }

    #[test]
    fn only_the_prettier_prefix_references_output_prettier() {
        // `unformatted_prettier_*` normalizes to `output_prettier.*` (N8), so that —
        // not `input` — is the form its whitespace was added to or removed from.
        assert_eq!(
            claimed_direction("unformatted_prettier_spaces.css", ".css"),
            Some((Reference::PrettierOutput, Direction::Spaces))
        );
        // Its two siblings normalize to input (N4/N5) and must not follow it there.
        for name in [
            "unformatted_compact.svelte",
            "unformatted_ours_compact.svelte",
        ] {
            assert_eq!(
                claimed_direction(name, ".svelte").map(|(r, _)| r),
                Some(Reference::Input)
            );
        }
    }

    #[test]
    fn claimed_direction_rejects_qualified_and_oracle_names() {
        // Qualified names make a narrower claim than the bare direction.
        assert_eq!(
            claimed_direction("unformatted_unicode_spaces.svelte", ".svelte"),
            None
        );
        assert_eq!(
            claimed_direction("unformatted_spaces_before_colon.svelte", ".svelte"),
            None
        );
        assert_eq!(
            claimed_direction("unformatted_compactish.svelte", ".svelte"),
            None
        );
        // Prettier's own output is never graded.
        assert_eq!(
            claimed_direction("prettier_variant_compact.svelte", ".svelte"),
            None
        );
        assert_eq!(claimed_direction("variant_compact.svelte", ".svelte"), None);
        // A wrong-extension name is not this fixture's variant at all.
        assert_eq!(claimed_direction("unformatted_compact.ts", ".svelte"), None);
    }

    #[test]
    fn line_of_counts_from_one() {
        assert_eq!(line_of("a\nb\nc", 0), 1);
        assert_eq!(line_of("a\nb\nc", 2), 2);
        assert_eq!(line_of("a\nb\nc", 4), 3);
    }
}
