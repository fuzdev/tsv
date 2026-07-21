# Performance

Profiling methodology and tracking for the TypeScript/Svelte/CSS formatter.

**Goal:** Identify where time is spent, make targeted improvements, and measure before/after.

## Formatter Pipeline

```
source → Parse → AST → Format → formatted string
         lexer    │      per-statement:
         parser   │        build_statement_doc() → DocId (arena-allocated)
                  │        write_arena_doc() → arena_print_doc_with_indent_resolved()
                  │          └── arena_fits() (line-breaking decisions)
                  │
              tsv_ts::parse()                      tsv_ts::format()
```

Doc building and rendering are **interleaved** per-statement inside `format()` — each statement's Doc is built as arena-allocated `DocId` nodes and immediately rendered. This means the cleanest measurable phase split is **parse vs format**. Within format, `perf` can break down time further by function.

**Key files:**

- Parse — `tsv_ts` — `parser::parse_typescript()`
- Format (orchestration) — `tsv_ts` — `printer::Printer::print_program()`
- Doc building — `tsv_ts` — `printer::Printer::build_statement_doc()` → `DocId`
- Doc rendering — `tsv_lang` — `doc::arena_render::arena_print_doc_with_indent_resolved()`
- Line-break decisions — `tsv_lang` — `doc::arena_fits::arena_fits()`

## Measurement corpora

Comment- and allocation-path work is corpus-sensitive — comment density alone
varies by an order of magnitude across the trees below — so pick the corpus to
match what you're measuring, and **run a SmallVec-sizing histogram and the
heaptrack that validates it on the _same_ corpus.** Gate on the measured
alloc/wall delta, never a static spill rate: a high spill *rate* over a small
*population* (comment-collect spills are a fraction of a percent of all
allocations) is a negligible absolute change.

- **Headline rate / profile** — `~/dev/zzz/src/lib`. Typical app code,
  comment-sparse; the per-byte baseline the tables here track.
- **Comment- / alloc-dense stress** — `~/dev/fuz_app/src/lib`. TSDoc-dense
  library code; the extreme for comment-path and allocation changes (zzz's
  comment density is a fraction of fuz_app's, so zzz alone under-represents
  these paths).
- **Svelte-component-dense** — `~/dev/fuz_ui/src/lib`. Mostly `.svelte`
  components with a thin `.ts` slice — the markup-heavy complement to
  fuz_app's TSDoc-dense TS, and a stable in-ecosystem stand-in for the
  external `.svelte` slices below.
- **Representative real-world** — `~/dev/svelte/packages/svelte/src`,
  `~/dev/kit/packages/kit/src`, and `~/dev/svelte-docinfo/src`. Large, diverse
  sources at moderate comment density — the middle ground the two app corpora
  bracket. svelte and kit are mostly `.js` (which `tsv format` skips, but
  `profile`/`json_profile` still time, parsed as TypeScript), so kit's
  `.svelte` + `.ts` and svelte-docinfo's `.ts` are the formattable slices.

**Measuring one language in isolation:** because `profile`/`json_profile` route
every non-`.svelte`/`.css` file to the TypeScript parser, a directory that
co-locates other files with the language under test pollutes that language's
rate — e.g. `../prettier/tests/format/css` holds per-directory `.js` test
drivers beside its `.css` fixtures. Copy only the target extension into a
scratch directory and profile that.

**There is no CSS corpus in the list above, and the obvious guess is a trap.**
`~/dev/fuz_css/src` is a CSS *framework*, but by bytes it is ~92% TypeScript —
profiling it measures the TS path and reads a CSS change as noise, which is
exactly how a real CSS win gets mistaken for a placement artifact (and a CSS
*regression* gets missed). Build a genuine `.css`-only corpus instead: run
`deno task bench:harvest:svelte-styles` to extract real `<style>` blocks into
`benches/js/.cache/svelte_styles/`, and add the authored stylesheets scattered
across the ecosystem and the spec checkouts (`fuz_css/src/lib/{theme,style}.css`,
`../csswg-drafts`, `../wpt/css`). That lands ~1 MB of real CSS, enough to hold a
sub-percent read steady. For attribution in the other direction, a **pure-`.ts`**
corpus (no `.svelte` — a `.svelte` file's `<style>` block routes through the CSS
parser) is the control that must read ~0.000% for a CSS-only change.

## Tooling

Four tools, in order of use:

### 1. `tsv_debug profile` — phase timing

Measures parse vs format timing across files. Pure Rust, no external dependencies.

```bash
# Profile a directory
cargo run --release -p tsv_debug -- profile ~/dev/zzz/src/lib

# Profile specific files
cargo run --release -p tsv_debug -- profile file1.ts file2.svelte

# More iterations for stability (default: 10)
cargo run --release -p tsv_debug -- profile ~/dev/zzz/src/lib --iterations 20

# JSON output for scripting
cargo run --release -p tsv_debug -- profile ~/dev/zzz/src/lib --json
```

Output shows per-file and aggregate timing, plus normalized rates. The
`split` column is parse time as a percentage of total (lower =
format-dominated, higher = parse-dominated); `us/KB` is the per-byte rate.
The summary block adds per-language totals (when languages are mixed) and
`per file` / `per KB` rows. Wall totals move with corpus growth/shrink, so
compare the rates across runs — on a quiet machine; rates normalize corpus
changes, not machine state (see the wall-clock caveat below):

```
                                   file    lang     size       parse      format       total  split    us/KB
                                   ----    ----     ----       -----      ------       -----  -----    -----
 .../src/lib/CapabilityWebsocket.svelte  svelte   12.3KB       608us     10.22ms     10.83ms     6%    876.9
  .../src/lib/SocketMessageQueue.svelte  svelte   10.1KB       502us      7.21ms      7.71ms     7%    763.1
          .../src/lib/socket_helpers.ts      ts     248B         6us         8us        14us    44%     57.7
                                                    ----       -----      ------       -----
                             (89 files)      ts  369.9KB     12.81ms     31.89ms     44.70ms    29%    120.8
                            (123 files)  svelte  250.1KB     11.67ms     70.81ms     82.49ms    14%    329.8
                            (212 files)          620.0KB     24.48ms    102.70ms    127.18ms    19%    205.1
                               per file            2.9KB       115us       484us       600us
                                 per KB                       39.5us     165.6us     205.1us

iterations: 30 (median shown)
```

The table above is illustrative sample output — absolute wall times are
machine-dependent; compare per-byte rates across runs, not wall totals. Uses
median of N iterations to reduce noise from OS scheduling.
The same aggregates (including the per-language breakdown under `langs`) are in
the `--json` output as `*_us_per_kb` / `*_us_per_file` fields.

### 2. `tsv_debug json_profile` — parse→JSON emission timing

Times the two phases of the FFI parse path (`parse` +
`convert_ast_json_bytes`) per file across a corpus. The writer
(`convert_ast_json_bytes`) is the sole emission path — it walks the internal
AST once and emits the final char-space wire JSON directly, so there are no
sub-steps to decompose (per-language pipeline shapes:
[architecture.md §Closed Scope, Open Convention](./architecture.md#closed-scope-open-convention)).
Pure Rust, no external dependencies.

```bash
# Profile a directory (aggregate report per language)
cargo run --release -p tsv_debug -- json_profile ~/dev/zzz/src/lib

# JSON output with per-file data (e.g. to split costs by multibyte flag)
cargo run --release -p tsv_debug -- json_profile ~/dev/zzz/src/lib --json
# Also: --iterations <n> (default: 5)
```

Output shows, per language: file/byte/wire-byte/multibyte counts and the
`parse` and `write` medians (sums of per-file medians).

### 3. `[profile.profiling]` — cargo profile for perf

The release profile strips debug symbols (`strip = true`), making `perf` useless. The `profiling` profile keeps symbols at release speed:

```toml
# Already configured in Cargo.toml
[profile.profiling]
inherits = "release"
debug = true
strip = false
```

Build with: `cargo build --profile profiling -p tsv_debug`

Because `profiling` inherits `release` (only `debug`/`strip` differ), codegen
is identical — wall rates and `perf stat` instruction counts read the same on
either build; only the symbol-attributing tools (`perf report`/`annotate`,
heaptrack) need the `profiling` build's retained symbols.

### 4. `perf` — function-level and line-level hotspots

Once phase timing identifies _which_ phase to optimize, `perf` identifies _which functions_ within that phase.

```bash
# Record samples while profiling a workload
cargo build --profile profiling -p tsv_debug
perf record --call-graph=dwarf -- target/profiling/tsv_debug profile ~/dev/zzz/src/lib

# Function-level hotspots (text output)
perf report --stdio

# Line-level hotspots within a specific function (exact demangled name from perf report)
perf annotate --stdio -s 'tsv_lang::doc::arena::DocArena::will_break_fill'

# Collapsed stacks (greppable text, one line per unique stack; cargo install inferno)
perf script | inferno-collapse-perf > stacks.txt
grep fits_with_lookahead stacks.txt | head
```

`perf annotate -s` matches the **exact demangled name** as shown in
`perf report` — a substring silently annotates nothing. It also comes up
empty for functions with multiple monomorphizations sharing one demangled
name (e.g. `arena_fits_with_lookahead`, instantiated per `TextResolver`).
For those, dump everything and search by source line instead:

```bash
perf annotate --stdio > annotate.txt   # then search for the fn's source lines
```

**Telling real work from a code-layout artifact — `perf stat`.** When a logically
tiny change moves the native wall, count instructions before chasing a function:
**instruction count is layout-independent** (code placement cannot change how many
instructions run), so it separates real added work from a frontend/i-cache effect.

```bash
# Deterministic counts (±0.00% across -r runs); compare two binaries, one workload
perf stat -r 4 -e instructions,cycles,branches,branch-misses \
  target/profiling/tsv_debug profile ~/dev/fuz_app/src/lib --iterations 30
```

A near-flat instruction delta (e.g. ≤0.1%) paired with a larger, run-to-run-*variable*
cycles delta and a drop in instructions-per-cycle is a code-placement / i-cache
artifact, not a real cost — added code (a new monomorphization, more inlining)
shifted hot functions across cache lines. For a printer-only edit, **parse is a
built-in control**: its code is unchanged, so any instruction movement there is pure
layout. A real algorithmic change instead shows up as more *instructions*.

Anchor instruction counts on an **in-process corpus run** — `profile`
(parse+format) or `json_profile` (parse + wire-JSON write) over a directory
with `--iterations N`. A per-file `tsv parse` spawn loop (the CLI is
single-file) is a different anchor: it measures the whole CLI path including
process startup, dynamic linking, and allocator warmup per file — useful for
CLI-boundary changes, but never comparable to the in-process numbers.

For visual flamegraphs (useful for humans, not Claude):

```bash
cargo install flamegraph
cargo flamegraph --profile profiling -p tsv_debug -- profile ~/dev/zzz/src/lib
```

On Debian, `perf` ships in the `linux-perf` package (there is no package named
`perf`), and unprivileged profiling additionally requires
`kernel.perf_event_paranoid <= 2` — Debian patches the kernel default to 3,
which blocks unprivileged perf entirely:

```bash
sudo apt install linux-perf
sudo sysctl kernel.perf_event_paranoid=2  # persist via a drop-in in /etc/sysctl.d/
```

Both steps are automated in ~/dev/setup (`setup_zap/zap.ts`).

### 5. `heaptrack` — allocation-site profiling

When `perf` shows time inside malloc/free internals, it can't say _which_
allocation sites are responsible — glibc's allocator is diffuse from the CPU
side. `heaptrack` attributes every allocation to its call site, answering
"swap the allocator" vs "fix the hot sites" — and it sized, then confirmed, the
AST bump-arena win (per-node `Box`/`Vec` allocations collapse into the arena; see
[architecture.md §Nested AST](./architecture.md#nested-ast-bump-arena-not-flatindexed)
for the AST-allocation design).

```bash
# Record (build with the profiling profile for symbols)
cargo build --profile profiling -p tsv_debug
heaptrack -o /tmp/heaptrack_tsv target/profiling/tsv_debug profile ~/dev/zzz/src/lib --iterations 2

# Bounded textual report (top allocators / peaks / temporaries)
heaptrack_print /tmp/heaptrack_tsv.zst -n 30 > report.txt

# Collapsed stacks for custom aggregation (by crate, phase, container kind)
heaptrack_print /tmp/heaptrack_tsv.zst -F allocs.folded --flamegraph-cost-type allocations -p0 -a0 -T0

# Full file:line backtraces for one site
heaptrack_print /tmp/heaptrack_tsv.zst -a1 -p0 -T0 -n3 -s8 --filter-bt-function build_chain_doc
```

Notes:

- **Allocation counts are machine-load-independent** — unlike wall time, they
  are stable across machine states. **Never read wall times off a heaptrack
  run**; the instrumentation inflates them severalfold.
- **An allocation-count cut is not automatically a wall-time win.** This is a
  traversal-bound formatter — the doc IR is walked many times (fitting memos +
  render), so storage **locality** and per-read cost can dominate `malloc`/`free`
  call count. A change that reduces allocations can be wall-neutral, or even
  *regress* format wall-time and peak memory — e.g. relocating hot,
  repeatedly-walked storage, or trading a tight contiguous `Vec` for a sparser
  arena that hurts cache density. A subtler regression is not about the data at
  all: added code (e.g. the inline-vs-heap discriminant a `Vec`→`SmallVec` swap
  inlines at each site) shifts **code placement** and can nudge hot functions across i-cache
  lines, raising the native wall while the *instruction* count stays flat — a
  code-layout artifact, not a real cost (confirm with `perf stat`, §4). It is
  corpus-dependent and can hit a corpus that barely exercises the changed path
  *harder* than one that leans on it, and it does not touch the WASM-format wall.
  Allocation count is the right gate for the
  **WASM-format** wall (allocator work in linear memory is costlier than
  native malloc, §6) *only when
  storage stays cache-dense*, and a churn signal for native — never a substitute
  for the format-phase wall A/B itself (`tsv_debug profile` native rate with
  parse as the machine-state control, plus `wasm_format_probe`). Confirm the
  wall; don't accept an alloc-count reduction as a format win on its own.
- Low `--iterations` is fine: attribution is ratio-based, and heaptrack
  overhead scales with allocation count.
- Cost types are `allocations`, `temporary`, `peak`, `leaked` — there is **no
  total-bytes-allocated axis**, so use counts as the churn metric (malloc/free
  internal cost scales with call count at typical allocation sizes) and peak
  as the footprint metric. `temporary` (freed with no intervening allocation)
  isolates pure churn.
- Folded exports are **multi-GB** (full Rust symbols × distinct stacks) —
  write them to a filesystem with room (e.g. `target/`), not tmpfs.
- The folded lines are root-first `frame;frame;...;leaf count`. Aggregating
  by the nearest first-party frame above the `alloc::`/`raw_vec` plumbing
  gives a per-site table; classifying by the plumbing frames distinguishes
  `Vec` growth / `String` / `Box`. With `--profile profiling` many small
  allocations inline the plumbing entirely, so a first-party leaf usually
  means an inlined `Vec`/`Box` alloc.
- **Caveat — the `-F` leaf over-credits pure dispatchers.** The folded leaf is
  the *symbol owning the allocation address*, so when the compiler inlines a
  small allocating callee into its caller the leaf moves up to that caller. A
  `match` dispatcher with no own buffer (e.g. `build_fragment_node_doc_impl`,
  `build_chain_doc`) then absorbs its inlined delegates' allocations and reads
  as the hot site when the real owner is a callee (one such delegate inlining a
  per-element `Vec` makes the dispatcher look like a `String`-content cluster
  when it is element-structure scratch). Before trusting a dispatcher leaf,
  cross-check it against the `-a` source-line backtraces
  (`heaptrack_print … -a1 --filter-bt-function <fn>`): `-a` expands inline
  frames with `file:line`, dis-aggregating the inlined delegates back to their
  own functions and the exact arm/line — so the apparent owner and its true
  callees separate. Leaf-attribute *then* `-a`-confirm before scoping a fix.

**Bounding an allocator swap without adding the dependency**: `LD_PRELOAD`
an alternative allocator and A/B it against glibc with paired interleaved
runs — alternate baseline/preload within each pair so machine drift cancels,
and compare pair medians, not absolute readings:

```bash
LD_PRELOAD=/usr/lib/x86_64-linux-gnu/libmimalloc.so.3 \
  target/profiling/tsv_debug profile ~/dev/zzz/src/lib --iterations 20 --json
```

Run an A/A control (same binary on both sides of each pair) to calibrate the
noise floor before trusting any delta; on this workload the floor is roughly
±1–3% per metric even on a quiet machine.

### 6. `wasm_format_probe.ts` — WASM format wall-time A/B

The tools above measure the native Rust side. Allocation *counts* are
target-independent (heaptrack reads the same on either), but WASM *wall-time* is
not: `@fuzdev/tsv_format_wasm` runs on talc (the wasm32 `#[global_allocator]` in
`tsv_wasm`; std's default dlmalloc before the swap), whose per-call cost profile
differs from native glibc — so an allocation-count win can move WASM format time
even when the same change is a wash on native. The full `deno task bench` is too coarse to
see those single-digit-% moves; `benches/js/diagnostics/wasm_format_probe.ts` resolves
them.

It applies the §5 paired discipline in a single invocation: interleaved pairs
alternate which build runs first, the A/A noise floor is measured in the *same*
run (a floor from a separate run is untrustworthy — a rebuild between runs shifts
CPU frequency/thermals ~10%, dwarfing a ~1% signal), and it reports `net = A/B ÷
floor` plus the A/B `[min,max]` spread so a noisy median is visible. A corpus
byte-identity check gates it — a no-behavior-change edit must format every file
identically across the builds, or the run aborts.

```bash
# copy the artifact aside before editing (pkg/ is gitignored)
cp -r crates/tsv_wasm/pkg/all/deno crates/tsv_wasm/pkg/all/deno.baseline
# ... edit source, then rebuild and A/B:
deno task build:wasm:all:deno
deno run --allow-read --allow-env --allow-net --allow-sys \
  benches/js/diagnostics/wasm_format_probe.ts \
  --baseline crates/tsv_wasm/pkg/all/deno.baseline/tsv_wasm.js
```

Defaults to `../zzz/src/lib` (the corpus the native profiling tools use, for
comparability); pass a directory to override, or `--lang`, `--pairs`, `--warmup`,
`--control` (a separate identical-code copy for a two-instance floor).

Omit `--baseline` for an **A/A-only run**: no comparison, just the current
build's per-language wall time and the noise floor (`floor` ≈ 1.00). It's the
cheapest way to sanity-check the floor and capture a fresh baseline number
before starting an A/B.

```bash
deno run --allow-read --allow-env --allow-net --allow-sys \
  benches/js/diagnostics/wasm_format_probe.ts
```

The memory-axis sibling, `benches/js/diagnostics/wasm_memory_probe.ts`,
measures WASM peak/high-water memory demand per file (documented in
`benches/js/CLAUDE.md`).

### 7. `tsv_debug arena_stats` — doc-arena node population

Formats a corpus into fresh `DocArena`s and walks `borrow_nodes()`, reporting the
memory shape of the doc IR: **nodes/byte** (actual vs the `with_source_size_hint`
2/byte pre-size) with **per-file density percentiles** (p50/p90/p95/p99/max — what a
safe hint must clear), **capacity fill %** (used vs reserved node slots), the **DocNode
variant histogram** (which node kind dominates the `Vec` the render/`fits`/build
loops linearly scan), and the **DocText sub-histogram** (`Static` / `Pooled` /
`SourceSpan` share of `Text`). `--reuse` instead reports the
**`reset()`-reuse high-water** — the peak retained node/children capacity across one
shared arena (as the CLI/FFI/WASM batch drivers use), the number that shows a lower
pre-size hint doesn't grow the batch footprint (it's bounded by actual max-file usage,
not the hint). The static, load-independent counterpart to the timing/allocation
tools — "what is the arena made of and how over-reserved is it" rather than "where
does the time go". Pure Rust, no Deno; covers `.ts` / `.svelte.ts` / `.svelte` / `.css`.

It also reports **container degeneracy** (empty/single/nested `Concat`/`Fill` — the
node-count lever) and audits the sibling pre-sizes (output `String`, AST bump). For
those two it prints per-file calibration distributions: **output/node** (the multiplier
`estimated_output_capacity = k · nodes.len()` must clear at its percentile so the dense
tail doesn't realloc) and **bump demand/byte** (an *un-pre-sized* `Bump::new()`'s
`allocated_bytes()` per source byte — the AST's byte demand, since the production
`bump_allocated` figure is dominated by the pre-size, not demand; note bumpalo never
copies on chunk growth, so the bump pre-size is a malloc-count/peak knob, not a
memcpy-churn one). `--list-errors` prints the path + parse error for every file the walk
skips — the fast native first pass for finding tsv parse over-rejections (a file the
canonical parser accepts but tsv rejects is a real gap; most corpus rejects are
intentional-error test fixtures the canonical parser also rejects).

```bash
cargo run -p tsv_debug arena_stats ~/dev/zzz/src/lib ~/dev/fuz_css/src/lib
cargo run -p tsv_debug arena_stats <paths> --json
cargo run -p tsv_debug arena_stats <paths> --reuse         # reset()-reuse high-water
cargo run -p tsv_debug arena_stats <paths> --list-errors   # list parse-skipped files
```

### 8. `tsv_debug buffer_sizes` — printer buffer sizing

Histograms for tuning the TS printer's SmallVec inline capacities. Two parse-time
metrics (static AST properties): named-import-specifier count per import
(`named_specs`), and line count per multi-line block comment (the population the
parked line-offset scratch iterates). With `--features buffer_stats` (off by
default — the record hooks sit in the chain printer's hot path), each file is also
*formatted* and four printer-buffer populations are sampled at their construction
chokepoints (`tsv_ts::printer::buffer_stats`), so inline-`N` claims are measured
data, not doc-comment prose: `ChainNodeVec` (nodes per linearized chain),
`ChainGroupVec` (groups per `group_chain_nodes` call), `ChainGroup.nodes` (nodes
per built group), and the leading-comment `CommentVec`
(per `collect_leading_comments` call — the type's dominant site). Covers
`.ts`/`.svelte.ts` AND `.svelte` (the `<script>`/`{expr}` feed the same TS-printer
buffers). Prints percentiles + spill rate at candidate inline N. For sizing,
exclude the prettier/svelte test suites (edge-case skew). Pure Rust, no Deno.

```bash
cargo run -p tsv_debug buffer_sizes ~/dev/zzz/src ~/dev/gro/src
cargo run -p tsv_debug --features buffer_stats buffer_sizes <paths>  # + chain/comment histograms
cargo run -p tsv_debug buffer_sizes <paths> --json
```

## Measurement Process

### Before an optimization

1. **`tsv_debug profile`** on the target workload — note the phase split
2. **`perf report --stdio`** — identify which functions are hot
3. **Record baseline** with corpus benchmarks: `deno task bench:perf`

### After an optimization

1. **`tsv_debug profile`** — same workload, compare phase split
2. **`deno task bench:perf`** — measure overall corpus impact (perf surface;
   the full `deno task bench` also runs the node conformance coverage surface — a
   pre-flight parse-coverage pass, no timed phase)
3. **Record results** — for regression detection, use `deno task bench:deno:run -- --save-baseline` / `-- --compare-baseline` (or the `bench:node:run` / `bench:bun:run` siblings for the other runtimes)

### Before optimizing a scan, print what it finds

A hot scan is not necessarily a scan that is *doing* anything. Stamp a throwaway
histogram of its input before designing the fix — length distribution, and how
often each branch it pays for actually fires. Two of the largest small wins on
record came straight out of one:

- The doc engine's per-text-node width probe ran three searchers looking for a
  newline, a non-ASCII byte, and tabs. Across every corpus measured, its input
  held **no newlines and (in CSS) no tabs at all**, and was ≤31 bytes over 99% of
  the time — three setups, each paid regardless of length, finding nothing on a
  ~6-byte string.
- A CSS declaration's value span arrives from the declaration scanner **already
  trimmed** (0 leading and 0 trailing whitespace bytes across 200K real
  declarations), so the trimming that recovered its offsets was computing zero.

The histogram also sizes the fix. Fusing N scans into one byte pass is a
**short-string** lever: on a long slice the searchers are SIMD and a plain byte
count auto-vectorizes, so three vector passes beat one scalar walk. Gate the
fused path on length and let the tail keep the vectorized shape — an ungated
fusion won on CSS and *regressed* pure TS, whose text nodes run longer.

### A corpus cannot grade arithmetic

**Before trusting a green corpus diff, ask what it can physically see.** A width,
offset, span or count only changes the output once it crosses a threshold (the
print width, a line break), so an arithmetic error on a rare byte leaves every
formatted file byte-identical. This is not hypothetical: corrupting the doc
engine's text-width tab arm by a single column passes the **fixture suite**, an
**11,696-file format diff**, and an **11,696-file wire diff**, and is caught only
by the exhaustive equivalence test that sits beside the function.

So a numeric change ships with an **equivalence test at its declaration**, graded
against the shape it replaced, over inputs chosen to hit every branch (the corpus
will not) — then **corrupt it and watch the test fail**. An oracle you have never
seen fail proves nothing. See the same rule applied to CSS keyword sets in
[`crates/tsv_css/CLAUDE.md`](../crates/tsv_css/CLAUDE.md), and to text width in
[`crates/tsv_lang/CLAUDE.md`](../crates/tsv_lang/CLAUDE.md).

Two harness rules that fall out of the same skepticism, both of which have faked
a result here:

- **Self-check any differential A-vs-A first.** Running the baseline binary
  against itself must report zero. It is how a diff harness that fed files through
  a shell `$(cat …)` — silently dropping null bytes and the trailing newline — was
  caught.
- **Build each placement variant into its own `CARGO_TARGET_DIR`, and hash the
  binaries.** Building a `codegen-units=16` variant inside the baseline's checkout
  overwrites its `target/`, after which a "cu1" run silently compares cu16-baseline
  against cu1-candidate. A build that finishes suspiciously fast is the tell.

## WASM bundle size

The `tsv_wasm` crate produces three WASM binaries via the `format` +
`parse` cargo features, each published as a separate npm package:

- `--no-default-features --features format` → `@fuzdev/tsv_format_wasm` (format only)
- `--no-default-features --features parse` → `@fuzdev/tsv_parse_wasm` (parse only)
- default build (both) → `@fuzdev/tsv_wasm` (full tool + `tsv` bin)

`binary_sizes.ts` in the bench runner reads the three
`pkg/<variant>/deno/tsv_wasm_bg.wasm` files and reports them side-by-side, with
gzipped wire size alongside raw on-disk size; current numbers land in the bench
report (`benches/js/results/report.<runtime>.md`).

Gzipped numbers come from `gzip -c` (system default level 6), matching
npm-tarball wire reality and `scripts/patch_npm_package.ts`. The parse feature
adds the wire-JSON writer (which fuses in the byte→char offset translation); the
format feature adds the printers (which the parse-only build drops at link
time); the AST crosses the JS boundary as a JSON string handed to the engine's
native `JSON.parse` (no `serde_wasm_bindgen`). All builds run wasm-opt with
explicit bulk-memory + nontrapping-float-to-int flags.

Build all three before running benches so the sizes appear in the report:

```bash
deno task build:wasm:deno         # format-only → pkg/format/deno/
deno task build:wasm:parse:deno   # parse-only → pkg/parse/deno/
deno task build:wasm:all:deno     # full (executed by the bench) → pkg/all/deno/
```

## Future tools (reach for when needed)

These aren't set up yet but may be useful for specific investigations:

- **Criterion microbenchmarks** — statistical rigor for isolated hot functions
- **Custom counters** — `fits()` call counts (when investigating algorithmic
  issues; doc-node counts are already covered by `arena_stats`, §7)

## Baselines and tracking

Methodology and tooling above are evergreen; corpus benchmark results land in
the per-runtime `benches/js/results/report.<runtime>.{json,md}` siblings
(`report.deno.*` / `report.node.*` / `report.bun.*`).

Wall-clock readings vary several-fold with machine state (CPU frequency scaling
and concurrent load) — trust only quiet-machine runs, and prefer per-byte rates
and relative profile shares as the portable metrics. Because the corpus changes
over time, compare per-byte rates rather than wall totals across runs.
