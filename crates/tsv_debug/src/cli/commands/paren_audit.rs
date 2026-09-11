//! Paren-authoring independence audit — same-operator logical re-association.
//!
//! ## Why this exists
//!
//! `a ?? b ?? c` and `a ?? (b ?? c)` are **one document**. Prettier says so
//! structurally: its parse postprocess (`../prettier/src/language-js/parse/postprocess/index.js`,
//! `onLeave(LogicalExpression)` → `rebalanceLogicalTree`) rewrites `a op (b op c)` into
//! `(a op b) op c` whenever `node.operator === node.right.operator`, recursively, for both
//! the `babel` and `typescript` parsers — so by the time its printer runs, the two
//! authorings ARE the same tree and print byte-identically.
//!
//! tsv cannot rebalance: acorn keeps the right-nested shape and tsv's wire must match it
//! (the drop-in AST contract, priority #1). So every rule that prettier asks of the
//! rebalanced tree, tsv must ask through a **rebalanced view** of its own
//! (`BinaryExpression::rebalanced_right`, named for prettier's `isUnbalancedLogicalTree`).
//! That is a per-rule obligation spread over six callers, and **a rule left on the raw
//! `binary.right` is invisible on every paren-free authoring** — which is every authoring
//! a formatted corpus contains, since tsv's own output normalizes the parens away.
//!
//! bug539 was exactly that: one half of `should_inline_logical_expression` reading
//! `binary.right`. It passed all ~4,300 fixtures and the whole 9,305-file corpus; only a
//! hand-written paren-nested twin exposed it. This audit is the standing instrument for
//! that class — nothing else enumerates the twin.
//!
//! ## The mutation (a pure two-character insertion)
//!
//! For every wire `LogicalExpression` `N` whose **left** is a `LogicalExpression` with the
//! **same operator** — i.e. a same-operator chain of three or more operands, which is what
//! a left-associative parse of `a op b op c` produces — insert `(` at `N.left.right.start`
//! and `)` at `N.end`. On `a op b op c` that yields `a op (b op c)`: the exact inverse of
//! prettier's rebalance, and the twin the class hides behind.
//!
//! Three facts make the splice sound, and each is a property of the acorn wire rather than
//! an assumption about the text:
//!
//! - **Both offsets are token boundaries.** `N.left.right.start` sits after the operator
//!   token (so the `(` can never glue onto a preceding identifier — the base is formatted,
//!   so whitespace separates them), and `N.end` is acorn's `lastTokEnd` for the whole node.
//! - **`N.end` is past every closing paren the node holds**, so a group opened inside
//!   `[open, close)` also closes inside it. The one unbalanced case is the mirror — a
//!   *parenthesized* `N.left.right`, whose own `(` sits before `open` (acorn's node span
//!   excludes it). The stray `)` then pairs with the inserted `(`, and the original `(`
//!   pairs with the inserted `)`, which re-parses as the same re-association one level out.
//!   So the variant always parses, and always parses as intended.
//! - **`&&` / `||` / `??` re-associate.** All three short-circuit left to right, so
//!   `(a op b) op c` and `a op (b op c)` evaluate the same operands in the same order to the
//!   same value. `LogicalExpression` carries exactly those three operators, which is why
//!   this keys on the node type and not on an operator list: a `BinaryExpression` (`+`, `&`)
//!   is NOT re-associable and prettier does not rebalance it.
//!
//! ## The verdict, and why it is zero-tolerance
//!
//! `format(variant)` must equal the base `F` — byte for byte, at every site. Unlike the
//! whitespace [`authoring_audit`](super::authoring_audit), which has a sanctioned dual-stable
//! remainder (a newline the author wrote is layout *intent*), a redundant paren carries no
//! intent that survives either formatter: prettier erases it at parse and tsv's printer
//! re-derives parens from the tree. There is no authoring signal here to preserve, so
//! **every divergence is a bug** and the audit is a hard gate rather than a ratchet.
//!
//! ## Blind spots
//!
//! - **Only the paren-INSERTING direction.** Sites are enumerated from `F`, tsv's own fixed
//!   point, which normalizes the parens away — so a chain reaches this audit paren-free and
//!   is probed toward the nested twin. Were tsv's fixed point ever to *retain* a redundant
//!   paren, that shape's paren-free twin would go unprobed here; it is also a prettier
//!   divergence, which `corpus:compare:format` grades.
//! - **One re-association step per node.** A four-operand chain contributes one site per
//!   same-operator node, each re-associating one level; the fully right-nested composition of
//!   those steps is not enumerated. One step is what turns `binary.right` from a leaf into a
//!   `LogicalExpression`, which is the whole signal.
//! - **Same-operator only**, because that is precisely what prettier rebalances.
//!   `a || (b && c)` is a different tree in both formatters and its parens are load-bearing.
//! - **A seed bearing a format-ignore directive is skipped** (`source_has_ignore_directive`):
//!   a frozen region reproduces the inserted parens verbatim, which is correct behavior and
//!   would read as a divergence.
//! - **A site whose `(` would land after a forward-binding block comment is excluded**
//!   ([`splits_forward_binding_comment`]) — the splice would re-bind a JSDoc cast or a bundler
//!   annotation, making the twin a different document. Counted in the report, not dropped
//!   silently.
//! - **CSS has no expression grammar**, so `.css` seeds hold no sites; the audit's subject
//!   filter is the Svelte + TypeScript families.
//!
//! ## Containment
//!
//! Each file's work runs under `catch_unwind` with the default panic hook suppressed
//! (`audit::panic_hook`), the same bracket the pristine sweep and the injection audits
//! use: a panic anywhere in a file — the base format, the wire walk, a variant format — is
//! recorded as a PANICKED entry (path + message, an exact count with a bounded sample) and
//! the walk continues, rather than one adversarial file killing a corpus run with its
//! findings unprinted. A panic FAILS the run: a crash on a seed is never a pass, and a
//! gate that reported it green would launder the loudest possible finding. Catching needs
//! the corpus profile's `panic = "unwind"`; a stack overflow is not a panic and still
//! aborts (the sized-stack contract in `tsv_cli::cli::stack` is what bounds that).
//!
// TODO: the general redundant-paren class — wrap ANY expression in parens and require one
// fixed point — is a strictly larger instrument this one is the first slice of. It needs a
// `preserve_parens` reparse to enumerate the removal direction, and its findings would mix
// the deliberate paren retentions (a JSDoc cast's shell, a required operand pair) with real
// bugs, so it wants its own triage rather than this gate's zero tolerance.

use argh::FromArgs;
use std::collections::BTreeMap;
use std::panic::AssertUnwindSafe;
use std::path::Path;

use serde_json::Value;

use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;

use crate::audit::excerpt::{first_line_diff, line_context};
use crate::audit::panic_hook::SuppressedPanicHook;
use crate::audit::properties::{
    BaseFixedPoint, Utf16ToByte, base_fixed_point, source_has_ignore_directive, tsv_parse_to_value,
};
use crate::audit::repro::{ReproCase, write_repro_case};
use crate::audit::tally::CappedPaths;
use crate::audit::vacuity::check_graded_nonzero;
use crate::cli::CliError;

use super::profile::{is_input_invalid_fixture, is_svelte, is_ts_family, resolve_seed_files_named};

/// Audit whether a same-operator logical chain formats identically to its
/// redundantly-parenthesized twin.
///
/// Prettier rebalances `a op (b op c)` into `(a op b) op c` at parse time, so the two
/// authorings are one document; tsv must reach one fixed point from both. Pure Rust — one
/// format per site, no sidecar. Defaults to `tests/fixtures` when no paths are given;
/// Svelte and TypeScript-family files (a `.css` seed holds no expressions).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "paren_audit")]
pub struct ParenAuditCommand {
    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// write a byte-exact repro (base / variant / ftry / ftry2) per finding into this dir
    #[argh(option)]
    dump_dir: Option<String>,

    /// cap the number of files audited (0 = unlimited)
    #[argh(option, default = "0")]
    limit: usize,

    /// cap the number of findings reported (default 20)
    #[argh(option, default = "20")]
    examples: usize,

    /// file paths, directories, or glob patterns (default: tests/fixtures)
    #[argh(positional)]
    paths: Vec<String>,
}

/// The three operators `LogicalExpression` carries — and exactly the three prettier
/// rebalances. Named rather than kept as the wire's string so the report's coverage
/// breakdown is a closed set: an operator with zero sites is a corpus gap the table shows.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum LogicalOp {
    And,
    Or,
    Nullish,
}

impl LogicalOp {
    fn from_wire(op: &str) -> Option<Self> {
        match op {
            "&&" => Some(Self::And),
            "||" => Some(Self::Or),
            "??" => Some(Self::Nullish),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::And => "&&",
            Self::Or => "||",
            Self::Nullish => "??",
        }
    }

    const ALL: [Self; 3] = [Self::And, Self::Or, Self::Nullish];
}

/// One re-association site in the formatted base: insert `(` at `open`, `)` at `close`.
#[derive(Clone, Copy, Debug)]
struct Site {
    open: usize,
    close: usize,
    op: LogicalOp,
}

/// A file's enumeration: the sites to probe, and a count of the ones excluded as unsound.
/// The exclusion is counted rather than dropped silently — an audit that quietly stops
/// probing a class reads exactly like one that finds nothing in it.
#[derive(Default)]
struct Sites {
    probed: Vec<Site>,
    comment_bound: usize,
}

/// Collect every same-operator re-association site in a wire AST.
///
/// The walk is over the wire `Value` rather than any language's internal AST because the
/// shape it looks for is identical in all of them: a `.ts` module and a `.svelte`
/// component's `<script>`, template expression, block head, attribute and `{@const}` all
/// serialize their expressions as the same acorn nodes at absolute file offsets. One walk
/// therefore covers both subject languages with no per-language enumerator.
fn collect_sites(node: &Value, f: &str, map: &Utf16ToByte, out: &mut Sites) {
    match node {
        Value::Array(items) => {
            for item in items {
                collect_sites(item, f, map, out);
            }
        }
        Value::Object(fields) => {
            if let Some(site) = site_at(fields, map) {
                // The one unsound splice, excluded rather than reported: see
                // [`splits_forward_binding_comment`].
                if splits_forward_binding_comment(f, site.open) {
                    out.comment_bound += 1;
                } else {
                    out.probed.push(site);
                }
            }
            for (key, value) in fields {
                // `loc` is the only object-valued field that carries no nodes; skipping it
                // is a cost saving, not a correctness one (it holds no `type`).
                if key != "loc" {
                    collect_sites(value, f, map, out);
                }
            }
        }
        _ => {}
    }
}

/// Whether a `(` inserted at `at` would land between a **forward-binding block comment** and
/// the token it binds — and so change what that comment annotates.
///
/// Two comment kinds bind forward: a JSDoc cast (`/** @type {T} */ (x)` — the comment plus the
/// parens ARE the cast) and a bundler annotation (`/* @__PURE__ */ f()`). Splicing a `(` after
/// one hands it a different subtree: `err && /** @type {any} */ (err).name && …` becomes
/// `err && /** @type {any} */ ((err).name && …)`, where the cast now annotates the whole
/// chain. That is a different document, so requiring the two to format alike would be asking
/// the formatter to erase the distinction — the finding would be about this instrument, not
/// about tsv. (Both of the corpus's only two divergences were exactly this.) The paren-vs-comment
/// binding question is `binding_audit`'s subject, where a migrating paren IS the finding.
///
/// A **line** comment is deliberately still in scope: it runs to the end of its line, so
/// anything it precedes sits on the next line and it is that line's trailing comment — it binds
/// backward and cannot be a cast or an annotation. The one line comment this coarse test cannot
/// tell from a block close is one whose own text ends in `*/` (`// see /* … */`); that site is
/// over-excluded, which is the safe direction and is why the count is reported.
///
/// The trim is Rust's `char::is_whitespace`, wider than JS's `WhiteSpace` production. Wrong in
/// the safe direction too: a wider class only skips more sites, where a narrower one would probe
/// a splice this is meant to exclude.
fn splits_forward_binding_comment(f: &str, at: usize) -> bool {
    f[..at].trim_end().ends_with("*/")
}

/// The site this node is, if it is one: a `LogicalExpression` whose left operand is a
/// `LogicalExpression` with the same operator.
fn site_at(fields: &serde_json::Map<String, Value>, map: &Utf16ToByte) -> Option<Site> {
    if fields.get("type")?.as_str()? != "LogicalExpression" {
        return None;
    }
    let op = LogicalOp::from_wire(fields.get("operator")?.as_str()?)?;
    let left = fields.get("left")?.as_object()?;
    if left.get("type")?.as_str()? != "LogicalExpression"
        || LogicalOp::from_wire(left.get("operator")?.as_str()?)? != op
    {
        return None;
    }
    // The inner right operand's start is where the redundant group opens; the OUTER node's
    // end is where it closes — never the outer right operand's end, which for a
    // parenthesized operand stops before its own `)`.
    let open = map.byte(left.get("right")?.as_object()?.get("start")?.as_u64()? as usize)?;
    let close = map.byte(fields.get("end")?.as_u64()? as usize)?;
    (open < close).then_some(Site { open, close, op })
}

/// Splice one site's redundant parens into the formatted base.
fn splice(f: &str, site: &Site) -> String {
    let mut out = String::with_capacity(f.len() + 2);
    out.push_str(&f[..site.open]);
    out.push('(');
    out.push_str(&f[site.open..site.close]);
    out.push(')');
    out.push_str(&f[site.close..]);
    out
}

/// What one site's probe found.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Verdict {
    /// The twin formats back to the base — the property holds.
    Converge,
    /// The twin formats to a second stable output: two fixed points for one document.
    Diverge,
    /// The twin's output is not even a fixed point of its own — F1 broken on the twin.
    NonIdempotent,
    /// The twin does not parse. The splice's soundness argument says this cannot happen,
    /// so it is a finding about the instrument (or the parser), never a pass.
    VariantParseError,
}

impl Verdict {
    fn key(self) -> &'static str {
        match self {
            Self::Converge => "converge",
            Self::Diverge => "diverge",
            Self::NonIdempotent => "diverge_non_idempotent",
            Self::VariantParseError => "variant_parse_error",
        }
    }

    /// Everything but [`Self::Converge`] fails the gate — see the module docs on why this
    /// audit has no sanctioned dual-stable remainder.
    fn is_finding(self) -> bool {
        self != Self::Converge
    }

    const ALL: [Self; 4] = [
        Self::Converge,
        Self::Diverge,
        Self::NonIdempotent,
        Self::VariantParseError,
    ];
}

/// One reported finding, carrying enough to reproduce it by hand.
struct Finding {
    path: String,
    offset: usize,
    op: LogicalOp,
    verdict: Verdict,
    context: String,
    /// The first line at which `format(variant)` departs from the base, as
    /// `(line number, base line, variant line)`. `None` when the twin failed to parse, and
    /// when the two are line-for-line equal yet not byte-equal (they differ only in a
    /// trailing newline) — there is no line to show either way.
    first_diff: Option<(usize, String, String)>,
}

#[derive(Default)]
struct Report {
    files_scanned: usize,
    files_parse_error: usize,
    /// Files that could not be read — never scanned.
    files_read_error: usize,
    files_output_reparse_error: usize,
    files_ignore_directive: usize,
    /// Seeds whose own format is not a fixed point: excluded from the re-association
    /// analysis (there is nothing for the twin to converge ON) and a failure of the run.
    /// A [`CappedPaths`] rather than a count beside a `Vec`, so the number the report
    /// prints cannot drift from the list under it and the list stays bounded on a corpus
    /// that has many.
    base_non_idempotent: CappedPaths,
    /// Files whose work PANICKED (`path: message`) — caught per file so the walk finishes,
    /// counted exactly with a bounded sample, and a failure of the run. See the module
    /// docs' §Containment. Counts recorded before the panic (sites, verdicts) stay in the
    /// report; each was complete when it was taken.
    panics: CappedPaths,
    sites: usize,
    sites_comment_bound: usize,
    counts: BTreeMap<Verdict, usize>,
    op_counts: BTreeMap<(LogicalOp, Verdict), usize>,
    findings: Vec<Finding>,
    /// Sequence number for `--dump-dir` case directories, so two findings in one file do not
    /// collide on a name.
    dump_seq: usize,
}

impl Report {
    fn count(&self, v: Verdict) -> usize {
        self.counts.get(&v).copied().unwrap_or(0)
    }

    fn op_count(&self, op: LogicalOp, v: Verdict) -> usize {
        self.op_counts.get(&(op, v)).copied().unwrap_or(0)
    }

    fn findings_total(&self) -> usize {
        Verdict::ALL
            .iter()
            .filter(|v| v.is_finding())
            .map(|v| self.count(*v))
            .sum()
    }

    /// Whether this run FAILS — the one question the exit code and the report's ✓ are two
    /// readings of, so the two cannot disagree. A base-non-idempotent file fails the run
    /// while producing no site finding at all, and that is exactly the case a ✓ keyed on the
    /// findings alone prints a pass over (`../prettier/tests/format` holds six such files).
    /// A panicking file is the same shape one step louder.
    fn failed(&self) -> bool {
        self.findings_total() + self.base_non_idempotent.count() + self.panics.count() > 0
    }
}

impl ParenAuditCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        // A scan over nothing must not read as a pass — resolution fails loud on an empty
        // subject set, naming it rather than the raw walk.
        let files = resolve_seed_files_named(
            &self.paths,
            self.limit,
            ".svelte / TypeScript-family files",
            |p| is_svelte(p) || is_ts_family(p),
        )?;

        let mut report = Report::default();
        {
            // Suppressed for the walk only: a caught panic is recorded below, and the
            // default hook's per-file backtrace would bury the report. Restored on drop.
            let _hook = SuppressedPanicHook::install();
            for path in &files {
                let scanned = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    self.scan_file(path, &mut report);
                }));
                if let Err(payload) = scanned {
                    report.panics.push_panic(path, payload.as_ref());
                }
            }
        }

        if self.json {
            print_json(&report);
        } else {
            print_human(&report, self.examples);
        }

        // A base-non-idempotent file is excluded from the re-association analysis (its fixed
        // point is undefined, so "formats back to the base" is meaningless) — but excluding
        // it is not a reason to pass the run, or a whole-file reflow could sit here green.
        if report.failed() {
            return Err(CliError::Failed);
        }
        // The floor UNDER the non-empty resolution: this audit's product is per-SITE, so a
        // corpus of files that hold no same-operator chain renders exactly the tables a
        // clean run does. The resolution's non-empty file list cannot see that.
        check_graded_nonzero(report.sites, "logical re-association sites")
    }

    fn scan_file(&self, path: &Path, report: &mut Report) {
        if is_input_invalid_fixture(path) {
            return;
        }
        let Ok(source) = std::fs::read_to_string(path) else {
            report.files_read_error += 1;
            return;
        };
        // A frozen region reproduces the inserted parens verbatim — correct behavior that
        // would read as a divergence. Same coarse pre-scan the injection audits exempt on.
        if source_has_ignore_directive(&source) {
            report.files_ignore_directive += 1;
            return;
        }
        let parser = ParserType::from_extension(&path.to_string_lossy());
        report.files_scanned += 1;
        let f = match base_fixed_point(&source, parser) {
            BaseFixedPoint::Ok(f) => f,
            BaseFixedPoint::ParseError => {
                report.files_parse_error += 1;
                return;
            }
            BaseFixedPoint::NonIdempotent => {
                report.base_non_idempotent.push(path.display().to_string());
                return;
            }
        };
        let Some(wire) = tsv_parse_to_value(&f, parser) else {
            // The base formatted but its own output does not reparse — `roundtrip_audit`'s
            // property, gated there. Counted apart from a seed the parser rejects: those are
            // ordinary (a `tsv_rejects` fixture, a prettier-rejects input), this is a bug
            // somewhere, and folding the two would hide it in the ordinary count.
            report.files_output_reparse_error += 1;
            return;
        };
        let map = Utf16ToByte::new(&f);
        let mut sites = Sites::default();
        collect_sites(&wire, &f, &map, &mut sites);
        report.sites += sites.probed.len();
        report.sites_comment_bound += sites.comment_bound;
        for site in &sites.probed {
            self.probe(path, &f, site, parser, report);
        }
    }

    fn probe(&self, path: &Path, f: &str, site: &Site, parser: ParserType, report: &mut Report) {
        let variant = splice(f, site);
        let formatted = format_source(&variant, parser).ok();
        let (verdict, first_diff) = match &formatted {
            None => (Verdict::VariantParseError, None),
            Some(ftry) if ftry == f => (Verdict::Converge, None),
            Some(ftry) => {
                let stable = format_source(ftry, parser).is_ok_and(|x| &x == ftry);
                let verdict = if stable {
                    Verdict::Diverge
                } else {
                    Verdict::NonIdempotent
                };
                (verdict, describe_first_diff(f, ftry))
            }
        };
        *report.counts.entry(verdict).or_default() += 1;
        *report.op_counts.entry((site.op, verdict)).or_default() += 1;
        if verdict.is_finding() {
            if let Some(dir) = &self.dump_dir {
                report.dump_seq += 1;
                write_repro_case(
                    dir,
                    &ReproCase {
                        seq: report.dump_seq,
                        tag: verdict.key(),
                        mutation: "splice one site's redundant parens",
                        src_path: path,
                        base: f,
                        variant: &variant,
                        ftry: formatted.as_deref().unwrap_or(""),
                        parser,
                    },
                );
            }
            report.findings.push(Finding {
                path: path.display().to_string(),
                offset: site.open,
                op: site.op,
                verdict,
                context: line_context(f, site.open),
                first_diff,
            });
        }
    }
}

/// [`first_line_diff`] in this report's own terms: owned lines, with the run-out side named.
fn describe_first_diff(base: &str, variant: &str) -> Option<(usize, String, String)> {
    first_line_diff(base, variant).map(|(line, b, v)| {
        let spell = |s: Option<&str>| s.unwrap_or("<eof>").to_string();
        (line, spell(b), spell(v))
    })
}

fn print_human(report: &Report, examples: usize) {
    println!("Paren-authoring independence audit (same-operator logical re-association)");
    println!(
        "  files: {} scanned, {} parse-error, {} read-error, {} output-reparse-error, {} ignore-directive (skipped), {} base-non-idempotent",
        report.files_scanned,
        report.files_parse_error,
        report.files_read_error,
        report.files_output_reparse_error,
        report.files_ignore_directive,
        report.base_non_idempotent.count(),
    );
    println!(
        "  sites probed: {} ({} excluded — a `(` there would re-bind a forward-binding comment)",
        report.sites, report.sites_comment_bound,
    );
    println!();
    println!("  verdicts:");
    println!(
        "    converge:                  {}",
        report.count(Verdict::Converge)
    );
    println!(
        "    DIVERGE (dual-stable):     {}",
        report.count(Verdict::Diverge)
    );
    println!(
        "    DIVERGE (non-idempotent):  {}",
        report.count(Verdict::NonIdempotent)
    );
    println!(
        "    variant parse error:       {}",
        report.count(Verdict::VariantParseError)
    );

    println!();
    println!("  by operator (a zero row is a corpus gap, not a pass):");
    print!("    {:<10}", "operator");
    for v in Verdict::ALL {
        print!("{:>24}", v.key());
    }
    println!();
    for op in LogicalOp::ALL {
        print!("    {:<10}", op.label());
        for v in Verdict::ALL {
            print!("{:>24}", report.op_count(op, v));
        }
        println!();
    }

    if !report.base_non_idempotent.is_empty() {
        println!();
        println!("  ✗ base-non-idempotent files (F1 broken — excluded from the re-association");
        println!("    analysis, but a failure in their own right):");
        for line in report.base_non_idempotent.sample_lines("    ") {
            println!("{line}");
        }
    }

    if !report.panics.is_empty() {
        println!();
        println!(
            "  ⚠ {} file(s) PANICKED (caught per file so the walk finished; a failure of the run):",
            report.panics.count()
        );
        for line in report.panics.sample_lines("    ") {
            println!("{line}");
        }
    }

    if report.findings.is_empty() {
        // Only claim the property when something actually carried it AND nothing else failed
        // the run: a run that graded no site is about to fail the vacuity floor, and one
        // holding a base-non-idempotent file has already printed the ✗ list above and exits
        // 1. A ✓ over either reads as a pass the run is not entitled to.
        if report.sites > 0 && !report.failed() {
            println!();
            println!("  ✓ every same-operator chain formats identically to its parenthesized twin");
        }
        return;
    }

    println!();
    println!("  ✗ findings (a redundant paren changed the output):");
    for finding in report.findings.iter().take(examples) {
        println!();
        println!(
            "    {}:{}  {}  [{}]",
            finding.path,
            finding.offset,
            finding.op.label(),
            finding.verdict.key(),
        );
        println!("      «{}»", finding.context);
        if let Some((line, base, variant)) = &finding.first_diff {
            println!("      first diff at line {line}:");
            println!("        base:    {}", base.trim_end());
            println!("        variant: {}", variant.trim_end());
        }
    }
    let shown = report.findings.len().min(examples);
    if report.findings.len() > shown {
        println!();
        println!(
            "    … {} more (raise --examples)",
            report.findings.len() - shown
        );
    }
}

fn print_json(report: &Report) {
    let counts: serde_json::Map<String, Value> = Verdict::ALL
        .iter()
        .map(|v| (v.key().to_string(), serde_json::json!(report.count(*v))))
        .collect();
    let op_counts: serde_json::Map<String, Value> = LogicalOp::ALL
        .iter()
        .map(|op| {
            let per: serde_json::Map<String, Value> = Verdict::ALL
                .iter()
                .map(|v| {
                    (
                        v.key().to_string(),
                        serde_json::json!(report.op_count(*op, *v)),
                    )
                })
                .collect();
            (op.label().to_string(), Value::Object(per))
        })
        .collect();
    let findings: Vec<Value> = report
        .findings
        .iter()
        .map(|f| {
            serde_json::json!({
                "path": f.path,
                "offset": f.offset,
                "operator": f.op.label(),
                "verdict": f.verdict.key(),
                "context": f.context,
                "first_diff": f.first_diff.as_ref().map(|(line, base, variant)| serde_json::json!({
                    "line": line,
                    "base": base,
                    "variant": variant,
                })),
            })
        })
        .collect();
    let out = serde_json::json!({
        "files_scanned": report.files_scanned,
        "files_parse_error": report.files_parse_error,
        "files_read_error": report.files_read_error,
        "files_output_reparse_error": report.files_output_reparse_error,
        "files_ignore_directive": report.files_ignore_directive,
        "files_base_non_idempotent": report.base_non_idempotent.count(),
        "files_panicked": report.panics.count(),
        "sites": report.sites,
        "sites_comment_bound": report.sites_comment_bound,
        "counts": counts,
        "op_counts": op_counts,
        "findings": findings,
        "base_non_idempotent_sample": report.base_non_idempotent.sample(),
        "panicked_sample": report.panics.sample(),
    });
    super::print_json_pretty(&out);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A caught panic is a failure of the run even with zero site findings — the same
    /// shape as a base-non-idempotent file, one step louder.
    #[test]
    fn a_panicking_file_fails_the_run() {
        let mut report = Report {
            sites: 1,
            ..Report::default()
        };
        assert!(!report.failed());
        report.panics.push("seed.ts: boom".to_string());
        assert!(report.failed());
    }

    /// Enumerate the sites of a TypeScript snippet, as the audit does.
    fn sites_of(source: &str) -> Vec<Site> {
        let wire = tsv_parse_to_value(source, ParserType::TypeScript).expect("parses");
        let map = Utf16ToByte::new(source);
        let mut out = Sites::default();
        collect_sites(&wire, source, &map, &mut out);
        out.probed
    }

    /// The variants a source's sites produce, in enumeration order.
    fn variants_of(source: &str) -> Vec<String> {
        sites_of(source).iter().map(|s| splice(source, s)).collect()
    }

    #[test]
    fn three_operand_chain_is_one_site() {
        assert_eq!(
            variants_of("const x = a ?? b ?? c;"),
            ["const x = a ?? (b ?? c);"]
        );
    }

    #[test]
    fn two_operand_chain_is_no_site() {
        assert!(sites_of("const x = a ?? b;").is_empty());
    }

    /// A four-operand chain is `((a op b) op c) op d` — one site per same-operator node, each
    /// re-associating one level. The fully right-nested `a && (b && c && d)` is a composition
    /// of two steps and is deliberately not enumerated: one step is what turns `binary.right`
    /// from a leaf into a `LogicalExpression`, which is the whole signal.
    #[test]
    fn each_chain_level_is_its_own_site() {
        let mut got = variants_of("const x = a && b && c && d;");
        got.sort();
        assert_eq!(
            got,
            [
                "const x = a && (b && c) && d;",
                "const x = a && b && (c && d);",
            ]
        );
    }

    /// A mixed-operator chain is not re-associable and prettier does not rebalance it.
    #[test]
    fn mixed_operators_are_no_site() {
        assert!(sites_of("const x = a || b && c;").is_empty());
        assert!(sites_of("const x = (a ?? b) || c;").is_empty());
    }

    /// `+` is a `BinaryExpression`: associative for neither strings nor floats, and never
    /// rebalanced. Keying on the node type is what excludes it.
    #[test]
    fn arithmetic_is_no_site() {
        assert!(sites_of("const x = a + b + c;").is_empty());
    }

    /// The close offset is the OUTER node's end, so it lands past a parenthesized right
    /// operand's own `)` rather than inside it.
    #[test]
    fn parenthesized_right_operand_closes_outside_its_paren() {
        assert_eq!(
            variants_of("const x = a ?? b ?? (c && d);"),
            ["const x = a ?? (b ?? (c && d));"]
        );
    }

    /// A parenthesized inner right operand leaves a stray `)` inside the spliced range; the
    /// inserted parens re-pair around it and the variant still parses as the intended
    /// re-association.
    #[test]
    fn parenthesized_inner_operand_still_parses_as_intended() {
        let source = "const x = a || (b && c) || d;";
        let variants = variants_of(source);
        assert_eq!(variants, ["const x = a || ((b && c) || d);"]);
        assert!(tsv_parse_to_value(&variants[0], ParserType::TypeScript).is_some());
    }

    /// A JSDoc cast before the operand makes the splice unsound — the `(` would re-bind the
    /// cast to the whole chain — so the site is excluded, not probed.
    #[test]
    fn cast_bound_operand_is_excluded() {
        assert!(sites_of("const x = a && /** @type {any} */ (b).c && d;").is_empty());
        // …and the same chain without the cast IS probed, so the exclusion is keyed on the
        // comment rather than on the shape around it.
        assert_eq!(sites_of("const x = a && (b).c && d;").len(), 1);
    }

    /// A line comment binds backward — it cannot be a cast or an annotation — so an operand
    /// it precedes stays in scope.
    #[test]
    fn line_comment_before_operand_stays_in_scope() {
        assert_eq!(sites_of("const x =\n\ta && // c\n\tb && c;").len(), 1);
    }

    /// A base-non-idempotent file fails the run while producing no site finding — the case
    /// the report's ✓ has to read from the same question the exit code does, or it prints a
    /// pass over a run that exits 1. `../prettier/tests/format` is a real corpus that lands
    /// here (six such files, zero site findings).
    #[test]
    fn a_base_non_idempotent_file_fails_the_run_with_no_findings() {
        let mut report = Report {
            sites: 12,
            ..Report::default()
        };
        report.base_non_idempotent.push("seed.svelte".to_string());
        assert!(report.findings.is_empty());
        assert_eq!(report.findings_total(), 0);
        assert!(report.failed());
    }

    /// The walk is over the wire, so a Svelte component's script, template expression and
    /// block head all enumerate through the same code path.
    #[test]
    fn svelte_expressions_enumerate() {
        let source =
            "<script>const x = a ?? b ?? c;</script>\n{#if p && q && r}<b>{u || v || w}</b>{/if}\n";
        let wire = tsv_parse_to_value(source, ParserType::Svelte).expect("parses");
        let map = Utf16ToByte::new(source);
        let mut out = Sites::default();
        collect_sites(&wire, source, &map, &mut out);
        let mut ops: Vec<&str> = out.probed.iter().map(|s| s.op.label()).collect();
        ops.sort_unstable();
        assert_eq!(ops, ["&&", "??", "||"]);
    }
}
