use argh::FromArgs;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use tsv_cli::cli::input::ParserType;
use tsv_lang::printing::visual_width;
use tsv_lang::{PRINT_WIDTH, TAB_WIDTH};

use crate::audit::ratchet::{Ratchet, SnapshotKey, print_ratchet_skipped, refuse_narrowed_update};
use crate::audit::shape::{markup_head, name_run};
use crate::audit::sweep::{PristineSweep, sweep_pristine};
use crate::audit::vacuity::{FIXTURES_FORMATTED_MIN, check_formatted_min, check_graded_nonzero};
use crate::cli::CliError;

use super::profile::resolve_seed_files;

/// Audit for output lines that exceed the print width.
///
/// [conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy)
/// says tsv treats `printWidth` as a **hard limit** where prettier treats it as a soft target —
/// *a line tsv can break is a line tsv does break*. Nothing measured that claim until this
/// audit: it formats each seed and counts the columns of every output line.
///
/// **No other gate covers this, and two bugs proved the cost.** Both emitted an over-width
/// line, one was also non-idempotent, and `deno task check` stayed green throughout — every
/// standing gate is blind by construction. F1 / fuzz / round-trip see a fixed point that
/// reparses. The ledger and census see every comment intact. The injection ratchets perturb
/// comment gaps, blank lines and directives; none measures a column. `authoring:audit` asks
/// for ONE fixed point per document, not a good one. And `corpus:compare:format` grades
/// against prettier — which on the widest shape emits the over-width line **itself**, so the
/// oracle vouches for the bug.
///
/// **Seeds are the whole tree, and that is load-bearing.** Measuring `input.*` alone would
/// have caught neither bug: tsv holds the correct form stable, so the overrun appears only
/// when formatting an *alternate authoring*. The default corpus therefore sweeps every file
/// under `tests/fixtures` — the `unformatted_*` / `*_variant_*` / `output_prettier.*` siblings
/// included — and **formats** each rather than measuring it as committed. Verified rather than
/// assumed: with the mid-run comment fix reverted, all seven extra over-width lines came from
/// `unformatted_ours_compact.svelte` and `divergent_variant_packed.svelte`; the `input.*` side
/// did not move at all.
///
/// Graded as a RATCHET over `width_audit_known.txt` — a no-new-KINDS gate, not a debt list.
/// Most over-width lines are the sanctioned overruns §Print Width Philosophy names (a comment
/// or `<pre>` body tsv never rewraps, an unbreakable token, a lone braced module list), so
/// "zero" is not the target and a hard gate is not available. What the snapshot buys is that a
/// new *kind* of over-width line — the shape a width bug produces — fails.
///
/// ⚠️ **Why this measures the finished text rather than instrumenting the renderer.** The
/// tempting design is a render-time hook: a break opportunity IS a `Line` doc node, so "an
/// over-width line that still held a flat `Line`" needs no lexing and no carve-out list, and
/// forced overruns are silent by construction. That was built and **rejected on evidence**: it
/// is blind to exactly the class it was built for. The mid-run comment bug *removed* the break
/// point (it baked the boundary space into the preceding word), so there was no unspent `Line`
/// to find — reverting the fix left the render check reporting zero while the output grew
/// seven over-width lines. A missing seam is invisible to a check that looks for unspent
/// seams. Do not re-derive that design without re-testing it against a reverted fix.
///
/// Pure Rust — no Deno, no instrumentation feature.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "width_audit")]
pub struct WidthAuditCommand {
    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// print every over-width line rather than the per-shape rollup
    #[argh(switch)]
    verbose: bool,

    /// regenerate the ratchet snapshot (refused on a narrowed run)
    #[argh(switch)]
    update: bool,

    /// stop after N seed files (diagnostic; never with --update)
    #[argh(option, default = "0")]
    limit: usize,

    /// file paths, directories, or glob patterns (default: tests/fixtures)
    #[argh(positional)]
    paths: Vec<String>,
}

const SNAPSHOT_HEADER: &str = "\
# Print-width ratchet — every line is a KIND of over-width output line that exists today.
#
# ⚠️ Read this before treating it like the gap/blank ratchets: those pin BUGS and
# shrink toward empty. This one does NOT. Most over-width lines are the overruns
# `conformance_prettier.md` §Print Width Philosophy sanctions — a comment or <pre>
# body tsv never rewraps, an unbreakable token, a lone braced module list, a
# template interpolation. Those are correct output, so \"zero\" is not the target
# and a hard gate is not available. What this buys is that a new KIND of
# over-width line fails, which is the shape a width bug takes.
#
# One line per shape, TAB-delimited: the language bucket, how the over-width line
# OPENS, whether a block/markup comment CLOSES inside it (`-` = no), how it ENDS.
# Shapes rather than paths, so the snapshot is corpus-portable and survives a
# fixture move. (The report renders the three as `head…tail`, or `head…inner…tail`
# when a comment closes inside; the file keeps them as separate fields so a head
# that CONTAINS `…` — an elided prose line — still round-trips.)
#
# The head/tail pair is the key because the head alone does NOT discriminate:
# measured against the mid-run comment bug, a head-only key produced no new shape
# at all while `head…tail` produced `IDENT…-->` — a long word running into a
# comment, which is exactly what that bug emitted.
#
# `inner` is the third component, and it exists because the two ends alone let a
# WELD hide in the fattest shapes. `<!--…-->` alone carries 45% of the over-width
# lines, and every one of them is a SINGLE whole comment — a forced overrun, since
# tsv never rewraps a comment interior. So the only bug that silhouette can hide is
# two comments welded onto one line, and `inner` separates them: neither `-->` nor
# `*/` can occur inside the comment it closes.
#
# ⚠️ Triage note: a new `inner` shape is a QUESTION, not a verdict, and a weld is the
# LEAST likely answer. Over real JS the usual one is an ordinary JSDoc cast
# (`… /** @type {T} */ (expr) …`) — a real block comment closing mid-line, no bug.
# The next is the mirror false positive: a `*/` or `-->` inside a string, template,
# regex, or the text of a `//` comment, read as interior with no comment involved.
# Neither occurs over tests/fixtures, the corpus this file pins.
#
# A shape found but not pinned FAILS (a new kind of overrun — triage it against
# §Print Width Philosophy before pinning). A pinned shape that no longer fires
# FAILS (a fix landed, or the fixture that carried it moved — re-pin).
#
# Regenerate with `deno task width:audit:update`.
";

const REPIN_HINT: &str = "deno task width:audit:update";

/// One shape of over-width line: which language, how the line opens and ends, and whether a
/// comment closes between the two.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct WidthShape {
    /// The seed's language bucket — the same silhouette in TS and in Svelte markup comes from
    /// two different printers, so the bucket is part of the key rather than a report column.
    lang: &'static str,
    /// How the line opens ([`head_shape`]).
    head: String,
    /// What closes INSIDE the line ([`inner_shape`]) — `-` when nothing does.
    inner: String,
    /// How the line ends ([`tail_shape`]).
    tail: String,
}

impl WidthShape {
    /// The human rendering — the report's `head…tail` silhouette, with the interior spliced in
    /// (`head…-->…tail`) when a comment closes inside. Deliberately NOT the snapshot line: `…`
    /// is a character a head can legitimately CONTAIN (a prose line opening on an ellipsis),
    /// and a key that renders to a line it cannot parse back would fail the gate as
    /// new-and-stale at once, with `--update` writing the same unparseable line forever. So the
    /// display glues and the record keeps four fields.
    fn render(&self) -> String {
        if self.inner == NO_INNER {
            return format!("{}…{}", self.head, self.tail);
        }
        format!("{}…{}…{}", self.head, self.inner, self.tail)
    }
}

impl SnapshotKey for WidthShape {
    fn to_line(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}",
            self.lang, self.head, self.inner, self.tail
        )
    }

    fn from_line(line: &str) -> Option<Self> {
        let mut cols = line.split('\t');
        let lang = lang_from_bucket(cols.next()?)?;
        let head = cols.next()?.to_string();
        let inner = cols.next()?.to_string();
        let tail = cols.next()?.to_string();
        // A fifth column means the line is not this key's record — no field can hold a TAB
        // (they all come from a trimmed line), so an extra one is drift, not data. A line with
        // three columns is the pre-`inner` spelling, and falls out above as a missing tail.
        if cols.next().is_some() {
            return None;
        }
        Some(Self {
            lang,
            head,
            inner,
            tail,
        })
    }

    /// Every shape is pinnable — unlike the gap/blank audits' PANIC class there is no absolute
    /// sub-invariant here. A line is over width or it is not.
    fn is_pinnable(&self) -> bool {
        true
    }
}

/// One over-width output line.
struct Overrun {
    path: PathBuf,
    /// 1-based line number in the formatted output.
    line: usize,
    /// Visual width in columns (tabs expanded to [`TAB_WIDTH`]).
    width: usize,
    /// The line, trimmed and elided to a readable reproducer.
    excerpt: String,
    shape: WidthShape,
}

/// What one corpus walk produced.
struct Sweep {
    /// Over-width lines in walk order — the human report.
    overruns: Vec<Overrun>,
    /// Every shape seen, deduped — what the ratchet grades.
    shapes: BTreeSet<WidthShape>,
    /// The shared skip/format bookkeeping (the [`check_formatted_min`] vacuity guard reads
    /// `formatted`; panics are counted there, not gated here — the panic gates own that class).
    pristine: PristineSweep,
    /// Output lines measured, over-width or not — the denominator that makes the overrun
    /// count readable and the vacuity guard meaningful at line granularity.
    lines_measured: usize,
}

impl WidthAuditCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let default_paths = self.paths.is_empty();
        let mut narrowed: Vec<&'static str> = Vec::new();
        if !default_paths {
            narrowed.push("explicit paths");
        }
        if self.limit > 0 {
            narrowed.push("--limit");
        }
        refuse_narrowed_update(
            self.update,
            &narrowed,
            "the over-width line shapes over tests/fixtures",
            "SUBSET",
        )?;

        let files = resolve_seed_files(&self.paths, self.limit)?;
        let sweep = sweep_files(&files);

        if self.json {
            print_json(&sweep);
        } else {
            print_report(&sweep, self.verbose);
        }
        // Always, and to stderr: the default panic hook is suppressed for the
        // sweep, so this is the only place a crashing input is named. Ungated
        // by `--json` (which writes stdout) so `2>/dev/null` still parses.
        sweep.pristine.print_panic_sample();

        check_graded_nonzero(sweep.pristine.formatted, "files formatted")?;
        let full_run = narrowed.is_empty();
        if full_run {
            check_formatted_min(sweep.pristine.formatted, FIXTURES_FORMATTED_MIN)?;
        }

        let ratchet = ratchet();
        if self.update {
            ratchet.write_pinned(&sweep.shapes, "shape")?;
            return Ok(());
        }
        // Off the default corpus the snapshot doesn't apply — it pins the full default run, so
        // grading a narrowed one would call every unreached shape stale. A narrowed run is a
        // DIAGNOSTIC: it reports and exits 0, because "this subtree has over-width lines" is
        // the normal state (the sanctioned overruns are everywhere) and failing on it would
        // make the command useless for triage. It says so out loud — an exit-0 run that
        // printed findings and no verdict is exactly the shape that reads as a green gate.
        if !full_run {
            print_ratchet_skipped(&narrowed);
            return Ok(());
        }

        ratchet.grade_and_report(
            &sweep.shapes,
            "over-width shape",
            &format!("{} files", sweep.pristine.formatted),
            |shape| format!("[{}] {}", shape.lang, shape.render()),
        )
    }
}

/// Format every file (via the shared pristine sweep) and measure every output line.
fn sweep_files(files: &[PathBuf]) -> Sweep {
    let mut overruns = Vec::new();
    let mut shapes = BTreeSet::new();
    let mut lines_measured = 0;
    let pristine = sweep_pristine(files, |path, parser, _source, output| {
        for (i, line) in output.lines().enumerate() {
            lines_measured += 1;
            let width = visual_width(line, TAB_WIDTH);
            if width <= PRINT_WIDTH {
                continue;
            }
            // The measurement is taken on the RAW line; the trim only feeds the key and the
            // excerpt. That ordering is what makes `str::trim`'s wide Unicode class safe here —
            // it strips characters the printer treats as content (NBSP, U+FEFF), which would be
            // a soundness hole in a class that decided a *width*, and is only a coarser key in
            // one that decides a *shape*. Do not hoist it above the width test.
            let trimmed = line.trim();
            let shape = WidthShape {
                lang: lang_bucket(parser),
                head: head_shape(trimmed),
                inner: inner_shape(trimmed),
                tail: tail_shape(trimmed),
            };
            shapes.insert(shape.clone());
            overruns.push(Overrun {
                path: path.to_path_buf(),
                line: i + 1,
                width,
                excerpt: excerpt(trimmed),
                shape,
            });
        }
    });
    Sweep {
        overruns,
        shapes,
        pristine,
        lines_measured,
    }
}

/// The language bucket a seed's parser stands for — the key's first field.
fn lang_bucket(parser: ParserType) -> &'static str {
    match parser {
        ParserType::TypeScript => "ts",
        ParserType::Css => "css",
        ParserType::Svelte => "svelte",
    }
}

/// The inverse of [`lang_bucket`], for reading a snapshot line back.
///
/// Returns `None` on anything else, so a drifted snapshot line fails to parse rather than
/// grading as a live shape — a bucket that silently became valid would mask a real one.
fn lang_from_bucket(s: &str) -> Option<&'static str> {
    match s {
        "ts" => Some("ts"),
        "css" => Some("css"),
        "svelte" => Some("svelte"),
        _ => None,
    }
}

/// How an over-width line OPENS — a stable token standing for the construct that starts it.
///
/// The markup arm is the shared [`markup_head`] (`fabrication_audit` keys its snapshot on the
/// same alphabet); the rest is this audit's own ladder — the comment kinds, then a structural
/// keyword, then `IDENT` for any other name, then the raw character for a continuation line
/// that opens on punctuation. Coarse on purpose, so the key survives an ordinary fixture edit.
///
/// It is **one component** of the key: on its own it does not discriminate at all (see the
/// snapshot header) — [`tail_shape`] is what makes the pair discriminate, and [`inner_shape`]
/// is what keeps a weld out of the fattest pair.
///
/// Note the comment arms come first: a `//`-opening line is not markup, so the order is a
/// readability choice rather than a correctness one — but `<!--` IS both, and there the shared
/// arm and this one agree by construction.
fn head_shape(trimmed: &str) -> String {
    if trimmed.is_empty() {
        return "^".to_string();
    }
    if trimmed.starts_with("//") {
        return "//".to_string();
    }
    if trimmed.starts_with("/*") || trimmed.starts_with('*') {
        return "/*".to_string();
    }
    if let Some(head) = markup_head(trimmed) {
        return head;
    }
    let word = name_run(trimmed);
    if word.is_empty() {
        // Leading punctuation (a continuation line opening on `|`, `?`, `.`, `)`).
        trimmed.chars().next().map(String::from).unwrap_or_default()
    } else if is_structural_keyword(&word) {
        word
    } else {
        "IDENT".to_string()
    }
}

/// The [`WidthShape::inner`] value for a line nothing closes inside — the overwhelming
/// majority, and the one the report elides.
const NO_INNER: &str = "-";

/// What CLOSES inside an over-width line: `-->`, `*/`, both (`-->+*/`), or [`NO_INNER`].
///
/// The third component of the key, and the one that keeps a WELD from hiding behind the two
/// ends. The ends alone put a whole-comment line and a two-comments-welded-onto-one-line in the
/// same bucket — `<!-- a -->` and `<!-- a --><!-- b -->` both open `<!--` and end `-->`. That
/// matters because the fattest shape by a wide margin is exactly that one, and every one of
/// its lines is a single whole comment — so its members are FORCED overruns (tsv never rewraps
/// a comment interior) and a weld is the only bug the silhouette could ever hide. Same for
/// `/*…*/`, and the same weld class the trailing-run comment emitters have produced before.
/// The distribution behind that claim is in
/// [audits.md §Print-Width Audit](../../../../../docs/audits.md#print-width-audit-widthaudit).
///
/// A closer is interior when the line does not END on the FIRST one — enough on its own,
/// because if the earliest closer sits at the end it is the only one.
///
/// Textual, like the rest of the key, and sound in the direction that matters: neither `-->`
/// nor `*/` can occur inside the comment it closes, so a whole comment never reads as a weld.
///
/// ⚠️ A non-`-` `inner` is a QUESTION, not a verdict, and over real code the likeliest answer
/// is neither "weld" nor "false positive": it is an ordinary **JSDoc cast**
/// (`… /** @type {T} */ (expr) …`), a real block comment closing mid-line. Triaged over
/// `../svelte/packages/svelte/src` + `../zzz/src`, 9 of the 13 interior shapes are that,
/// and only 4 are the mirror case — a `*/` / `-->` inside a string, template, regex, or the
/// text of a `//` comment, called interior with no comment involved. There are none of either
/// over `tests/fixtures`, the only corpus the ratchet grades, and even off it a false one
/// surfaces as a new shape to triage rather than a wrong verdict on a pinned one (a run
/// pointed off the default corpus reports without grading). Counts in the audits doc above.
fn inner_shape(trimmed: &str) -> String {
    let closes: Vec<&str> = ["-->", "*/"]
        .into_iter()
        .filter(|close| {
            trimmed
                .find(close)
                .is_some_and(|i| i + close.len() < trimmed.len())
        })
        .collect();
    if closes.is_empty() {
        NO_INNER.to_string()
    } else {
        closes.join("+")
    }
}

/// How an over-width line ENDS.
///
/// This half is what makes the key discriminate: an over-width line's *ending* says what ran
/// past the limit (a comment closing, a word, an open delimiter), where its opening says only
/// where the construct began.
fn tail_shape(trimmed: &str) -> String {
    for close in ["-->", "*/"] {
        if trimmed.ends_with(close) {
            return close.to_string();
        }
    }
    let Some(last) = trimmed.chars().next_back() else {
        return "^".to_string();
    };
    if last.is_alphanumeric() || last == '_' || last == '$' {
        "WORD".to_string()
    } else {
        last.to_string()
    }
}

/// Keywords worth naming in a head shape — the ones that identify a construct rather than a
/// user's identifier. Anything else collapses to `IDENT` so the key survives a rename.
fn is_structural_keyword(word: &str) -> bool {
    matches!(
        word,
        "import"
            | "export"
            | "const"
            | "let"
            | "var"
            | "type"
            | "interface"
            | "declare"
            | "class"
            | "function"
            | "return"
            | "enum"
            | "namespace"
            | "module"
    )
}

/// A readable one-line reproducer: the line's head and tail with the middle elided, so a wide
/// line does not wrap the terminal and both ends of the shape stay visible.
fn excerpt(trimmed: &str) -> String {
    const EDGE: usize = 36;
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() <= EDGE * 2 + 3 {
        return trimmed.to_string();
    }
    let head: String = chars[..EDGE].iter().collect();
    let tail: String = chars[chars.len() - EDGE..].iter().collect();
    format!("{head} … {tail}")
}

/// The ratchet over this audit's colocated snapshot, carrying its header + re-pin hint.
fn ratchet() -> Ratchet {
    Ratchet::colocated("width_audit_known.txt", SNAPSHOT_HEADER, REPIN_HINT)
}

/// One shape's rollup: how many lines carry it, and the widest of them as the reproducer.
struct Rollup<'a> {
    count: usize,
    widest: &'a Overrun,
}

/// Group the overruns by shape in ONE pass, keeping the widest of each as its reproducer.
///
/// Keyed off the overruns rather than iterated per shape, so the walk is linear and every entry
/// provably has a hit — a per-shape filter would need a "no hits" arm that cannot happen
/// ([`Sweep::shapes`] is built from these same overruns) and would read as a real case.
fn rollups(overruns: &[Overrun]) -> BTreeMap<&WidthShape, Rollup<'_>> {
    let mut by_shape: BTreeMap<&WidthShape, Rollup<'_>> = BTreeMap::new();
    for o in overruns {
        by_shape
            .entry(&o.shape)
            .and_modify(|r| {
                r.count += 1;
                if o.width > r.widest.width {
                    r.widest = o;
                }
            })
            .or_insert(Rollup {
                count: 1,
                widest: o,
            });
    }
    by_shape
}

fn print_report(sweep: &Sweep, verbose: bool) {
    let skipped = sweep.pristine.skipped_note();
    println!(
        "width_audit — {} file(s) · {} output line(s) · {} over {PRINT_WIDTH} cols across {} shape(s) ({skipped})\n",
        sweep.pristine.formatted,
        sweep.lines_measured,
        sweep.overruns.len(),
        sweep.shapes.len(),
    );

    if sweep.overruns.is_empty() {
        return;
    }

    if verbose {
        for o in &sweep.overruns {
            println!("  {}:{} — {} cols", o.path.display(), o.line, o.width);
            println!("      {}", o.excerpt);
        }
        return;
    }

    // The per-shape rollup: one line per shape with a count, the widest hit, and one
    // reproducer, so a single prose-heavy fixture cannot bury the others. `--report` prints
    // every line.
    for (shape, roll) in rollups(&sweep.overruns) {
        println!(
            "  [{}] {} — {} line(s), widest {} cols",
            shape.lang,
            shape.render(),
            roll.count,
            roll.widest.width
        );
        println!("      {}:{}", roll.widest.path.display(), roll.widest.line);
        println!("      {}", roll.widest.excerpt);
    }
}

fn print_json(sweep: &Sweep) {
    let items: Vec<serde_json::Value> = sweep
        .overruns
        .iter()
        .map(|o| {
            serde_json::json!({
                "path": o.path.to_string_lossy(),
                "line": o.line,
                "width": o.width,
                "lang": o.shape.lang,
                "head": o.shape.head,
                "inner": o.shape.inner,
                "tail": o.shape.tail,
                "excerpt": o.excerpt,
            })
        })
        .collect();
    // `print_width` leads, as it always has — this audit's parameter before its
    // measurements. The sweep's fields splice in after it, which is why
    // `json_report` takes both halves instead of prepending a fixed block.
    let output = sweep.pristine.json_report(
        serde_json::json!({ "print_width": PRINT_WIDTH }),
        serde_json::json!({
            "lines_measured": sweep.lines_measured,
            "overruns": sweep.overruns.len(),
            "shapes": sweep.shapes.len(),
            "items": items,
        }),
    );
    #[allow(clippy::unwrap_used)]
    let s = serde_json::to_string_pretty(&output).unwrap();
    println!("{s}");
}

#[cfg(test)]
mod tests {
    use super::{
        NO_INNER, WidthShape, excerpt, head_shape, inner_shape, lang_bucket, lang_from_bucket,
        tail_shape,
    };
    use crate::audit::ratchet::SnapshotKey;
    use tsv_cli::cli::input::ParserType;

    fn shape(lang: &'static str, head: &str, tail: &str) -> WidthShape {
        WidthShape {
            lang,
            head: head.to_string(),
            inner: NO_INNER.to_string(),
            tail: tail.to_string(),
        }
    }

    #[test]
    fn shape_round_trips_through_a_snapshot_line() {
        let s = shape("svelte", "IDENT", "-->");
        assert_eq!(s.to_line(), "svelte\tIDENT\t-\t-->");
        assert_eq!(
            s.render(),
            "IDENT…-->",
            "the report glues what the record splits, and elides an empty interior"
        );
        let back = WidthShape::from_line("svelte\tIDENT\t-\t-->").expect("parses");
        assert_eq!(back.lang, "svelte");
        assert_eq!(back.head, "IDENT");
        assert_eq!(back.inner, NO_INNER);
        assert_eq!(back.tail, "-->");
    }

    /// The weld the third component exists to separate: a whole comment and two comments
    /// welded onto one line share both ends, so only the interior tells them apart.
    #[test]
    fn a_weld_is_a_different_shape_from_the_whole_comment_it_hid_behind() {
        let whole = "<!-- one long comment that runs past the print width -->";
        let weld = "<!-- one long comment --><!-- and a second welded onto it -->";
        assert_eq!(head_shape(whole), head_shape(weld));
        assert_eq!(tail_shape(whole), tail_shape(weld));
        assert_eq!(inner_shape(whole), NO_INNER);
        assert_eq!(inner_shape(weld), "-->");
        // The block-comment twin, and a line holding both kinds.
        assert_eq!(inner_shape("/* one long block comment */"), NO_INNER);
        assert_eq!(inner_shape("/* one *//* and two */"), "*/");
        assert_eq!(inner_shape("<!-- markup --> then /* block */"), "-->");
        assert_eq!(
            inner_shape("<!-- markup --> then /* block */ tail"),
            "-->+*/"
        );
        // A comment that closes the line is not interior, however long the line.
        assert_eq!(inner_shape("code(); // trailing"), NO_INNER);
        assert_eq!(inner_shape(""), NO_INNER);
    }

    /// A round trip through the record with every field non-trivial — the interior is a field,
    /// not a suffix on the tail, so a weld shape survives the snapshot unchanged.
    #[test]
    fn an_interior_closer_round_trips_and_renders_spliced() {
        let s = WidthShape {
            lang: "svelte",
            head: "IDENT".to_string(),
            inner: "-->".to_string(),
            tail: "WORD".to_string(),
        };
        assert_eq!(s.to_line(), "svelte\tIDENT\t-->\tWORD");
        assert_eq!(s.render(), "IDENT…-->…WORD");
        assert_eq!(WidthShape::from_line(&s.to_line()).expect("parses"), s);
    }

    /// ⚠️ The reason the record is three fields rather than the report's `head…tail`: `…` is a
    /// character a HEAD can hold (a prose line opening on an elided sentence), and a key that
    /// renders to a line parsing back as a *different* key is graded new-and-stale at once —
    /// a red gate `--update` cannot clear, because the re-pin writes the same line again.
    #[test]
    fn a_head_containing_the_display_separator_still_round_trips() {
        assert_eq!(head_shape("… continued prose that runs long"), "…");
        let s = shape("svelte", "…", "WORD");
        let back = WidthShape::from_line(&s.to_line()).expect("parses");
        assert_eq!(back, s, "rendered by {:?}", s.to_line());
        // Both ends at once — the shape a `…`-opened line ending in one takes.
        let both = shape("svelte", "…", "…");
        assert_eq!(
            WidthShape::from_line(&both.to_line()).expect("parses"),
            both
        );
    }

    /// A drifted snapshot line must NOT parse — one that silently became a valid key would
    /// grade as a live shape and mask a real one.
    #[test]
    fn a_malformed_snapshot_line_does_not_parse() {
        assert!(WidthShape::from_line("rust\tIDENT\t-\t-->").is_none());
        assert!(WidthShape::from_line("svelte\tno-tail").is_none());
        assert!(WidthShape::from_line("no-tab-at-all").is_none());
        // The pre-3-field spelling: one TAB, the pair glued. Reading it as `head = "IDENT…-->"`
        // would pin a shape no run can ever produce.
        assert!(WidthShape::from_line("svelte\tIDENT…-->").is_none());
        // The pre-`inner` spelling: three fields, the interior missing. Reading it as
        // `inner = "-->"` with no tail would pin a weld shape the run never produced.
        assert!(WidthShape::from_line("svelte\tIDENT\t-->").is_none());
        // A fifth field is drift too — no field can hold a TAB.
        assert!(WidthShape::from_line("svelte\tIDENT\t-\t-->\textra").is_none());
    }

    /// Every bucket `lang_bucket` can emit must parse back, or a new language would write
    /// snapshot lines that `from_line` drops — every pin for it reading STALE forever.
    #[test]
    fn buckets_cover_every_parser_and_round_trip() {
        for parser in [ParserType::TypeScript, ParserType::Css, ParserType::Svelte] {
            let bucket = lang_bucket(parser);
            assert_eq!(
                lang_from_bucket(bucket),
                Some(bucket),
                "{bucket} does not parse back"
            );
        }
        assert_eq!(lang_bucket(ParserType::TypeScript), "ts");
        assert_eq!(lang_bucket(ParserType::Css), "css");
        assert_eq!(lang_bucket(ParserType::Svelte), "svelte");
        assert_eq!(lang_from_bucket("rust"), None);
    }

    #[test]
    fn heads_drop_attributes_and_expressions() {
        assert_eq!(head_shape("<div class=\"a\" data-x=\"v\">"), "<div");
        assert_eq!(head_shape("</div>"), "</div");
        assert_eq!(head_shape("<!-- anything at all -->"), "<!--");
        assert_eq!(head_shape("// a line comment"), "//");
        assert_eq!(head_shape("/* a block comment */"), "/*");
        assert_eq!(head_shape("* a block comment continuation"), "/*");
        assert_eq!(head_shape("{#if cond}"), "{#if");
        assert_eq!(head_shape("{/await}"), "{/await");
        assert_eq!(head_shape("{@debug a, b}"), "{@debug");
        assert_eq!(head_shape("{expr}"), "{");
        assert_eq!(head_shape("import { a } from 'b';"), "import");
        // A user's identifier collapses, so a rename does not churn the snapshot.
        assert_eq!(head_shape("someUserFunction(a, b)"), "IDENT");
        assert_eq!(head_shape("otherName(a, b)"), "IDENT");
        // A rune opens a name run too (`$` is in the name class), so it collapses rather than
        // minting a `$` punctuation shape per rune.
        assert_eq!(head_shape("$effect(() => {});"), "IDENT");
        assert_eq!(head_shape("| SomeUnionMember"), "|");
        assert_eq!(head_shape(""), "^");
    }

    /// The continuation-line arm: a line opening on punctuation keys on that character, and
    /// the snapshot's live shapes exercise each of these.
    #[test]
    fn heads_of_continuation_lines_key_on_their_opening_character() {
        assert_eq!(head_shape("? aaa : bbb"), "?");
        assert_eq!(head_shape(").prop.method();"), ")");
        assert_eq!(head_shape("['a', 'b']"), "[");
        assert_eq!(head_shape("@decorator()"), "@");
        assert_eq!(head_shape("'a string literal'"), "'");
        assert_eq!(head_shape("`a template`;"), "`");
    }

    /// The tail half is what discriminates — these are the pairs that differ only there, which
    /// is the whole reason the key is not head-only.
    #[test]
    fn tails_separate_lines_that_share_a_head() {
        assert_eq!(tail_shape("xxxx <!--pad pad-->"), "-->");
        assert_eq!(tail_shape("xxxx some words"), "WORD");
        assert_eq!(tail_shape("call(a, b) {"), "{");
        assert_eq!(tail_shape("/* a block */"), "*/");
        assert_eq!(tail_shape("<pre>"), ">");
        assert_eq!(tail_shape("const a = `x`;"), ";");
        // A non-ASCII word character is still a WORD — the class is Unicode, so a CJK or
        // accented tail does not mint a per-character shape.
        assert_eq!(tail_shape("prose ending in café"), "WORD");
        assert_eq!(tail_shape(""), "^");
        // The pair the mid-run comment bug produced vs an ordinary prose overrun.
        assert_ne!(
            (head_shape("xxx <!--c-->"), tail_shape("xxx <!--c-->")),
            (head_shape("xxx more words"), tail_shape("xxx more words"))
        );
    }

    #[test]
    fn excerpts_elide_the_middle_and_keep_both_ends() {
        let short = "a short line";
        assert_eq!(excerpt(short), short);
        let long: String = "a".repeat(40) + &"b".repeat(40);
        let e = excerpt(&long);
        assert!(e.starts_with("aaaa"), "{e}");
        assert!(e.ends_with("bbbb"), "{e}");
        assert!(e.contains('…'), "{e}");
        // The elision boundary: 75 chars is kept whole, 76 is elided (EDGE * 2 + 3).
        let at_boundary: String = "c".repeat(75);
        assert_eq!(excerpt(&at_boundary), at_boundary);
        assert!(excerpt(&"c".repeat(76)).contains('…'));
        // Multibyte-safe: eliding by chars, never by bytes.
        let wide: String = "é".repeat(200);
        assert!(excerpt(&wide).contains('…'));
    }
}
