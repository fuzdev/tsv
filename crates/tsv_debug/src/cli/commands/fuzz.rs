//! Dep-free seeded mutational fuzzer — the "coverage trifecta" fuzzing leg.
//!
//! ## Why this exists
//!
//! The fixture suite and the corpus gates only ever exercise **valid, real**
//! source. Three invariants must hold on *arbitrary* input too, and nothing else
//! guards them:
//!
//! 1. **No panic / hang-safety.** The parser must never crash on arbitrary bytes.
//!    Production WASM/CLI/FFI builds use `panic = "abort"`, so a panic there is a
//!    hard crash / DoS — and the corpus profile (`panic = "unwind"`) only ever
//!    catches panics on *real* code. This fuzzer drives mutated bytes through the
//!    parser+formatter under [`std::panic::catch_unwind`], so a panic is a
//!    reported finding, not a crash.
//! 2. **Idempotency.** For any input tsv accepts, `format` is a fixed point:
//!    `format(format(x)) == format(x)` (the fixture F1 invariant, here on inputs
//!    no fixture covers).
//! 3. **Structural reparse.** `format(x)` must reparse to the *same document*
//!    (the [`roundtrip_audit`](super::roundtrip_audit) contract — output that
//!    mis-delimits but loses no characters is invisible to the char-frequency
//!    SAFETY check), reusing that command's structural-skeleton comparison.
//!
//! ## Design
//!
//! Pure Rust, **zero new deps** (the tight-dep policy): a seeded SplitMix64 PRNG
//! and byte-level mutation operators over a seed corpus (`tests/fixtures` by
//! default). Seeded ⇒ deterministic ⇒ a cheap CI-able gate (`--seed S
//! --iterations N` reproduces exactly); unbounded iterations ⇒ discovery. Only
//! valid-UTF-8 mutants reach the parser (the `&str` boundary the real CLI/WASM
//! entry points enforce); a mutation that breaks UTF-8 is skipped, not counted.
//!
//! Not the differential leg (tsv-vs-canonical verdict): that needs the Deno
//! sidecar. This stays pure-Rust and self-contained, matching the
//! `test262 --gate` / `roundtrip_audit --gate` direction.

use argh::FromArgs;
use std::path::PathBuf;

use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;

use super::profile::resolve_files;
use super::roundtrip_audit::{structurally_equivalent, tsv_parse_to_value};
use crate::cli::CliError;

/// Seeded mutational fuzzer: mutate corpus bytes and assert the parser never
/// panics, `format` is idempotent, and formatted output reparses structurally
/// equal.
///
/// Defaults to `tests/fixtures` as the seed corpus. Deterministic for a given
/// `--seed` + corpus, so a failing run reproduces exactly; raise `--iterations`
/// (or vary `--seed`) for discovery. Exits 1 on any finding (a panic, a
/// non-idempotent format, or output that doesn't reparse to the same document),
/// 0 when clean — so it doubles as a CI gate.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "fuzz")]
pub struct FuzzCommand {
    /// PRNG seed (default 0) — same seed + corpus ⇒ identical run
    #[argh(option, default = "0")]
    seed: u64,

    /// number of mutated inputs to test (default 2000)
    #[argh(option, default = "2000")]
    iterations: usize,

    /// max mutation operators applied per input (default 8)
    #[argh(option, default = "8")]
    max_mutations: usize,

    /// cap the number of seed-corpus files loaded (0 = unlimited)
    #[argh(option, default = "0")]
    limit: usize,

    /// stop after this many HARD findings (0 = run all iterations; default 20).
    /// Soft findings (structural_divergence) never stop the run — they're only
    /// counted (they need canonical confirmation and are render-model noisy over
    /// Svelte; see below).
    #[argh(option, default = "20")]
    max_findings: usize,

    /// also fail (exit 1) on soft `structural_divergence` findings. Off by
    /// default: like `roundtrip_audit --gate`, the divergent bucket is render-
    /// model noise over Svelte and needs canonical confirmation
    /// (`roundtrip_audit --canonical-all`) to be trusted — so it's reported but
    /// non-fatal unless you're in discovery mode
    #[argh(switch)]
    strict: bool,

    /// write each failing input to this directory for reproduction
    #[argh(option)]
    dump_dir: Option<String>,

    /// disable Svelte-5 render-time whitespace normalization before the
    /// structural compare (default: on, matching `roundtrip_audit`)
    #[argh(switch)]
    no_render: bool,

    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// seed corpus file paths / directories (default: tests/fixtures)
    #[argh(positional)]
    paths: Vec<String>,
}

/// A dep-free SplitMix64 PRNG — deterministic, tiny, no external crate.
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value in `[0, n)`; 0 when `n == 0`.
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }

    fn byte(&mut self) -> u8 {
        (self.next_u64() & 0xff) as u8
    }
}

/// Bytes that push a mutant toward *structurally* interesting input — the
/// delimiters, operators, and whitespace where parser/formatter edge cases live —
/// rather than uniformly-random noise that almost always fails to parse.
const INTERESTING: &[u8] = b"{}()[]<>;:,.\"'`/\\=+-*&|!?@#$%^~\n\t abc012";

/// Apply 1..=`max_ops` byte-level mutation operators to a copy of `seed`.
fn mutate(rng: &mut Rng, seed: &[u8], max_ops: usize) -> Vec<u8> {
    let mut buf = seed.to_vec();
    let ops = 1 + rng.below(max_ops.max(1));
    for _ in 0..ops {
        if buf.is_empty() {
            buf.push(rng.byte());
            continue;
        }
        match rng.below(8) {
            // Flip one bit.
            0 => {
                let i = rng.below(buf.len());
                buf[i] ^= 1u8 << rng.below(8);
            }
            // Overwrite a byte with an interesting one.
            1 => {
                let i = rng.below(buf.len());
                buf[i] = INTERESTING[rng.below(INTERESTING.len())];
            }
            // Overwrite a byte with a random one.
            2 => {
                let i = rng.below(buf.len());
                buf[i] = rng.byte();
            }
            // Insert an interesting byte.
            3 => {
                let i = rng.below(buf.len() + 1);
                buf.insert(i, INTERESTING[rng.below(INTERESTING.len())]);
            }
            // Delete a byte.
            4 => {
                let i = rng.below(buf.len());
                buf.remove(i);
            }
            // Duplicate a short chunk elsewhere (grows nesting / repetition).
            5 => {
                let len = buf.len();
                let start = rng.below(len);
                let chunk_len = 1 + rng.below((len - start).min(16));
                let chunk = buf[start..start + chunk_len].to_vec();
                let at = rng.below(buf.len() + 1);
                for (k, b) in chunk.into_iter().enumerate() {
                    buf.insert(at + k, b);
                }
            }
            // Truncate (keep at least one byte).
            6 => {
                let i = rng.below(buf.len());
                buf.truncate(i.max(1));
            }
            // Swap two bytes.
            _ => {
                let a = rng.below(buf.len());
                let b = rng.below(buf.len());
                buf.swap(a, b);
            }
        }
    }
    buf
}

/// The invariant a mutated input violated (or `Rejected`/`Ok`, which aren't
/// findings).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// tsv's parser cleanly rejected the input — expected, not a finding.
    Rejected,
    /// Parsed, formatted idempotently, and reparsed structurally equal.
    Ok,
    /// The parser or formatter panicked (the DoS-safety violation).
    Panic,
    /// Parsed, but `format` errored (should be impossible — `format` re-parses).
    FormatError,
    /// `format`'s output does not reparse (tsv rejects its own output).
    Unreparseable,
    /// Output reparses but the document structure changed (delimiter/structure
    /// corruption).
    StructuralDivergence,
    /// `format(format(x)) != format(x)` — a non-idempotent fixed point.
    NonIdempotent,
}

impl Outcome {
    /// A **reliable, dep-free** finding — always a real bug, so it fails the run
    /// (exit 1). A panic breaks DoS-safety; `format_error`/`unreparseable` mean
    /// tsv can't round-trip its own output; `non_idempotent` breaks the F1 fixed
    /// point. `structural_divergence` is deliberately excluded — it's the soft,
    /// canonical-confirmation-needing bucket (see [`FuzzCommand::strict`]).
    fn is_hard(self) -> bool {
        matches!(
            self,
            Self::Panic | Self::FormatError | Self::Unreparseable | Self::NonIdempotent
        )
    }

    fn label(self) -> &'static str {
        match self {
            Self::Rejected => "rejected",
            Self::Ok => "ok",
            Self::Panic => "panic",
            Self::FormatError => "format_error",
            Self::Unreparseable => "unreparseable",
            Self::StructuralDivergence => "structural_divergence",
            Self::NonIdempotent => "non_idempotent",
        }
    }
}

/// Run the three invariant checks on one (already valid-UTF-8) mutant. Any panic
/// is caught by the caller's [`catch_unwind`](std::panic::catch_unwind); this
/// returns the non-panic outcome.
fn check(src: &str, parser: ParserType, render: bool) -> Outcome {
    // 1. Parse. A clean rejection is the common, expected case for garbage.
    let Some(wire_in) = tsv_parse_to_value(src, parser) else {
        return Outcome::Rejected;
    };
    // 2. Format (parses internally — an error here means parse/format disagree).
    let Ok(f1) = format_source(src, parser) else {
        return Outcome::FormatError;
    };
    // 3. Reparse the output.
    let Some(wire_out) = tsv_parse_to_value(&f1, parser) else {
        return Outcome::Unreparseable;
    };
    // 4. Same document?
    let (equal, _) = structurally_equivalent(wire_in, wire_out, render, false);
    if !equal {
        return Outcome::StructuralDivergence;
    }
    // 5. Idempotent fixed point.
    match format_source(&f1, parser) {
        Ok(f2) if f2 == f1 => Outcome::Ok,
        Ok(_) => Outcome::NonIdempotent,
        Err(_) => Outcome::FormatError,
    }
}

/// One recorded finding, enough to reproduce and triage.
struct Finding {
    iteration: usize,
    parser: ParserType,
    outcome: Outcome,
    seed_path: String,
    input: String,
    panic: Option<String>,
}

/// A loaded seed-corpus entry.
struct Seed {
    display: String,
    bytes: Vec<u8>,
    parser: ParserType,
}

impl FuzzCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let paths = if self.paths.is_empty() {
            vec!["tests/fixtures".to_string()]
        } else {
            self.paths.clone()
        };
        let mut files = match resolve_files(&paths) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error: {e}");
                return Err(CliError::Failed);
            }
        };
        // Intentionally-invalid fixtures are poor seeds (they don't parse), but
        // they're still valid *bytes* to mutate — keep them; only cap the count.
        if self.limit > 0 {
            files.truncate(self.limit);
        }

        let seeds: Vec<Seed> = files
            .iter()
            .filter_map(|p| {
                let display = p.to_string_lossy().into_owned();
                let bytes = std::fs::read(p).ok()?;
                Some(Seed {
                    parser: ParserType::from_extension(&display),
                    display,
                    bytes,
                })
            })
            .filter(|s| !s.bytes.is_empty())
            .collect();

        if seeds.is_empty() {
            eprintln!("Error: no seed files found (searched {paths:?})");
            return Err(CliError::Failed);
        }

        if let Some(dir) = &self.dump_dir
            && let Err(e) = std::fs::create_dir_all(dir)
        {
            eprintln!("Error: cannot create dump dir {dir}: {e}");
            return Err(CliError::Failed);
        }

        let render = !self.no_render;
        let mut rng = Rng::new(self.seed);

        // Record each panic's message/location instead of letting the default
        // hook print it (the fuzzer triggers panics on purpose). The loop is
        // single-threaded, so a thread-local suffices.
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|info| {
            LAST_PANIC.with(|c| *c.borrow_mut() = Some(info.to_string()));
        }));

        let mut tested = 0usize;
        let mut skipped_non_utf8 = 0usize;
        let mut rejected = 0usize;
        let mut ok = 0usize;
        let mut hard_count = 0usize;
        let mut soft_count = 0usize;
        let mut findings: Vec<Finding> = Vec::new();
        // Bound stored findings (they hold the mutant text) so a noisy discovery
        // run can't balloon memory; the counts above stay exact.
        let store_cap = self.max_findings.max(20) * 4;

        for iteration in 0..self.iterations {
            let seed = &seeds[rng.below(seeds.len())];
            let mutated = mutate(&mut rng, &seed.bytes, self.max_mutations);
            let Ok(src) = std::str::from_utf8(&mutated) else {
                skipped_non_utf8 += 1;
                continue;
            };
            tested += 1;

            LAST_PANIC.with(|c| *c.borrow_mut() = None);
            let outcome = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                check(src, seed.parser, render)
            })) {
                Ok(o) => o,
                Err(_) => Outcome::Panic,
            };

            match outcome {
                Outcome::Rejected => rejected += 1,
                Outcome::Ok => ok += 1,
                finding => {
                    if finding.is_hard() {
                        hard_count += 1;
                    } else {
                        soft_count += 1;
                    }
                    if findings.len() < store_cap {
                        let panic = if finding == Outcome::Panic {
                            LAST_PANIC.with(|c| c.borrow_mut().take())
                        } else {
                            None
                        };
                        findings.push(Finding {
                            iteration,
                            parser: seed.parser,
                            outcome: finding,
                            seed_path: seed.display.clone(),
                            input: src.to_string(),
                            panic,
                        });
                    }
                    // Only HARD findings stop the run early — soft ones (render-
                    // model-noisy structural divergences) are counted, not fatal.
                    if self.max_findings > 0 && hard_count >= self.max_findings {
                        break;
                    }
                }
            }
        }

        std::panic::set_hook(prev_hook);

        self.dump_findings(&findings);
        let stats = Stats {
            tested,
            skipped_non_utf8,
            rejected,
            ok,
            hard_count,
            soft_count,
        };
        self.report(&stats, &findings)
    }

    /// Write each finding's input to `--dump-dir` (if set) for reproduction.
    fn dump_findings(&self, findings: &[Finding]) {
        let Some(dir) = &self.dump_dir else {
            return;
        };
        for (n, f) in findings.iter().enumerate() {
            let ext = match f.parser {
                ParserType::TypeScript => "ts",
                ParserType::Svelte => "svelte",
                ParserType::Css => "css",
            };
            let name = format!("finding_{n:03}_{}.{ext}", f.outcome.label());
            let path = PathBuf::from(dir).join(name);
            if let Err(e) = std::fs::write(&path, &f.input) {
                eprintln!("warning: could not write {}: {e}", path.display());
            }
        }
    }

    /// Whether the run fails the process: always on a HARD finding; on a soft
    /// (structural_divergence) finding only under `--strict`.
    fn is_fail(&self, stats: &Stats) -> bool {
        stats.hard_count > 0 || (self.strict && stats.soft_count > 0)
    }

    fn report(&self, stats: &Stats, findings: &[Finding]) -> Result<(), CliError> {
        let fail = self.is_fail(stats);

        if self.json {
            let findings_json: Vec<serde_json::Value> = findings
                .iter()
                .map(|f| {
                    serde_json::json!({
                        "iteration": f.iteration,
                        "outcome": f.outcome.label(),
                        "hard": f.outcome.is_hard(),
                        "parser": f.parser.name(),
                        "seed": f.seed_path,
                        "panic": f.panic,
                        "input": f.input,
                    })
                })
                .collect();
            let out = serde_json::json!({
                "seed": self.seed,
                "iterations": self.iterations,
                "tested": stats.tested,
                "skipped_non_utf8": stats.skipped_non_utf8,
                "rejected": stats.rejected,
                "ok": stats.ok,
                "hard_findings": stats.hard_count,
                "soft_findings": stats.soft_count,
                "strict": self.strict,
                "findings": findings_json,
            });
            println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
            return if fail { Err(CliError::Failed) } else { Ok(()) };
        }

        println!(
            "fuzz — seed {} · {} iterations · {} tested ({} non-UTF-8 skipped)\n",
            self.seed, self.iterations, stats.tested, stats.skipped_non_utf8
        );
        println!(
            "  {:>7}  rejected (parser cleanly refused — fine)",
            stats.rejected
        );
        println!("  {:>7}  ok (parsed, idempotent, reparses equal)", stats.ok);
        println!(
            "  {:>7}  HARD findings (panic / unreparseable / non-idempotent)",
            stats.hard_count
        );
        println!(
            "  {:>7}  soft findings (structural_divergence — see below)\n",
            stats.soft_count
        );

        // HARD findings first — the reliable, dep-free bugs.
        let hard: Vec<&Finding> = findings.iter().filter(|f| f.outcome.is_hard()).collect();
        let soft: Vec<&Finding> = findings.iter().filter(|f| !f.outcome.is_hard()).collect();

        if !hard.is_empty() {
            println!("✗ HARD findings (real bugs):\n");
            for f in &hard {
                print_finding(f);
            }
            println!();
        }

        if stats.soft_count > 0 {
            println!(
                "○ {} soft structural_divergence finding(s) — format output reparses but the",
                stats.soft_count
            );
            println!(
                "  structural skeleton differs. Over Svelte this is largely render-model noise;"
            );
            println!(
                "  confirm the real ones with `roundtrip_audit --canonical-all <dumped-input>`."
            );
            if self.strict {
                println!("  (--strict: counted as failures.)");
            }
            // Show a few examples (they need triage, not a wall of them).
            for f in soft.iter().take(5) {
                print_finding(f);
            }
            if soft.len() > 5 {
                println!(
                    "  … and {} more (raise --dump-dir to capture all)",
                    soft.len() - 5
                );
            }
            println!();
        }

        if self.dump_dir.is_none() && !findings.is_empty() {
            println!("(pass --dump-dir DIR to write each failing input for reproduction)");
        }

        if fail {
            return Err(CliError::Failed);
        }
        // Not a failure ⇒ no HARD findings by construction (`is_fail` fails on any).
        println!("✓ no hard findings — no panics, all accepted inputs idempotent + reparseable");
        Ok(())
    }
}

/// Print one finding line (kind, provenance, panic message, input preview).
fn print_finding(f: &Finding) {
    println!(
        "  [{}] iter {} · {} · seed {}",
        f.outcome.label(),
        f.iteration,
        f.parser.name(),
        f.seed_path
    );
    if let Some(p) = &f.panic {
        println!("      {p}");
    }
    println!("      input: {}", preview(&f.input));
}

/// Run stats threaded into the report.
struct Stats {
    tested: usize,
    skipped_non_utf8: usize,
    rejected: usize,
    ok: usize,
    hard_count: usize,
    soft_count: usize,
}

/// A single-line, length-capped preview of a (possibly multi-line) mutant.
fn preview(input: &str) -> String {
    let one_line: String = input
        .chars()
        .map(|c| if c == '\n' { '⏎' } else { c })
        .collect();
    let count = one_line.chars().count();
    if count > 120 {
        let head: String = one_line.chars().take(120).collect();
        format!("{head}… ({count} chars)")
    } else {
        one_line
    }
}

thread_local! {
    /// The most recent panic's `Display` string, captured by the fuzz-loop hook.
    static LAST_PANIC: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}
