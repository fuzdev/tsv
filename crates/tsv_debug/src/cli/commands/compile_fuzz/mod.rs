//! Differential compile fuzzer — the compiler's adversarial leg.
//!
//! ## Why this exists
//!
//! The formatter has `fuzz` / `gaps` / `blanks` / `roundtrip` / `binding` /
//! `authoring` plus the print-once comment ledger: layered machinery that generates
//! inputs nobody wrote. The compiler has **one corpus plus human review** — and that
//! corpus's blind spot is empirically established, not theoretical: the interaction
//! bugs found by hand were all invisible while the full corpus was green. A corpus
//! containing every feature can still miss nearly every feature *pair*.
//!
//! So this generates feature *cross-products* and grades each against the oracle.
//!
//! ## Grading
//!
//! Two verdicts are bugs by the refusal contract, and both are HARD:
//!
//! - **MISMATCH** — both compilers produced output and the canonical reprints
//!   differ. tsv's contract is refuse-or-match, so a difference is never acceptable.
//! - **OVER-ACCEPTANCE** — the oracle *rejected* the mutant and tsv compiled it
//!   anyway. Nothing invalid in runes mode may compile, so this is the same severity
//!   (and the same gate) as a mismatch.
//!
//! A tsv **refusal** is a clean "not yet", never a finding. A tsv *parse* rejection
//! is bucketed and reported but not gated: it is a frontend (`tsv_svelte`) question,
//! and mutants reach shapes no real component has.
//!
//! The parity bar is [`compare_canonical`] — the same bar `compile_compare` and
//! `compile_corpus_compare` use, so a comment-POSITION difference (tsv's placement
//! vs the oracle's esrap) is tolerated and a remaining diff is a real code
//! difference.
//!
//! ## Throughput: the tsv-first pre-filter
//!
//! The one real lever. tsv's `compile()` is pure Rust and ~10–40× faster than a warm
//! oracle round trip, and a refusal is *definitionally* outside the target set
//! above — neither a mismatch nor an over-acceptance can involve a refused mutant.
//! So tsv runs **first** and the sidecar is skipped entirely on `Unsupported`. The
//! run reports the resulting pass-through rate; it is the number the throughput
//! model rests on.
//!
//! (`compile_corpus_compare` calls the oracle first, deliberately, for its own
//! per-file bookkeeping — that ordering is right there and this one is right here.)
//!
//! Everything else is the simple thing on purpose. A warm pooled sidecar sustains
//! hundreds of round trips per second, so there is **no batching protocol and no
//! result cache**: a content-addressed cache would be sound (the oracle is pinned
//! deterministic) but has a near-0% hit rate on freshly generated mutants.
//!
//! ## Determinism
//!
//! Every mutant is generated up front, single-threaded, from **per-seed-file
//! path-keyed PRNG streams** ([`stream_seed`]) scheduled round-robin. Grading then
//! fans out over the sidecar pool, and results are re-sorted by mutant index before
//! reporting, so the report is a pure function of `--seed` + `--iterations` + the
//! corpus. A given `--seed` reproduces a run exactly, independent of `--jobs`.
//!
//! **Corpus-add stability holds, with one honest exception.** A seed file's mutant
//! sequence is a pure function of `(master seed, that file's path, k)`, so adding,
//! removing, or renaming a fixture leaves every other file's sequence byte-identical
//! and only trims or extends its tail (`generate_is_corpus_add_stable` pins this).
//! **[`Operator::SpliceDonor`] is outside that guarantee by construction**: it grafts
//! material from *another* seed, so it reads the whole corpus, and a corpus edit
//! changes which donor a given draw selects for every seed. That is inherent to a
//! cross-product engine rather than a fixable defect — but it does mean this fuzzer's
//! stability property is weaker than the formatter fuzzer's, and a fixture add can
//! shift the donor-grafted mutants. Nine of the ten operators are unaffected.

mod anchors;
mod operators;

use anchors::Anchors;
use argh::FromArgs;
use futures_util::StreamExt;
use operators::{Donor, Operator};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tsv_cli::json_utils::to_json_with_tabs;
use tsv_svelte_compile::{
    CompileError, CompileOptions, Parity, canonicalize_js, compare_canonical, compile,
};

use super::fuzz::{Rng, stream_seed};
use crate::cli::CliError;
use crate::deno::{self, DenoError, SvelteGenerate};
use crate::diff::{ColorChoice, DiffOptions, diff_to_string};

/// Differential compile fuzzer: generate feature cross-products from the compile
/// fixtures and grade each against the canonical Svelte compiler.
///
/// A MISMATCH (both compiled, canonical code differs) and an OVER-ACCEPTANCE (the
/// oracle rejected it, tsv compiled it) are both bugs by the refusal contract and
/// both fail the run; a tsv refusal is a clean "not yet". tsv's compile runs FIRST
/// and a refusal skips the sidecar entirely.
///
/// Deterministic for a given `--seed` + corpus, and corpus-add-stable: each seed
/// file's mutants come from its own path-keyed stream. Sidecar-dependent, so it is
/// NOT part of `deno task check` — run it on demand (`deno task compile:fuzz`).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "compile_fuzz")]
pub struct CompileFuzzCommand {
    /// PRNG master seed (default 0) — same seed + corpus ⇒ identical run
    #[argh(option, default = "0")]
    seed: u64,

    /// number of mutants to generate and grade (default 500), scheduled
    /// round-robin over the seed files
    #[argh(option, default = "500")]
    iterations: usize,

    /// max mutation operators applied per mutant (default 3). The document is
    /// re-anchored between operators, so each one sees the real post-previous
    /// document
    #[argh(option, default = "3")]
    max_mutations: usize,

    /// cap the number of seed files loaded (0 = unlimited)
    #[argh(option, default = "0")]
    limit: usize,

    /// concurrent grading tasks (0 = derive from the machine's parallelism)
    #[argh(option, default = "0")]
    jobs: usize,

    /// stop reporting after this many findings (0 = unbounded; default 50). The
    /// counts stay exact regardless
    #[argh(option, default = "50")]
    max_findings: usize,

    /// write each finding's mutant to this directory for reproduction
    #[argh(option)]
    dump_dir: Option<String>,

    /// list the discovered seed files without fuzzing
    #[argh(switch)]
    list: bool,

    /// emit a machine-readable JSON report
    #[argh(switch)]
    json: bool,

    /// seed corpus paths (default: tests/fixtures_compile)
    #[argh(positional)]
    paths: Vec<String>,
}

/// One graded mutant's verdict.
enum Verdict {
    /// tsv refused (`Unsupported`) — the sidecar was skipped. Not a finding.
    Refused(String),
    /// tsv's Svelte *parser* rejected the mutant. Reported, not gated — a frontend
    /// question, and mutants reach shapes no real component has.
    TsvParseRejected(String),
    /// The oracle rejected the mutant because the mutant's JS does not *parse*
    /// (`js_parse_error`) — the generator emitted invalid syntax, not a component the
    /// refusal contract has anything to say about. Reported, never gated: routing it
    /// to `OverAcceptance` would report a HARNESS regression as a compiler bug, which
    /// is this tool's worst failure mode. Zero today (the three known operator causes
    /// are fixed); the bucket exists so a future operator can only make the count
    /// visible, never make the run lie.
    HarnessInvalidJs(String),
    /// Both compiled and the canonical reprints match.
    Parity { tolerated: bool },
    /// Both compiled and the canonical reprints differ — a compiler bug. HARD.
    Mismatch(String),
    /// The oracle rejected the mutant and tsv compiled it — a refusal-contract
    /// violation. HARD.
    OverAcceptance(String),
    /// tsv's compile panicked. HARD.
    Panic(String),
    /// tsv's own output self-validation fired (`CorruptOutput` / `TypeErasureLeak`)
    /// — always a compiler bug, and one the oracle never had to be consulted for.
    /// HARD.
    SelfCheck(&'static str, String),
    /// A harness failure (sidecar, canonicalizer).
    Error(&'static str, String),
}

impl Verdict {
    /// A finding that fails the run.
    fn is_hard(&self) -> bool {
        matches!(
            self,
            Self::Mismatch(_) | Self::OverAcceptance(_) | Self::Panic(_) | Self::SelfCheck(_, _)
        )
    }

    /// A GENERATOR defect: the mutant's JS does not parse, so the oracle's
    /// rejection says nothing about tsv. Never gated (see [`Self::is_hard`]) — but
    /// it IS reported and dumped, because the tally alone conflates two very
    /// different things: the generator emitting invalid syntax, and tsv *accepting*
    /// invalid syntax (a real frontend over-acceptance). Only the mutant source
    /// separates them, so the source has to reach a human.
    fn is_harness_invalid_js(&self) -> bool {
        matches!(self, Self::HarnessInvalidJs(_))
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Refused(_) => "refused",
            Self::TsvParseRejected(_) => "tsv_parse_rejected",
            Self::HarnessInvalidJs(_) => "harness_invalid_js",
            Self::Parity { .. } => "parity",
            Self::Mismatch(_) => "mismatch",
            Self::OverAcceptance(_) => "over_acceptance",
            Self::Panic(_) => "panic",
            Self::SelfCheck(kind, _) => kind,
            Self::Error(kind, _) => kind,
        }
    }

    /// The verdict's detail line, for the report.
    fn detail(&self) -> &str {
        match self {
            Self::Refused(d)
            | Self::TsvParseRejected(d)
            | Self::HarnessInvalidJs(d)
            | Self::Mismatch(d)
            | Self::OverAcceptance(d)
            | Self::Panic(d)
            | Self::SelfCheck(_, d)
            | Self::Error(_, d) => d,
            Self::Parity { .. } => "",
        }
    }
}

/// A generated mutant, ready to grade.
struct Mutant {
    /// The mutant's index in generation order — the report's sort key.
    index: usize,
    /// The seed file it descends from.
    seed_path: String,
    /// The operators applied, in order.
    ops: Vec<&'static str>,
    source: String,
}

/// A graded mutant.
struct Graded {
    index: usize,
    seed_path: String,
    ops: Vec<&'static str>,
    source: String,
    verdict: Verdict,
}

/// A loaded seed component.
struct Seed {
    display: String,
    source: String,
    /// This file's own mutant stream, keyed by `(master seed, path)`.
    rng: Rng,
}

impl CompileFuzzCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let paths = if self.paths.is_empty() {
            vec!["tests/fixtures_compile".to_string()]
        } else {
            self.paths.clone()
        };
        let files = discover_svelte(&paths)?;
        if self.list {
            for f in &files {
                println!("{}", f.display());
            }
            println!("\n{} seed file(s)", files.len());
            return Ok(());
        }

        let (seeds, donors, unparseable) = self.load_seeds(&files);
        if seeds.is_empty() {
            eprintln!("Error: no parseable seed components found (searched {paths:?})");
            return Err(CliError::Failed);
        }

        let mutants = self.generate(seeds, &donors);

        // Record each panic instead of letting the default hook print it — the
        // fuzzer expects to trigger some, and a wall of backtraces buries the report.
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|info| {
            LAST_PANIC.with(|c| *c.borrow_mut() = Some(info.to_string()));
        }));
        let rt = super::create_runtime();
        let result = rt.block_on(self.grade_and_report(mutants, unparseable));
        std::panic::set_hook(previous);
        result
    }

    /// Read each file, keeping the ones tsv's parser accepts (an unparseable seed is
    /// simply unusable as one — it is counted, not silently dropped).
    fn load_seeds(&self, files: &[PathBuf]) -> (Vec<Seed>, Vec<Donor>, usize) {
        let mut seeds = Vec::new();
        let mut donors = Vec::new();
        let mut unparseable = 0usize;
        for path in files {
            if self.limit > 0 && seeds.len() >= self.limit {
                break;
            }
            let display = path.to_string_lossy().into_owned();
            let Ok(source) = std::fs::read_to_string(path) else {
                unparseable += 1;
                continue;
            };
            // Parseability is the seed gate — an unparseable component yields no
            // anchors, so it can seed nothing. The anchors themselves are re-collected
            // per mutant in `generate`, so they are not carried here.
            if Anchors::collect(&source).is_none() {
                unparseable += 1;
                continue;
            }
            if let Some(donor) = Donor::from_source(&source) {
                donors.push(donor);
            }
            seeds.push(Seed {
                rng: Rng::new(stream_seed(self.seed, &display)),
                display,
                source,
            });
        }
        (seeds, donors, unparseable)
    }

    /// Generate every mutant up front, single-threaded — the determinism seam. Each
    /// seed draws from its own stream, scheduled round-robin.
    fn generate(&self, mut seeds: Vec<Seed>, donors: &[Donor]) -> Vec<Mutant> {
        let mut mutants = Vec::with_capacity(self.iterations);
        for index in 0..self.iterations {
            let slot = index % seeds.len();
            let seed = &mut seeds[slot];
            let mut source = seed.source.clone();
            // Re-collected per mutant rather than borrowed from the seed. One extra
            // parse per mutant, against a pipeline whose dominant cost is the oracle
            // round trip most mutants go on to pay — and it keeps the loop free of
            // aliasing between the seed's anchors and each mutant's.
            let mut anchors = Anchors::collect(&source);
            let mut ops = Vec::new();
            let count = 1 + seed.rng.below(self.max_mutations.max(1));
            for _ in 0..count {
                let op = Operator::ALL[seed.rng.below(Operator::ALL.len())];
                let Some(current) = anchors.as_ref() else {
                    break; // the previous operator left something tsv cannot parse
                };
                // This document carries no anchor this operator needs. The turn is
                // spent (no retry loop — that would bias the mix toward whichever
                // operators this seed happens to support); the next turn draws afresh.
                let Some(next) = operators::apply(op, &source, current, donors, &mut seed.rng)
                else {
                    continue;
                };
                source = next;
                ops.push(op.label());
                // Re-anchor so the NEXT operator sees the real post-this document. A
                // mutant tsv can no longer parse is still worth grading, so stop
                // composing rather than discarding it.
                anchors = Anchors::collect(&source);
            }
            if ops.is_empty() {
                continue; // no operator applied — nothing to grade
            }
            mutants.push(Mutant {
                index,
                seed_path: seed.display.clone(),
                ops,
                source,
            });
        }
        mutants
    }

    /// Grade every mutant over the sidecar pool, then report.
    async fn grade_and_report(
        self,
        mutants: Vec<Mutant>,
        unparseable: usize,
    ) -> Result<(), CliError> {
        let total = mutants.len();
        // Own fan-out rather than `spawn_work_stream`, which derives its concurrency
        // from the machine and so cannot honor `--jobs`. Same shape otherwise: size
        // the sidecar pool BEFORE the first call, then `tokio::spawn` each mutant so
        // the pure-Rust half (compile, canonicalize, diff) spreads across workers
        // while the JS half rides the small pool.
        let concurrency = if self.jobs > 0 {
            deno::set_pool_size(deno::bulk_pool_size(self.jobs));
            self.jobs
        } else {
            deno::init_bulk_pool()
        };
        let mut stream = futures_util::stream::iter(mutants)
            .map(|mutant| tokio::spawn(grade(mutant)))
            .buffer_unordered(concurrency);
        let mut graded = Vec::with_capacity(total);
        while let Some(joined) = stream.next().await {
            graded.push(super::task_result(joined, "compile-fuzz")?);
        }
        // Completion order is nondeterministic; the report must not be.
        graded.sort_by_key(|g| g.index);

        let report = Report::build(&graded, unparseable);
        self.dump(&graded)?;
        if self.json {
            report.print_json(&graded, self.max_findings)?;
        } else {
            report.print_human(&graded, self.max_findings);
        }
        report.verdict()
    }

    /// Write each finding's mutant to `--dump-dir` for reproduction.
    fn dump(&self, graded: &[Graded]) -> Result<(), CliError> {
        let Some(dir) = &self.dump_dir else {
            return Ok(());
        };
        if let Err(e) = std::fs::create_dir_all(dir) {
            eprintln!("Error: cannot create dump dir {dir}: {e}");
            return Err(CliError::Errored);
        }
        // HARD findings plus the ungated `harness_invalid_js` bucket: an invalid-JS
        // mutant is a harness defect, but it is only diagnosable from its source.
        for g in graded
            .iter()
            .filter(|g| g.verdict.is_hard() || g.verdict.is_harness_invalid_js())
        {
            let name = format!("finding_{:05}_{}.svelte", g.index, g.verdict.label());
            let path = PathBuf::from(dir).join(name);
            if let Err(e) = std::fs::write(&path, &g.source) {
                eprintln!("warning: could not write {}: {e}", path.display());
            }
        }
        Ok(())
    }
}

/// The per-mutant pipeline. **tsv first** — a refusal is definitionally outside the
/// target set, so it skips the sidecar entirely (the run's one throughput lever).
async fn grade(mutant: Mutant) -> Graded {
    let verdict = grade_source(&mutant.source).await;
    Graded {
        index: mutant.index,
        seed_path: mutant.seed_path,
        ops: mutant.ops,
        source: mutant.source,
        verdict,
    }
}

async fn grade_source(source: &str) -> Verdict {
    // tsv's compile is the pre-filter. `catch_unwind` because a panic here is a
    // finding, not a dead worker (the corpus profile keeps unwinding on).
    let Ok(ours) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        compile(source, &CompileOptions::default())
    })) else {
        return Verdict::Panic(
            LAST_PANIC
                .with(|c| c.borrow_mut().take())
                .unwrap_or_default(),
        );
    };
    let ours = match ours {
        Ok(output) => output,
        // The honest refusal contract — and the pre-filter's whole point.
        Err(CompileError::Unsupported(reason)) => {
            return Verdict::Refused(reason.bucket_key().into_owned());
        }
        Err(CompileError::Parse(e)) => return Verdict::TsvParseRejected(e.to_string()),
        // Both self-checks are unconditional compiler bugs, provable without the
        // oracle: emitted JS that does not reparse, or a TypeScript-only node that
        // survived erasure.
        Err(CompileError::CorruptOutput(e)) => {
            return Verdict::SelfCheck("corrupt_output", e.to_string());
        }
        Err(CompileError::TypeErasureLeak(span)) => {
            return Verdict::SelfCheck("type_erasure_leak", format!("at {span:?}"));
        }
        Err(CompileError::GeneratedNameMissing(span)) => {
            return Verdict::SelfCheck("generated_name_missing", format!("at {span:?}"));
        }
    };

    // tsv compiled — only now is the oracle worth a round trip.
    let oracle = match deno::svelte_compile(source, SvelteGenerate::Server, false).await {
        Ok(o) => o,
        Err(DenoError::ToolError { message }) => {
            // A `svelte.dev/e/{code}` URL is the compiler REJECTING the source; a
            // ToolError without one is a sidecar-internal failure and must not be
            // reported as an over-acceptance (that would fail the run on a harness
            // hiccup).
            return match oracle_reject_code(&message) {
                // `js_parse_error` says the mutant is not valid JavaScript, so it is a
                // GENERATOR defect rather than a component tsv over-accepted.
                Some(code) if code == "js_parse_error" => {
                    Verdict::HarnessInvalidJs(first_line(&message))
                }
                Some(code) => Verdict::OverAcceptance(code),
                None => Verdict::Error("oracle_tool", first_line(&message)),
            };
        }
        Err(e) => return Verdict::Error("oracle_sidecar", e.to_string()),
    };

    let oracle_canon = match canonicalize_js(&oracle.js) {
        Ok(c) => c,
        Err(e) => return Verdict::Error("canonicalize_oracle", e.to_string()),
    };
    let ours_canon = match canonicalize_js(&ours.js) {
        Ok(c) => c,
        Err(e) => return Verdict::Error("canonicalize_ours", e.to_string()),
    };
    match compare_canonical(&ours_canon, &oracle_canon) {
        p if p.is_parity() => Verdict::Parity {
            tolerated: p == Parity::CommentPosition,
        },
        _ => Verdict::Mismatch(bounded_diff(&ours_canon, &oracle_canon)),
    }
}

/// A bounded canonical diff for the report (a full one can be thousands of lines).
fn bounded_diff(ours: &str, oracle: &str) -> String {
    let options = DiffOptions {
        color: false,
        color_choice: ColorChoice::Never,
        ..DiffOptions::compile_compare()
    };
    let diff = diff_to_string(ours, oracle, &options);
    let mut out: String = diff
        .lines()
        .take(DIFF_LINE_CAP)
        .collect::<Vec<_>>()
        .join("\n");
    if diff.lines().count() > DIFF_LINE_CAP {
        out.push_str("\n      … (diff truncated)");
    }
    out
}

/// Lines of a mismatch diff carried into the report.
const DIFF_LINE_CAP: usize = 20;

/// The Svelte error code in an oracle rejection message (`svelte.dev/e/{code}`), or
/// `None` when the message carries none — which means a sidecar failure, not a
/// rejection.
fn oracle_reject_code(message: &str) -> Option<String> {
    let at = message.find("svelte.dev/e/")? + "svelte.dev/e/".len();
    let rest = &message[at..];
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    (end > 0).then(|| rest[..end].to_string())
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or_default().to_string()
}

/// Discover `.svelte` files under the given paths, sorted (the round-robin schedule
/// must not depend on filesystem order).
fn discover_svelte(paths: &[String]) -> Result<Vec<PathBuf>, CliError> {
    let mut out = Vec::new();
    for path in paths {
        let path = PathBuf::from(path);
        if path.is_file() {
            out.push(path);
            continue;
        }
        if !path.is_dir() {
            eprintln!("Error: no such path: {}", path.display());
            return Err(CliError::Failed);
        }
        collect_svelte(&path, &mut out);
    }
    out.sort();
    if out.is_empty() {
        eprintln!("Error: no .svelte files found in {paths:?}");
        return Err(CliError::Failed);
    }
    Ok(out)
}

fn collect_svelte(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_svelte(&path, out);
        } else if path.extension().is_some_and(|e| e == "svelte") {
            out.push(path);
        }
    }
}

/// The run's tallies.
struct Report {
    total: usize,
    unparseable_seeds: usize,
    refused: usize,
    refusal_reasons: BTreeMap<String, usize>,
    tsv_parse_rejected: usize,
    harness_invalid_js: usize,
    parity: usize,
    parity_tolerated: usize,
    mismatch: usize,
    over_acceptance: usize,
    over_acceptance_codes: BTreeMap<String, usize>,
    panic: usize,
    self_check: usize,
    error: usize,
    /// Operators that actually reached a graded mutant, by label.
    ops: BTreeMap<&'static str, usize>,
}

impl Report {
    fn build(graded: &[Graded], unparseable_seeds: usize) -> Self {
        let mut r = Self {
            total: graded.len(),
            unparseable_seeds,
            refused: 0,
            refusal_reasons: BTreeMap::new(),
            tsv_parse_rejected: 0,
            harness_invalid_js: 0,
            parity: 0,
            parity_tolerated: 0,
            mismatch: 0,
            over_acceptance: 0,
            over_acceptance_codes: BTreeMap::new(),
            panic: 0,
            self_check: 0,
            error: 0,
            ops: BTreeMap::new(),
        };
        for g in graded {
            for op in &g.ops {
                *r.ops.entry(op).or_default() += 1;
            }
            match &g.verdict {
                Verdict::Refused(reason) => {
                    r.refused += 1;
                    *r.refusal_reasons.entry(reason.clone()).or_default() += 1;
                }
                Verdict::TsvParseRejected(_) => r.tsv_parse_rejected += 1,
                Verdict::HarnessInvalidJs(_) => r.harness_invalid_js += 1,
                Verdict::Parity { tolerated } => {
                    r.parity += 1;
                    r.parity_tolerated += usize::from(*tolerated);
                }
                Verdict::Mismatch(_) => r.mismatch += 1,
                Verdict::OverAcceptance(code) => {
                    r.over_acceptance += 1;
                    *r.over_acceptance_codes.entry(code.clone()).or_default() += 1;
                }
                Verdict::Panic(_) => r.panic += 1,
                Verdict::SelfCheck(_, _) => r.self_check += 1,
                Verdict::Error(_, _) => r.error += 1,
            }
        }
        r
    }

    /// Mutants that survived the tsv-first pre-filter and cost an oracle round trip.
    /// The number the throughput model rests on — reported because it was unmeasured.
    fn oracle_calls(&self) -> usize {
        self.parity + self.mismatch + self.over_acceptance + self.harness_invalid_js + self.error
    }

    fn hard(&self) -> usize {
        self.mismatch + self.over_acceptance + self.panic + self.self_check
    }

    /// A HARD finding fails the run; a harness error means some mutant got no verdict.
    fn verdict(&self) -> Result<(), CliError> {
        if self.hard() > 0 {
            Err(CliError::Failed)
        } else if self.error > 0 {
            Err(CliError::Errored)
        } else {
            Ok(())
        }
    }

    // Counts are mutant tallies — nowhere near 2^53, so the cast is exact here.
    #[allow(clippy::cast_precision_loss)]
    fn pass_through_pct(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            100.0 * self.oracle_calls() as f64 / self.total as f64
        }
    }

    fn print_human(&self, graded: &[Graded], max_findings: usize) {
        println!("compile_fuzz — {} mutants graded\n", self.total);
        if self.unparseable_seeds > 0 {
            println!(
                "  {:>7}  seed file(s) skipped (unreadable or tsv-unparseable)",
                self.unparseable_seeds
            );
        }
        println!(
            "  {:>7}  refused by tsv (sidecar skipped — not a finding)",
            self.refused
        );
        println!(
            "  {:>7}  rejected by tsv's PARSER (reported, not gated)",
            self.tsv_parse_rejected
        );
        println!(
            "  {:>7}  invalid JS from the GENERATOR (a harness defect, not a bug — not gated)",
            self.harness_invalid_js
        );
        println!(
            "  {:>7}  parity ({} via tolerated comment position)",
            self.parity, self.parity_tolerated
        );
        println!(
            "  {:>7}  MISMATCH (both compiled, code differs — a bug)",
            self.mismatch
        );
        println!(
            "  {:>7}  OVER-ACCEPTANCE (oracle rejected, tsv compiled — a bug)",
            self.over_acceptance
        );
        println!("  {:>7}  panic in tsv's compile (a bug)", self.panic);
        println!(
            "  {:>7}  self-check fired (corrupt output / erasure leak — a bug)",
            self.self_check
        );
        println!("  {:>7}  harness error\n", self.error);

        println!(
            "  PRE-FILTER: {} of {} mutants ({:.1}%) survived tsv's compile and cost an oracle",
            self.oracle_calls(),
            self.total,
            self.pass_through_pct()
        );
        println!(
            "              round trip; the other {} were refused or parse-rejected.\n",
            self.total - self.oracle_calls()
        );

        if !self.ops.is_empty() {
            println!("  operators applied:");
            for (op, n) in &self.ops {
                println!("    {n:>6}  {op}");
            }
            println!();
        }

        if !self.refusal_reasons.is_empty() {
            let mut reasons: Vec<_> = self.refusal_reasons.iter().collect();
            reasons.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
            println!("  refusal reasons (top 15):");
            for (reason, n) in reasons.iter().take(15) {
                println!("    {n:>6}  {reason}");
            }
            println!();
        }

        if !self.over_acceptance_codes.is_empty() {
            println!("  over-acceptance oracle error codes:");
            for (code, n) in &self.over_acceptance_codes {
                println!("    {n:>6}  {code}");
            }
            println!();
        }

        let invalid_js: Vec<&Graded> = graded
            .iter()
            .filter(|g| g.verdict.is_harness_invalid_js())
            .collect();
        if !invalid_js.is_empty() {
            let shown = if max_findings == 0 {
                invalid_js.len()
            } else {
                max_findings.min(invalid_js.len())
            };
            println!(
                "○ GENERATOR defects — mutants whose JS does not parse (NOT gated; fix the operator):\n"
            );
            for g in &invalid_js[..shown] {
                println!(
                    "  [harness_invalid_js] mutant {} · seed {} · ops {}",
                    g.index,
                    g.seed_path,
                    g.ops.join(" → ")
                );
                for line in g.source.lines() {
                    println!("      | {line}");
                }
                println!();
            }
            if shown < invalid_js.len() {
                println!(
                    "  … and {} more (raise --max-findings)\n",
                    invalid_js.len() - shown
                );
            }
        }

        let findings: Vec<&Graded> = graded.iter().filter(|g| g.verdict.is_hard()).collect();
        if !findings.is_empty() {
            println!("✗ HARD findings (each is a bug by the refusal contract):\n");
            let shown = if max_findings == 0 {
                findings.len()
            } else {
                max_findings.min(findings.len())
            };
            for g in &findings[..shown] {
                println!(
                    "  [{}] mutant {} · seed {} · ops {}",
                    g.verdict.label(),
                    g.index,
                    g.seed_path,
                    g.ops.join(" → ")
                );
                let detail = g.verdict.detail();
                if !detail.is_empty() {
                    for line in detail.lines() {
                        println!("      {line}");
                    }
                }
                println!();
            }
            if shown < findings.len() {
                println!(
                    "  … and {} more (raise --max-findings)\n",
                    findings.len() - shown
                );
            }
            println!("  (pass --dump-dir DIR to write each finding's mutant for reproduction)");
        } else if self.error > 0 {
            println!(
                "○ no findings, but {} harness error(s) left mutants ungraded.",
                self.error
            );
        } else {
            println!("✓ no findings — every graded mutant was refused or matched the oracle");
        }
    }

    fn print_json(&self, graded: &[Graded], max_findings: usize) -> Result<(), CliError> {
        let findings: Vec<&Graded> = graded.iter().filter(|g| g.verdict.is_hard()).collect();
        let shown = if max_findings == 0 {
            findings.len()
        } else {
            max_findings.min(findings.len())
        };
        let findings_json: Vec<serde_json::Value> = findings[..shown]
            .iter()
            .map(|g| {
                serde_json::json!({
                    "index": g.index,
                    "kind": g.verdict.label(),
                    "seed": g.seed_path,
                    "ops": g.ops,
                    "detail": g.verdict.detail(),
                    "source": g.source,
                })
            })
            .collect();
        // The ungated generator-defect bucket, carried with its source for the same
        // reason the human report shows it: only the source tells a harness defect
        // apart from tsv accepting invalid syntax.
        let invalid_js: Vec<&Graded> = graded
            .iter()
            .filter(|g| g.verdict.is_harness_invalid_js())
            .collect();
        let invalid_js_shown = if max_findings == 0 {
            invalid_js.len()
        } else {
            max_findings.min(invalid_js.len())
        };
        let invalid_js_json: Vec<serde_json::Value> = invalid_js[..invalid_js_shown]
            .iter()
            .map(|g| {
                serde_json::json!({
                    "index": g.index,
                    "seed": g.seed_path,
                    "ops": g.ops,
                    "detail": g.verdict.detail(),
                    "source": g.source,
                })
            })
            .collect();
        let out = serde_json::json!({
            "total": self.total,
            "unparseable_seeds": self.unparseable_seeds,
            "refused": self.refused,
            "refusal_reasons": self.refusal_reasons,
            "tsv_parse_rejected": self.tsv_parse_rejected,
            "harness_invalid_js": self.harness_invalid_js,
            "parity": self.parity,
            "parity_comment_position_tolerated": self.parity_tolerated,
            "mismatch": self.mismatch,
            "over_acceptance": self.over_acceptance,
            "over_acceptance_codes": self.over_acceptance_codes,
            "panic": self.panic,
            "self_check": self.self_check,
            "error": self.error,
            "oracle_calls": self.oracle_calls(),
            "pass_through_pct": self.pass_through_pct(),
            "operators": self.ops,
            "findings": findings_json,
            "harness_invalid_js_findings": invalid_js_json,
        });
        match to_json_with_tabs(&out) {
            Ok(json) => {
                println!("{json}");
                Ok(())
            }
            Err(e) => {
                eprintln!("Error serializing report: {e}");
                Err(CliError::Errored)
            }
        }
    }
}

thread_local! {
    /// The most recent panic's `Display` string, captured by the panic hook installed
    /// in [`CompileFuzzCommand::run`]. Per-thread, and `catch_unwind` returns on the
    /// thread that panicked, so a concurrent grader reads its own panic.
    static LAST_PANIC: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_reject_code_extracts_the_svelte_code() {
        assert_eq!(
            oracle_reject_code("Error: bad (https://svelte.dev/e/legacy_reactive_statement)")
                .as_deref(),
            Some("legacy_reactive_statement")
        );
        // A sidecar failure carries no code — it must NOT read as a rejection, or a
        // harness hiccup would fail the run as an over-acceptance.
        assert_eq!(oracle_reject_code("actor shutdown"), None);
    }

    fn graded(verdict: Verdict) -> Graded {
        Graded {
            index: 0,
            seed_path: "seed.svelte".to_string(),
            ops: vec!["shadow_read"],
            source: String::new(),
            verdict,
        }
    }

    #[test]
    fn verdict_gates_on_mismatch_and_over_acceptance() {
        for verdict in [
            Verdict::Mismatch(String::new()),
            Verdict::OverAcceptance("x".to_string()),
            Verdict::Panic(String::new()),
            Verdict::SelfCheck("corrupt_output", String::new()),
        ] {
            let label = verdict.label();
            let report = Report::build(&[graded(verdict)], 0);
            assert!(report.verdict().is_err(), "{label} must fail the run");
        }
    }

    #[test]
    fn refusals_and_parse_rejections_do_not_gate() {
        let report = Report::build(
            &[
                graded(Verdict::Refused("x".to_string())),
                graded(Verdict::TsvParseRejected("y".to_string())),
                graded(Verdict::Parity { tolerated: false }),
            ],
            0,
        );
        assert!(report.verdict().is_ok());
        // The pre-filter measurement: only the parity mutant cost an oracle call.
        assert_eq!(report.oracle_calls(), 1);
    }

    /// A mutant whose JS does not parse is a GENERATOR defect — reporting it as an
    /// over-acceptance would blame the compiler for a harness regression.
    #[test]
    fn invalid_generated_js_is_a_harness_bucket_not_an_over_acceptance() {
        let report = Report::build(&[graded(Verdict::HarnessInvalidJs("boom".to_string()))], 0);
        assert!(report.verdict().is_ok());
        assert_eq!(report.harness_invalid_js, 1);
        assert_eq!(report.over_acceptance, 0);
        // It DID cost an oracle round trip, so the pre-filter arithmetic must count it.
        assert_eq!(report.oracle_calls(), 1);
    }

    /// …but it must still be REPORTED. A silent tally cannot tell a generator defect
    /// apart from tsv accepting invalid syntax (a real frontend over-acceptance) —
    /// only the mutant source can, so it is dumped and printed like a finding while
    /// staying outside [`Verdict::is_hard`] (and so outside the gate above).
    #[test]
    fn invalid_generated_js_is_reported_but_never_gated() {
        let verdict = Verdict::HarnessInvalidJs("boom".to_string());
        assert!(verdict.is_harness_invalid_js());
        assert!(!verdict.is_hard(), "reporting it must not start gating it");

        let dir = std::env::temp_dir().join(format!(
            "tsv_compile_fuzz_dump_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let mut cmd = command(0, 0);
        cmd.dump_dir = Some(dir.to_string_lossy().into_owned());
        let mut g = graded(verdict);
        g.source = "<p>x</p>".to_string();
        cmd.dump(&[g]).expect("dump");

        let dumped = dir.join("finding_00000_harness_invalid_js.svelte");
        assert_eq!(
            std::fs::read_to_string(&dumped).expect("dumped file"),
            "<p>x</p>"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A command with everything defaulted except what a test sets.
    fn command(seed: u64, iterations: usize) -> CompileFuzzCommand {
        CompileFuzzCommand {
            seed,
            iterations,
            max_mutations: 3,
            limit: 0,
            jobs: 0,
            max_findings: 0,
            dump_dir: None,
            list: false,
            json: false,
            paths: Vec::new(),
        }
    }

    fn seed_of(path: &str, source: &str, master: u64) -> Seed {
        Seed {
            display: path.to_string(),
            source: source.to_string(),
            rng: Rng::new(stream_seed(master, path)),
        }
    }

    /// Adding a fixture must not rewrite any OTHER fixture's mutants — the property
    /// that keeps a corpus edit from silently reshuffling the run onto an unrelated
    /// latent bug. Each file's sequence is keyed by its own path, so a corpus edit may
    /// only trim or extend that file's tail.
    ///
    /// The donor pool is held fixed here on purpose: `SpliceDonor` reads the whole
    /// corpus and is documented as outside the guarantee (see the module docs), so
    /// including it would test the exception rather than the rule.
    #[test]
    fn generate_is_corpus_add_stable() {
        const A: &str = "<script>let a = 1;</script><p>{a}</p>";
        const B: &str = "{#each [0] as x}<i>{x}</i>{/each}";
        const C: &str = "<div><span>c</span></div>";
        let donors = Vec::new();

        let mutants_by_path = |seeds: Vec<Seed>| {
            let mut by_path: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for mutant in command(0, 60).generate(seeds, &donors) {
                by_path
                    .entry(mutant.seed_path)
                    .or_default()
                    .push(mutant.source);
            }
            by_path
        };

        let before = mutants_by_path(vec![seed_of("a.svelte", A, 0), seed_of("b.svelte", B, 0)]);
        let after = mutants_by_path(vec![
            seed_of("a.svelte", A, 0),
            seed_of("b.svelte", B, 0),
            seed_of("c.svelte", C, 0),
        ]);

        assert!(
            after.contains_key("c.svelte"),
            "the added file produced mutants"
        );
        for path in ["a.svelte", "b.svelte"] {
            let (old, new) = (&before[path], &after[path]);
            let shared = old.len().min(new.len());
            assert!(shared > 0, "{path} produced mutants in both runs");
            assert_eq!(
                old[..shared],
                new[..shared],
                "{path}'s mutant sequence was rewritten by an unrelated corpus add"
            );
        }
    }

    #[test]
    fn a_harness_error_is_errored_not_failed() {
        let report = Report::build(
            &[graded(Verdict::Error("oracle_sidecar", String::new()))],
            0,
        );
        assert!(matches!(report.verdict(), Err(CliError::Errored)));
    }
}
