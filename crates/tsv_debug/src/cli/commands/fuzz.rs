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
//!    reported finding, not a crash. A *hang* (the exponential-rebuild class)
//!    can't be caught in-process, so the loop leaves two tripwires instead:
//!    every attempt's input is written to a last-input repro file **before** the
//!    attempt (a killed hung run leaves its exact input on disk), and any attempt
//!    over `--slow-budget-ms` wall-clock is reported (never fatal — a new
//!    blowup instance shows up here first, on shapes `build_fanout_audit`'s
//!    synthetic axes don't build).
//! 2. **Idempotency.** For any input tsv accepts, `format` is a fixed point:
//!    `format(format(x)) == format(x)` (the fixture F1 invariant, here on inputs
//!    no fixture covers).
//! 3. **Structural reparse + leaf conservation.** `format(x)` must reparse to the
//!    *same document* (the [`roundtrip_audit`](super::roundtrip_audit) contract —
//!    output that mis-delimits but loses no characters is invisible to the
//!    char-frequency SAFETY check), reusing that command's structural-skeleton
//!    comparison, **plus** the complementary
//!    [`leaf_conservation_diff`](crate::audit::properties::leaf_conservation_diff)
//!    check — a still-parses value change (a mis-decoded string, a miscanonicalized
//!    number) the skeleton erases and so cannot see.
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
//! **Corpus-add-stable.** Each seed file draws its mutants from its **own** PRNG
//! stream, keyed by `(master seed, file path)` and scheduled round-robin — so
//! adding, removing, or renaming a fixture changes only *that file's* mutants:
//! every other file's stream is byte-identical, and a shrunken per-file budget
//! trims a stream's **tail** rather than rewriting it. A corpus edit therefore
//! can't reshuffle the gate onto an unrelated latent bug; it can only add the
//! new file's own mutants (which surfacing a real bug is the gate working).
//!
//! Beyond the corpus-derived gate run, two opt-in discovery aids: `--evolve`
//! feeds accepted mutants back into the seed pool (walking deeper into the
//! accepted-input space, where the formatter invariants actually bite), and
//! `--minimize` ddmin-shrinks each hard finding into a consumable reproduction.
//!
//! Not the differential leg (tsv-vs-canonical verdict): that needs the Deno
//! sidecar. This stays pure-Rust and self-contained, matching the
//! `test262 --gate` / `roundtrip_audit --gate` direction.

use argh::FromArgs;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use tsv_cli::cli::input::ParserType;

use super::profile::resolve_files;
use crate::audit::properties::{F1Outcome, f1_check};
use crate::cli::CliError;

/// Seeded mutational fuzzer: mutate corpus bytes and assert the parser never
/// panics, `format` is idempotent, and formatted output reparses structurally
/// equal.
///
/// Defaults to `tests/fixtures` as the seed corpus. Deterministic for a given
/// `--seed` + corpus — and corpus-add-stable: each seed file's mutants come from
/// its own path-keyed PRNG stream, so a corpus edit changes only that file's
/// mutants. Raise `--iterations` (or vary `--seed`) for discovery; add
/// `--evolve` and `--minimize` there. Exits 1 on any finding (a panic, a
/// non-idempotent format, or output that doesn't reparse to the same document),
/// 0 when clean — so it doubles as a CI gate.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "fuzz")]
#[allow(clippy::struct_excessive_bools)] // independent CLI flags
pub struct FuzzCommand {
    /// PRNG master seed (default 0) — same seed + corpus ⇒ identical run
    #[argh(option, default = "0")]
    seed: u64,

    /// number of mutated inputs to test (default 2000), scheduled round-robin
    /// over the seed files (each file's mutants come from its own stream)
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

    /// wall-clock budget per attempt in ms (default 2000); attempts over it are
    /// reported, never fatal — the tripwire for a NEW exponential-rebuild
    /// instance on shapes `build_fanout_audit`'s synthetic axes don't build
    #[argh(option, default = "2000")]
    slow_budget_ms: u64,

    /// discovery mode: a mutant that passes every invariant joins the seed pool
    /// (bounded at 2x the initial corpus), so later mutants walk deeper into the
    /// ACCEPTED-input space — the formatter's coverage. Off by default: the
    /// gate's mutant set should stay corpus-derived and prefix-stable
    #[argh(switch)]
    evolve: bool,

    /// ddmin-shrink each stored HARD finding before reporting/dumping: greedily
    /// remove byte chunks while the same outcome reproduces (bounded probes).
    /// Without it a finding is a whole seed file with up to --max-mutations
    /// random edits
    #[argh(switch)]
    minimize: bool,

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

/// Per-file stream seed: FNV-1a over the path bytes XOR the master seed,
/// finalized through one SplitMix64 step. Keying each file's mutant stream by
/// its own path is what makes the gate corpus-add-stable (see the module doc).
fn stream_seed(master: u64, path: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a offset basis
    for &b in path.as_bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01B3); // FNV-1a prime
    }
    Rng::new(h ^ master).next_u64()
}

/// Bytes that push a mutant toward *structurally* interesting input — the
/// delimiters, operators, and whitespace where parser/formatter edge cases live —
/// rather than uniformly-random noise that almost always fails to parse.
const INTERESTING: &[u8] = b"{}()[]<>;:,.\"'`/\\=+-*&|!?@#$%^~\n\t abc012";

/// Multi-byte sequences the single-byte ops essentially never assemble: the
/// unicode span/width stress set (NBSP, zero-width space, BOM, a combining
/// mark, CJK, a 4-byte emoji) plus CR/CRLF. Without these the fuzzer is blind
/// to non-ASCII span math — bit-flips almost never form valid multi-byte UTF-8,
/// so those mutants die at the `from_utf8` boundary.
const INTERESTING_SEQUENCES: &[&str] = &[
    "\u{A0}",
    "\u{200B}",
    "\u{FEFF}",
    "e\u{301}",
    "\u{4E2D}",
    "\u{1F600}",
    "\r\n",
    "\r",
];

/// Structure-bearing tokens byte-level ops essentially never assemble — Svelte
/// block/tag delimiters, TS operators, comment fences, CSS forms — aimed at the
/// parser's ACCEPT paths: a mutant must parse before the F1/reparse invariants
/// grade the formatter at all.
const INTERESTING_TOKENS: &[&str] = &[
    "{#if ",
    "{:else}",
    "{/if}",
    "{#each ",
    "{/each}",
    "{#snippet ",
    "{/snippet}",
    "{@render ",
    "{@const ",
    "{@html ",
    "${",
    "/**",
    "*/",
    "//",
    "<!--",
    "-->",
    "</script>",
    "<script>",
    "=>",
    "?.",
    "...",
    " satisfies ",
    " as const",
    " extends ",
    "@media ",
    "calc(",
    "url(",
    "!important",
    "\\3A ",
];

/// Insert one of `set` at a random position, snapped forward to a UTF-8
/// boundary so a splice into the middle of an existing multi-byte char doesn't
/// waste the mutant on invalid UTF-8.
fn insert_str(rng: &mut Rng, buf: &mut Vec<u8>, set: &[&str]) {
    let s = set[rng.below(set.len())];
    let mut at = rng.below(buf.len() + 1);
    while at < buf.len() && (buf[at] & 0xC0) == 0x80 {
        at += 1;
    }
    for (k, b) in s.bytes().enumerate() {
        buf.insert(at + k, b);
    }
}

/// Apply 1..=`max_ops` byte-level mutation operators to a copy of `seed`.
fn mutate(rng: &mut Rng, seed: &[u8], max_ops: usize) -> Vec<u8> {
    let mut buf = seed.to_vec();
    let ops = 1 + rng.below(max_ops.max(1));
    for _ in 0..ops {
        if buf.is_empty() {
            buf.push(rng.byte());
            continue;
        }
        match rng.below(10) {
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
            // Insert a multi-byte unicode/CRLF sequence (span/width stress).
            7 => insert_str(rng, &mut buf, INTERESTING_SEQUENCES),
            // Insert a structure-bearing token (aimed at the accept paths).
            8 => insert_str(rng, &mut buf, INTERESTING_TOKENS),
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
    /// Output reparses with an **equal skeleton** but a decode-invariant leaf value
    /// changed (a mis-decoded string, a miscanonicalized number, a mangled
    /// comment) — the skeleton-blind class (a shape change is instead the soft
    /// `structural_divergence`). A hard finding.
    LeafValueCorruption,
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
    /// point; `leaf_value_corruption` is a still-parses value change (invisible to
    /// the structural skeleton). `structural_divergence` is deliberately excluded —
    /// it's the soft, canonical-confirmation-needing bucket (see
    /// [`FuzzCommand::strict`]).
    fn is_hard(self) -> bool {
        matches!(
            self,
            Self::Panic
                | Self::FormatError
                | Self::Unreparseable
                | Self::LeafValueCorruption
                | Self::NonIdempotent
        )
    }

    fn label(self) -> &'static str {
        match self {
            Self::Rejected => "rejected",
            Self::Ok => "ok",
            Self::Panic => "panic",
            Self::FormatError => "format_error",
            Self::Unreparseable => "unreparseable",
            Self::LeafValueCorruption => "leaf_value_corruption",
            Self::StructuralDivergence => "structural_divergence",
            Self::NonIdempotent => "non_idempotent",
        }
    }
}

/// Run the three invariant checks on one (already valid-UTF-8) mutant. Any panic
/// is caught by [`attempt`]'s [`catch_unwind`](std::panic::catch_unwind); this
/// returns the non-panic outcome.
///
/// A thin map over the shared [`f1_check`] — the six-step sequence lives in
/// [`audit::properties`](crate::audit::properties) so `blank_audit` shares it. The mapping is
/// 1:1 and total, so the fuzzer's behavior is byte-for-byte what the inline version produced;
/// [`Outcome`] keeps `Panic` (which [`attempt`] supplies) plus fuzz's own labels.
fn check(src: &str, parser: ParserType, render: bool) -> Outcome {
    match f1_check(src, parser, render) {
        F1Outcome::Rejected => Outcome::Rejected,
        F1Outcome::Ok => Outcome::Ok,
        F1Outcome::FormatError => Outcome::FormatError,
        F1Outcome::Unreparseable => Outcome::Unreparseable,
        F1Outcome::LeafValueCorruption => Outcome::LeafValueCorruption,
        F1Outcome::StructuralDivergence => Outcome::StructuralDivergence,
        F1Outcome::NonIdempotent => Outcome::NonIdempotent,
    }
}

/// One guarded attempt: write the last-input repro, clear the panic slot, run
/// [`check`] under `catch_unwind`, map a panic to [`Outcome::Panic`].
fn attempt(src: &str, parser: ParserType, render: bool, last: &mut LastInput) -> Outcome {
    last.write(parser, src);
    LAST_PANIC.with(|c| *c.borrow_mut() = None);
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(src, parser, render))) {
        Ok(o) => o,
        Err(_) => Outcome::Panic,
    }
}

/// File extension for a parser — dump + last-input repro file names.
fn parser_ext(parser: ParserType) -> &'static str {
    match parser {
        ParserType::TypeScript => "ts",
        ParserType::Svelte => "svelte",
        ParserType::Css => "css",
    }
}

/// Best-effort pre-attempt repro file: written **before** every parse/format
/// attempt and removed on an orderly exit, so an input that HANGS the formatter
/// (the exponential-rebuild class — `catch_unwind` can't see an infinite loop)
/// leaves its exact bytes on disk for triage after the process is killed.
struct LastInput {
    /// The paths written this run (one per extension), removed on cleanup.
    paths: HashSet<PathBuf>,
    warned: bool,
}

impl LastInput {
    fn new() -> Self {
        Self {
            paths: HashSet::new(),
            warned: false,
        }
    }

    /// The per-process repro path pattern, for the startup notice.
    fn pattern() -> PathBuf {
        std::env::temp_dir().join(format!("tsv_fuzz_last_input_{}.*", std::process::id()))
    }

    fn write(&mut self, parser: ParserType, src: &str) {
        let path = std::env::temp_dir().join(format!(
            "tsv_fuzz_last_input_{}.{}",
            std::process::id(),
            parser_ext(parser)
        ));
        match std::fs::write(&path, src) {
            Ok(()) => {
                self.paths.insert(path);
            }
            Err(e) => {
                if !self.warned {
                    eprintln!(
                        "warning: cannot write last-input repro {}: {e}",
                        path.display()
                    );
                    self.warned = true;
                }
            }
        }
    }

    fn cleanup(self) {
        for p in self.paths {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// One recorded finding, enough to reproduce and triage.
struct Finding {
    /// The mutation iteration that produced it, or `None` when the *unmutated*
    /// seed file itself violated an invariant (the pristine pass).
    iteration: Option<usize>,
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
    /// This file's own mutant stream — keyed by `(master seed, path)`, so a
    /// corpus edit elsewhere never changes the mutants THIS file produces.
    rng: Rng,
}

/// Bound on stored findings per severity (they hold the mutant text, so a noisy
/// discovery run must not balloon memory — the counts stay exact regardless).
/// Hard findings get their **own** store so a noisy soft
/// (`structural_divergence`) run can never crowd a hard finding's reproduction
/// out of the report/dump.
const HARD_STORE_CAP: usize = 100;
const SOFT_STORE_CAP: usize = 50;

/// The findings a run accumulates: exact counts plus bounded per-severity
/// stores of the findings themselves.
///
/// Owns the counters so both passes (pristine seeds, then mutants) share one
/// [`Self::record`] rather than threading a `&mut` per counter through a closure.
struct Found {
    hard: usize,
    soft: usize,
    hard_store: Vec<Finding>,
    soft_store: Vec<Finding>,
}

impl Found {
    fn new() -> Self {
        Self {
            hard: 0,
            soft: 0,
            hard_store: Vec::new(),
            soft_store: Vec::new(),
        }
    }

    /// Count a finding and store it if its severity's store has room.
    /// `iteration` is `None` for a pristine (unmutated) seed.
    fn record(
        &mut self,
        outcome: Outcome,
        iteration: Option<usize>,
        seed_path: &str,
        parser: ParserType,
        src: &str,
    ) {
        let (count, store, cap) = if outcome.is_hard() {
            (&mut self.hard, &mut self.hard_store, HARD_STORE_CAP)
        } else {
            (&mut self.soft, &mut self.soft_store, SOFT_STORE_CAP)
        };
        *count += 1;
        if store.len() < cap {
            let panic = if outcome == Outcome::Panic {
                LAST_PANIC.with(|c| c.borrow_mut().take())
            } else {
                None
            };
            store.push(Finding {
                iteration,
                parser,
                outcome,
                seed_path: seed_path.to_string(),
                input: src.to_string(),
                panic,
            });
        }
    }
}

/// Probe budget per finding for `--minimize` — each probe is a full
/// parse+format attempt on a shrinking input (cheap; panicky findings pay
/// unwind cost per probe).
const MINIMIZE_PROBE_BUDGET: usize = 1000;

/// Greedy ddmin-lite: repeatedly remove byte chunks (halving the chunk size on
/// stall) while the SAME outcome reproduces. Deterministic and bounded by
/// [`MINIMIZE_PROBE_BUDGET`]. The result is a locally minimal reproduction —
/// the point is a consumable finding, not a global minimum.
fn minimize(
    orig: &str,
    parser: ParserType,
    render: bool,
    target: Outcome,
    last: &mut LastInput,
) -> String {
    let mut best = orig.as_bytes().to_vec();
    let mut probes = 0usize;
    let mut chunk = (best.len() / 2).max(1);
    loop {
        let mut removed_any = false;
        let mut i = 0;
        while i < best.len() && probes < MINIMIZE_PROBE_BUDGET {
            let end = (i + chunk).min(best.len());
            let mut cand = Vec::with_capacity(best.len() - (end - i));
            cand.extend_from_slice(&best[..i]);
            cand.extend_from_slice(&best[end..]);
            probes += 1;
            let reproduces = !cand.is_empty()
                && std::str::from_utf8(&cand)
                    .is_ok_and(|s| attempt(s, parser, render, last) == target);
            if reproduces {
                best = cand;
                removed_any = true; // retry the same offset at this chunk size
            } else {
                i += chunk;
            }
        }
        if probes >= MINIMIZE_PROBE_BUDGET || (chunk == 1 && !removed_any) {
            break;
        }
        if !removed_any {
            chunk = (chunk / 2).max(1);
        }
    }
    // `best` is valid UTF-8 by construction (candidates are only accepted after
    // a successful `from_utf8`); the fallback is pure defensiveness.
    String::from_utf8(best).unwrap_or_else(|_| orig.to_string())
}

/// Cap on stored slow-attempt entries (the count stays exact).
const SLOW_STORE_CAP: usize = 20;

/// Wall-clock outliers: attempts that exceeded `--slow-budget-ms`. Never fatal —
/// debug-build timing is noisy — but a NEW exponential-rebuild instance shows
/// up here first.
struct Slow {
    count: usize,
    store: Vec<(String, u128)>,
}

impl Slow {
    fn new() -> Self {
        Self {
            count: 0,
            store: Vec::new(),
        }
    }

    fn track(&mut self, elapsed: Duration, budget_ms: u64, origin: impl FnOnce() -> String) {
        if elapsed.as_millis() > u128::from(budget_ms) {
            self.count += 1;
            if self.store.len() < SLOW_STORE_CAP {
                self.store.push((origin(), elapsed.as_millis()));
            }
        }
    }
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

        let mut seeds: Vec<Seed> = files
            .iter()
            .filter_map(|p| {
                let display = p.to_string_lossy().into_owned();
                let bytes = std::fs::read(p).ok()?;
                Some(Seed {
                    parser: ParserType::from_extension(&display),
                    rng: Rng::new(stream_seed(self.seed, &display)),
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
        let mut last_input = LastInput::new();
        eprintln!(
            "last-input repro: {} (written before each attempt; removed on a clean exit — \
             if this run hangs, the hung input is in that file)",
            LastInput::pattern().display()
        );

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
        let mut evolved = 0usize;
        let mut pristine_reflow = 0usize;
        // Paths of the pristine seeds that reflowed (see the pass-1 soft arm). Bounded
        // like the findings store, though each entry is only a path; the count stays exact.
        let mut reflow_paths: Vec<String> = Vec::new();
        let mut found = Found::new();
        let mut slow = Slow::new();

        // Pass 1 — every seed **as authored**. The mutation loop below only ever drives
        // mutants, so a pristine corpus file is never itself checked; yet the corpus is
        // the richest source of real, formatter-reachable inputs. Over `tests/fixtures`
        // this is the only gate that drives the *non-`input.*`* fixture files — the
        // validator claims F1 on `input.*` alone, so `output_prettier.*`, `variant_*`
        // and `unformatted_*` (all real, formatter-reachable code) went unchecked.
        for seed in &seeds {
            let Ok(src) = std::str::from_utf8(&seed.bytes) else {
                skipped_non_utf8 += 1;
                continue;
            };
            tested += 1;

            let started = Instant::now();
            let outcome = attempt(src, seed.parser, render, &mut last_input);
            slow.track(started.elapsed(), self.slow_budget_ms, || {
                format!("as authored · {}", seed.display)
            });

            match outcome {
                Outcome::Rejected => rejected += 1,
                Outcome::Ok => ok += 1,
                // The soft (structural_divergence) verdict does not FAIL a pristine
                // seed: over `tests/fixtures` the corpus deliberately holds
                // mis-formatted files (`unformatted_*`) whose formatting legitimately
                // reflows the whitespace skeleton, and structural reparse is
                // `roundtrip_audit`'s gate anyway (it has the canonical confirmation
                // this compare lacks). It is still REPORTED: over a real-code corpus
                // (`deno task idempotency:sweep`) there are no `unformatted_*` files,
                // so every one of these wants triage — and a count with no paths is
                // not actionable. The seed path IS the reproduction (an unmutated file
                // on disk), so record that rather than dumping the input.
                f if !f.is_hard() => {
                    pristine_reflow += 1;
                    if reflow_paths.len() < REFLOW_PATH_CAP {
                        reflow_paths.push(seed.display.clone());
                    }
                }
                finding => found.record(finding, None, &seed.display, seed.parser, src),
            }
        }

        // Pass 2 — mutants, round-robin over the (sorted) seed files, each file
        // drawing from its own stream (the corpus-add-stability property).
        let evolve_cap = seeds.len() * 2;
        for iteration in 0..self.iterations {
            let idx = iteration % seeds.len();
            let mutated = {
                let Seed { rng, bytes, .. } = &mut seeds[idx];
                mutate(rng, bytes, self.max_mutations)
            };
            let Ok(src) = std::str::from_utf8(&mutated) else {
                skipped_non_utf8 += 1;
                continue;
            };
            tested += 1;

            let parser = seeds[idx].parser;
            let started = Instant::now();
            let outcome = attempt(src, parser, render, &mut last_input);
            slow.track(started.elapsed(), self.slow_budget_ms, || {
                format!("iter {iteration} · {}", seeds[idx].display)
            });

            match outcome {
                Outcome::Rejected => rejected += 1,
                Outcome::Ok => {
                    ok += 1;
                    if self.evolve && seeds.len() < evolve_cap {
                        // The evolved seed gets its own stream, keyed by its
                        // synthetic display name (unique per origin iteration).
                        let display = format!("{}·evolved@i{iteration}", seeds[idx].display);
                        let rng = Rng::new(stream_seed(self.seed, &display));
                        evolved += 1;
                        seeds.push(Seed {
                            display,
                            bytes: mutated,
                            parser,
                            rng,
                        });
                    }
                }
                finding => {
                    found.record(finding, Some(iteration), &seeds[idx].display, parser, src);
                    // Only HARD findings stop the run early — soft ones (render-
                    // model-noisy structural divergences) are counted, not fatal.
                    if self.max_findings > 0 && found.hard >= self.max_findings {
                        break;
                    }
                }
            }
        }

        // Shrink hard findings while the custom panic hook is still installed
        // (minimizing a panic finding re-panics per probe).
        if self.minimize {
            for f in &mut found.hard_store {
                f.input = minimize(&f.input, f.parser, render, f.outcome, &mut last_input);
            }
        }

        std::panic::set_hook(prev_hook);
        last_input.cleanup();

        let all: Vec<&Finding> = found
            .hard_store
            .iter()
            .chain(found.soft_store.iter())
            .collect();
        self.dump_findings(&all);
        let stats = Stats {
            tested,
            skipped_non_utf8,
            rejected,
            ok,
            evolved,
            hard_count: found.hard,
            soft_count: found.soft,
            pristine_reflow,
            reflow_paths,
            slow_count: slow.count,
            slow: slow.store,
        };
        self.report(&stats, &found.hard_store, &found.soft_store)
    }

    /// Write each finding's input to `--dump-dir` (if set) for reproduction.
    fn dump_findings(&self, findings: &[&Finding]) {
        let Some(dir) = &self.dump_dir else {
            return;
        };
        for (n, f) in findings.iter().enumerate() {
            let name = format!(
                "finding_{n:03}_{}.{}",
                f.outcome.label(),
                parser_ext(f.parser)
            );
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

    fn report(&self, stats: &Stats, hard: &[Finding], soft: &[Finding]) -> Result<(), CliError> {
        let fail = self.is_fail(stats);

        if self.json {
            let findings_json: Vec<serde_json::Value> = hard
                .iter()
                .chain(soft.iter())
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
            let slow_json: Vec<serde_json::Value> = stats
                .slow
                .iter()
                .map(|(origin, ms)| serde_json::json!({ "origin": origin, "ms": ms }))
                .collect();
            let out = serde_json::json!({
                "seed": self.seed,
                "iterations": self.iterations,
                "tested": stats.tested,
                "skipped_non_utf8": stats.skipped_non_utf8,
                "rejected": stats.rejected,
                "ok": stats.ok,
                "evolved": stats.evolved,
                "hard_findings": stats.hard_count,
                "soft_findings": stats.soft_count,
                "pristine_reflow": stats.pristine_reflow,
                "pristine_reflow_paths": stats.reflow_paths,
                "slow_budget_ms": self.slow_budget_ms,
                "slow_count": stats.slow_count,
                "slow": slow_json,
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
        if self.evolve {
            println!(
                "  {:>7}  accepted mutants evolved into the seed pool",
                stats.evolved
            );
        }
        println!(
            "  {:>7}  seed files that reflow structurally when formatted, as authored\n           (not a failure — see below)",
            stats.pristine_reflow
        );
        println!(
            "  {:>7}  HARD findings (panic / unreparseable / non-idempotent)",
            stats.hard_count
        );
        println!(
            "  {:>7}  soft findings (structural_divergence — see below)\n",
            stats.soft_count
        );

        if !hard.is_empty() {
            println!("✗ HARD findings (real bugs):\n");
            for f in hard {
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

        if stats.pristine_reflow > 0 {
            println!(
                "○ {} seed file(s) whose format reflows the structural skeleton, AS AUTHORED.",
                stats.pristine_reflow
            );
            println!(
                "  Not a run failure: over tests/fixtures the `unformatted_*` seeds reflow by"
            );
            println!(
                "  design, and structural reparse is roundtrip_audit's gate (it has the canonical"
            );
            println!(
                "  confirmation this compare lacks). Over a REAL-CODE corpus there are no such"
            );
            println!("  seeds — triage each with `roundtrip_audit --canonical-all <path>`:");
            for p in stats.reflow_paths.iter().take(20) {
                println!("    {p}");
            }
            if stats.pristine_reflow > stats.reflow_paths.len().min(20) {
                println!(
                    "    … and {} more",
                    stats.pristine_reflow - stats.reflow_paths.len().min(20)
                );
            }
            println!();
        }

        if stats.slow_count > 0 {
            println!(
                "○ {} attempt(s) over the --slow-budget-ms wall-clock budget ({} ms) — not a",
                stats.slow_count, self.slow_budget_ms
            );
            println!(
                "  failure (debug-build timing is noisy), but a NEW exponential-rebuild instance"
            );
            println!("  shows up here first (fanout_audit guards only the known synthetic axes):");
            for (origin, ms) in &stats.slow {
                println!("    {ms:>6} ms  {origin}");
            }
            if stats.slow_count > stats.slow.len() {
                println!("    … and {} more", stats.slow_count - stats.slow.len());
            }
            println!();
        }

        if self.dump_dir.is_none() && (!hard.is_empty() || !soft.is_empty()) {
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
    let origin = match f.iteration {
        Some(i) => format!("iter {i}"),
        None => "as authored".to_string(),
    };
    println!(
        "  [{}] {} · {} · seed {}",
        f.outcome.label(),
        origin,
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
    /// Accepted mutants fed back into the seed pool (`--evolve` only).
    evolved: usize,
    hard_count: usize,
    soft_count: usize,
    /// Pristine seeds whose formatting reflows the whitespace skeleton. Not a run
    /// failure (the `unformatted_*` fixtures reflow by design), but reported with
    /// paths — over a real-code corpus every one wants triage.
    pristine_reflow: usize,
    /// The seed paths behind `pristine_reflow`, capped at [`REFLOW_PATH_CAP`].
    reflow_paths: Vec<String>,
    /// Attempts over the `--slow-budget-ms` wall-clock budget.
    slow_count: usize,
    /// `(origin, elapsed ms)` behind `slow_count`, capped at [`SLOW_STORE_CAP`].
    slow: Vec<(String, u128)>,
}

/// Cap on stored `reflow_paths` — enough to triage, bounded on a noisy corpus.
const REFLOW_PATH_CAP: usize = 50;

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
