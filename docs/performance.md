# Performance

Profiling methodology and tracking for the TypeScript/Svelte/CSS formatter.

**Goal:** Identify where time is spent, make targeted improvements, and measure before/after.

## Formatter Pipeline

```
source → Parse → AST → Format → formatted string
         lexer    │      per-statement:
         parser   │        build_statement_doc() → DocId (arena-allocated)
                  │        write_arena_doc() → arena_print_doc_with_indent_resolved_into()
                  │          └── arena_fits() (line-breaking decisions)
                  │
              tsv_ts::parse()                      tsv_ts::format()
```

Doc building and rendering are **interleaved** per-statement inside `format()` — each statement's Doc is built as arena-allocated `DocId` nodes and immediately rendered. This means the cleanest measurable phase split is **parse vs format**. Within format, `perf` can break down time further by function.

**Key files:**

- Parse — `tsv_ts` — `parser::parse_typescript()`
- Format (orchestration) — `tsv_ts` — `printer::Printer::print_program()`
- Doc building — `tsv_ts` — `printer::Printer::build_statement_doc()` → `DocId`
- Doc rendering — `tsv_lang` — `doc::arena_render::arena_print_doc_with_indent_resolved_into()`
- Line-break decisions — `tsv_lang` — `doc::arena_fits::arena_fits()`

## Measurement corpora

Comment- and allocation-path work is corpus-sensitive — comment density alone
varies by an order of magnitude across the trees below — so pick the corpus to
match what you're measuring, and **run a SmallVec-sizing histogram and the
heaptrack that validates it on the _same_ corpus.** Gate on the measured
alloc/wall delta, never a static spill rate: a high spill *rate* over a small
*population* (comment-collect spills are a fraction of a percent of all
allocations) is a negligible absolute change.

- **Headline rate / profile** — `../zzz/src/lib`. Typical app code,
  comment-sparse; the per-byte baseline the tables here track.
- **Comment- / alloc-dense stress** — `../fuz_app/src/lib`. TSDoc-dense
  library code; the extreme for comment-path and allocation changes (zzz's
  comment density is a fraction of fuz_app's, so zzz alone under-represents
  these paths).
- **Svelte-component-dense** — `../fuz_ui/src/lib`. Mostly `.svelte`
  components with a thin `.ts` slice — the markup-heavy complement to
  fuz_app's TSDoc-dense TS, and a stable in-ecosystem stand-in for the
  external `.svelte` slices below.
- **Representative real-world** — `../svelte/packages/svelte/src`,
  `../kit/packages/kit/src`, and `../svelte-docinfo/src`. Large, diverse
  sources at moderate comment density — the middle ground the two app corpora
  bracket. svelte and kit are mostly `.js`, which
  tsv formats like the rest of the JS/TS family (parsed as TypeScript), so all
  three sources are formattable end to end.

**Measuring one language in isolation:** because `profile`/`json_profile` route
every non-`.svelte`/`.css` file to the TypeScript parser, a directory that
co-locates other files with the language under test pollutes that language's
rate — e.g. `../prettier/tests/format/css` holds per-directory `.js` test
drivers beside its `.css` fixtures. Copy only the target extension into a
scratch directory and profile that.

**There is no CSS corpus in the list above, and the obvious guess is a trap.**
`../fuz_css/src` is a CSS *framework*, but by bytes it is ~92% TypeScript —
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

The tools, in order of use:

### 1. `tsv_debug profile` — phase timing

Measures parse vs format timing across files. Pure Rust, no external dependencies.
The `--bind` form instead measures parse vs lower+bind timing through the
`tsv_check` crate (the experimental typechecker, which may never ship — see
[typechecker.md](typechecker.md); TypeScript files only) and reports peak RSS
(`VmHWM` from `/proc/self/status`) — the binder's standing perf-anchor form;
add `--flow-stats` for its deterministic flow-construction counters.

```bash
# Profile a directory
cargo run --release -p tsv_debug -- profile ../zzz/src/lib

# Profile specific files
cargo run --release -p tsv_debug -- profile file1.ts file2.svelte

# More iterations for stability (default: 10)
cargo run --release -p tsv_debug -- profile ../zzz/src/lib --iterations 20

# JSON output for scripting
cargo run --release -p tsv_debug -- profile ../zzz/src/lib --json
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
cargo run --release -p tsv_debug -- json_profile ../zzz/src/lib

# JSON output with per-file data (e.g. to split costs by multibyte flag)
cargo run --release -p tsv_debug -- json_profile ../zzz/src/lib --json
# Also: --iterations <n> (default: 5)
```

Output shows, per language: file/byte/wire-byte/multibyte counts and the
`parse` and `write` medians (sums of per-file medians).

**When A/B-ing a write-path change, read `write` from here and `parse` from
`profile` (§1) — not from this command.** Both phases run in one process against
one allocator, so a change that alters what the writer allocates also changes the
state the *next* iteration's `parse` starts from; that alone can move this
command's `parse` median by a few percent in either direction while the parse
code is untouched. `profile` (§1) never calls `convert_ast_json_bytes` at all,
which makes it both the trustworthy parse surface and the **null control** for a
write-path change: a genuine write-path win leaves its totals flat.

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
perf record --call-graph=dwarf -- target/profiling/tsv_debug profile ../zzz/src/lib

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
name (e.g. `render_doc_core`, instantiated per `RenderPolicy`).
For those, dump everything and search by source line instead:

```bash
perf annotate --stdio > annotate.txt   # then search for the fn's source lines
```

⚠️ **`annotate` has two more silent-empty modes, and neither says why.** A
recording made with `--call-graph=dwarf` annotates empty — record *without* it
when the goal is line-level attribution (keep the dwarf recording separately for
callchain work). And `-s <exact name>` can still return nothing where an
unfiltered dump annotates that same symbol fine. **When `-s` is empty, dump
everything before concluding anything about the symbol** — an empty `-s` is
evidence about `perf`, not about the binary:

```bash
perf record -q -F 4000 -o flat.data -- <workload>       # no --call-graph
perf annotate -i flat.data --stdio > annotate.txt       # then slice the symbol out
```

`perf annotate` may segfault after writing usable output; the already-written
portion is still valid, so redirect to a file rather than piping to a reader.

**A name on the board is not necessarily a function.** With debug info present,
`perf` attributes samples to *inlined* frames too, so a symbol can top the report
while having no out-of-line copy anywhere in the binary — its cost is the work of
one source site, scattered across every caller. `perf annotate -s` returning
nothing is the tell; `nm` is the confirmation:

```bash
nm -C target/release/tsv_debug | grep node_header_impl   # empty ⇒ fully inlined
```

This changes what a fix can even look like: an inline-attributed leaf's cost is
one source site's work scattered across its callers — a "take that function
apart" lead is unbuildable as stated. Source-line attribution says what the cost
actually is (the worked case — the wire writer's per-node header, whose 16%
share turned out to be `Vec` append bookkeeping — continues in
§An `inline(never)` leaf's real cost below):

```bash
perf report --stdio --no-children -q -s srcline -g none   # flat, by source line
perf report --stdio --no-children -q -s srcfile,srcline   # + per-line callchains
```

⚠️ **An inlining verdict is a fact about one build, and it expires.** That same
leaf is out-of-line a few commits later, with `perf annotate` working on it
directly — nothing about it changed, the surrounding code did. Inlining, symbol
presence and monomorphization counts are all emergent from whole-program codegen,
so **re-run `nm` against the binary in front of you** rather than trusting a
recorded finding. The method is durable; the finding is not.

⚠️ Only **basenames** are printed, so std's `alloc/src/vec/mod.rs` and a crate's
own `mod.rs` are indistinguishable in the flat view — read a line's callchain
(drop `-g none`) to tell them apart.

⚠️ Board **shares** move when only the denominator does. That same leaf rose
15.97% → 16.63% across a change that made *other* things faster and left it
untouched. Compare absolute instruction or cycle counts across boards, never
percentages.

**Telling real work from a code-layout artifact — `perf stat`.** When a logically
tiny change moves the native wall, count instructions before chasing a function:
**instruction count is layout-independent** (code placement cannot change how many
instructions run), so it separates real added work from a frontend/i-cache effect.

```bash
# Deterministic counts (±0.00% across -r runs); compare two binaries, one workload
perf stat -r 4 -e instructions,cycles,branches,branch-misses \
  target/profiling/tsv_debug profile ../fuz_app/src/lib --iterations 30
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
cargo flamegraph --profile profiling -p tsv_debug -- profile ../zzz/src/lib
```

On Debian, `perf` ships in the `linux-perf` package (there is no package named
`perf`), and unprivileged profiling additionally requires
`kernel.perf_event_paranoid <= 2` — Debian patches the kernel default to 3,
which blocks unprivileged perf entirely:

```bash
sudo apt install linux-perf
sudo sysctl kernel.perf_event_paranoid=2  # persist via a drop-in in /etc/sysctl.d/
```


### 5. `heaptrack` — allocation-site profiling

When `perf` shows time inside malloc/free internals, it can't say _which_
allocation sites are responsible — glibc's allocator is diffuse from the CPU
side. `heaptrack` attributes every allocation to its call site, answering
"swap the allocator" vs "fix the hot sites" (the AST-allocation design it
validated:
[architecture.md §Nested AST](./architecture.md#nested-ast-bump-arena-not-flatindexed)).

```bash
# Record (build with the profiling profile for symbols)
cargo build --profile profiling -p tsv_debug
heaptrack -o /tmp/heaptrack_tsv target/profiling/tsv_debug profile ../zzz/src/lib --iterations 2

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
  target/profiling/tsv_debug profile ../zzz/src/lib --iterations 20 --json
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

Two siblings cover the other WASM axes: `wasm_memory_probe.ts` measures
peak/high-water linear-memory demand per file (the axis the wall probe can't
see), and `wasm_json_probe.ts` attributes the WASM-vs-native JSON *parse*
penalty (parse vs materialization vs the JS-side `JSON.parse`). Both are
documented in `benches/js/CLAUDE.md`.

### 7. `tsv_debug arena_stats` — doc-arena node population

Formats a corpus into fresh `DocArena`s and walks `borrow_nodes()`, reporting the
memory shape of the doc IR: **nodes/byte** (actual vs the `with_source_size_hint`
2/byte pre-size) with **per-file density percentiles** (p50/p90/p95/p99/max — what a
safe hint must clear), **capacity fill %** (used vs reserved node slots), the **DocNode
variant histogram** (which node kind dominates the `Vec` the render/`fits`/build
loops linearly scan), and the **DocText sub-histogram** (`Static` / `Pooled` /
`SourceSpan` / `VerbatimSpan` share of `Text`). `--reuse` instead reports the
**`reset()`-reuse high-water** — the peak retained node/children capacity across one
shared arena (as the CLI/FFI/NAPI/WASM batch drivers use), the number that shows a lower
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
cargo run -p tsv_debug arena_stats ../zzz/src/lib ../fuz_css/src/lib
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
cargo run -p tsv_debug buffer_sizes ../zzz/src ../gro/src
cargo run -p tsv_debug --features buffer_stats buffer_sizes <paths>  # + chain/comment histograms
cargo run -p tsv_debug buffer_sizes <paths> --json
```

### 9. `tsv_debug compile_profile` — Svelte compile against the format wall

Times the Svelte compiler (`tsv_svelte_compile::compile`) per file and reports it
as a **ratio** against parsing plus formatting the same file. The ratio is the
point: it says how many times the format wall a compile costs, which is the cheap
tripwire for super-linear or rebuilt work in the compile pipeline. The design
frame is ~2–3× for an all-linear pipeline.

The two rows deliberately keep different shapes, so the number means what it says:
`compile` is the whole **cold per-call** cost (the compiler has no warm arena-reuse
entry point, so that *is* its production shape), while `parse + format` uses warm
`reset()`-reuse arenas (the `tsv_cli` shape). **Compare ratios only against ratios
from this same command** — never against a raw timing from `profile`.

Refusals and parse failures are counted, not timed. A `CorruptOutput` or a
`TypeErasureLeak` is a compiler bug and fails the run. Pure Rust, no Deno; run with
`--release` for anchors.

```bash
cargo run --release -p tsv_debug -- compile_profile tests/fixtures_compile
cargo run --release -p tsv_debug -- compile_profile ../svelte/packages/svelte/tests/runtime-runes
cargo run --release -p tsv_debug -- compile_profile <paths> --iterations 20 --json
```

Because the ratio is relative to the corpus it ran on, a compiled corpus that
*grows* (a refusal class getting unlocked) reshapes the denominator — so an anchor
is only comparable to another anchor over the same corpus.

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
4. **Check the size axis if the change shrank a hot function** — see
   [An instruction A/B is blind to code size](#an-instruction-ab-is-blind-to-code-size); no `check` gate covers it

### Grading a change that touches the `format` worker pool

Two traps specific to the parallel path, both of which produce confident wrong numbers.

**Measure the pool, not the process.** Timing `tsv format` end to end folds in process
startup and the machine's mood. Instrument the pool itself — its makespan, each worker's
busy time, and `idle = makespan × workers − total_busy`. Beyond removing the startup floor,
`total_busy` makes visible the assumption every scheduling model rests on: that per-file cost
is independent of the order and width you run at. It is not. Formatting the largest files
concurrently raises the cost of each of them, so a change that improves the *schedule* can
still lose on the wall.

**The wall is topology-sensitive, and `taskset` lets one machine stand in for several.** SMT
siblings are paired (`/sys/devices/system/cpu/cpuN/topology/thread_siblings_list` reads `0-1`,
`2-3`, …), so masking to one CPU per pair gives a no-SMT machine and masking fewer pairs gives a
smaller one — no root needed. `available_parallelism()` reads `sched_getaffinity`, so tsv's own
default worker count follows the mask; confirm that before trusting a sweep (under `-c 0,2` the
default run should match `--jobs 2`). Score a candidate default by its **worst case across
corpora**, not its best: the optimum width differs by repo shape — a large tree where discovery
dominates wants fewer workers than a flat repo of the same file count, because the walk thread is
then competing with them for cores.

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
seen fail proves nothing.

**The same rule covers scans, with one extra axis: alignment.** A word-at-a-time
rewrite of a byte scan fails on *where the pattern lands relative to the stride*,
which a corpus samples arbitrarily. So grade a scan over every input length and
**every alignment across the word stride** — the line-start scans are checked
against the byte-at-a-time shapes they replaced over every string of length 0–4
on an alphabet covering each arm (`\n`, `\r`, `\0`, ordinary, `0x7f`) × every
alignment 0–16. Note what no corpus could have covered: real source contains no
`\r` at all, so the entire CRLF arm is corpus-dead. Both corruptions tried
(dropping the scalar tail; reading the highest set lane of the SWAR mask instead
of the lowest) were caught only there. See the same rule applied to CSS keyword sets in
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

### An instruction A/B is blind to code size

**A lever that makes a hot leaf smaller or simpler gets inlined at more call
sites, and pays for itself in bytes at every one of them.** The instruction
counter cannot see that, and neither can any gate in `deno task check` — the size
bounds live in `scripts/validate_artifacts.ts` (the WASM bundles, at `deno task
publish` Step 6) and `deno task validate:napi` (the native artifacts, in the
tag-triggered release workflow), not in `check`.

The worked case: rewriting the wire writer's integer emitter to end in a
fixed-width copy instead of a runtime-length one removed a libc `memmove` call
and measured a clean **−3.2% parse-product instructions**. It also grew
`@fuzdev/tsv_parse_wasm` **+6% raw, past its `max` bound** — the smaller body was
now inlined at ~200 writer call sites, each carrying the fixed-width blit.
`cargo test --workspace`, `deno task check`, an 8,257-file byte-identity diff and
the instruction A/B were **all green through the regression**.

So a "make a hot leaf smaller" change ships with the size axis measured too:

```bash
deno task build:wasm:parse:deno && deno task build:wasm:deno
# compare raw bytes against BOUNDS in scripts/validate_artifacts.ts
```

Two rules fall out:

- **Measure HEAD's bundle too, not just the bound.** A bound can already sit near
  its limit from unrelated work, and attributing that to your change wastes a
  session (the mirror of the general "verify HEAD is green before blaming
  yourself" rule).
- **Reach for `#[inline(never)]` before abandoning the win.** Where the win comes
  from work removed *inside* a function body, keeping the body out-of-line costs
  nothing — the caller was already paying the call. In the case above it kept the
  entire instruction win and left both bundles *smaller* than baseline. ⚠️ The
  converse also happens: where the win *is* the inlining (see the reload tax
  below), out-of-lining costs the whole thing, and the size growth is the price
  of the lever rather than an accident to optimize away.

**Recentering the bounds is a deliberate act with a rule.** `BOUNDS` in
`scripts/validate_artifacts.ts` is ±8% around a measured value, and `DELTAS`
(`all − format` = the parse feature, `all − parse` = the format feature) the
same. When a size change is accepted, **recenter every variant on freshly
measured values rather than raising the one ceiling that failed** — a band left
stale goes asymmetric as unrelated work accumulates, and an asymmetric band both
false-positives on ordinary growth and stops catching a suspicious *shrink*,
which is the half of the check with no other tripwire. Watch the deltas
especially: they are the tighter constraint in practice, and a delta failure
reads as "a feature gate broke", which is a confusing way to learn that a bundle
merely grew. Record the new measurements and the date in the comment above the
constants, and note that `format` cannot move on a convert-path change at all
(it builds without the `json` feature) — if it did, something else is going on.

### An instruction A/B is blind to stalls and to store traffic

The companion to the section above, and the reason a lever must be graded on the
counter that matches what it *moves*:

| what the change moves | counter that can see it |
| --- | --- |
| work (fewer operations) | `instructions:u` |
| bytes, alignment, access width or ordering | `cycles:u` |
| code size | the WASM bounds (see above) |

The worked case is the direct sequel to the fixed-width-copy change described
above. That copy loaded 16 bytes out of a stack scratch which the digit loop had
just filled with **2-byte stores** — a narrow-store → wide-load pattern that
**never store-forwards** — and it wrote the scratch's full width (20 bytes to
keep ~3). One store instruction ended up carrying **71% of that function's
self-time, ~22% of the whole parse→JSON run.** Packing the digits into a register
and appending one word instead measured **−7.3..−7.7% cycles across four
corpora with instructions FLAT (−0.06..−0.25%)** — a win an instruction-only A/B
scores as noise and discards.

Two practical rules:

- **`perf annotate` before theorizing about a hot leaf.** A percentage tells you
  *where*; only the per-instruction breakdown tells you the cost is one store
  rather than the arithmetic around it.
- **Cycles are far noisier than instructions** — on the reference machine the
  same binary pair reads a ±2% band run to run, and a format-path control drifted
  2.2–3.3 G for identical work. Use **best-of-N interleaved** (minimum cycles is
  the least contaminated estimator), never a single pair, and keep a control
  corpus the lever's code never runs.

### A per-site precondition is only as cheap as its fold

A question asked once per construct — per declaration, per selector, per
combinator, per comma — is a per-site *tax*, and the way to retire it is a
document-level precondition that answers for all of them at once. The CSS
boundary-whitespace claims are the worked case: every member of that class is
non-ASCII ([`printer/boundary_ws.rs`](../crates/tsv_css/src/printer/boundary_ws.rs)),
so a source with none anywhere cannot have one in any gap of it, and one scan of
the source retires every claim in the document.

**The precondition is not the expensive half — its fold is.** Wall figures below
are against the same tree with the claim family absent, three binaries
interleaved in one session (8 ABBA rounds, per-round paired, cv < 0.6%); the
instruction column is the layout-free one:

| build | print | total | instructions (191 KB corpus, 200 iterations) |
| --- | --- | --- | --- |
| claim family, no precondition | +8.5..+10.7% | +5.0..+7.0% | 8.677 G |
| precondition as a plain `bool` field read | −3.2%* | — | 8.504 G |
| the same read, gate `#[inline]` + scan out of line | +1.7..+3.1% | +0.7..+2.3% | 8.414 G |
| claim entry points stubbed to a constant | +2.3%* | — | 8.321 G |
| claim family absent entirely | 0 | 0 | 8.240 G |

\* measured against the ungated build rather than against the absent one; the
column is a ladder, not one run. The wall ranges span two `.css` corpora
(191 KB / 516 KB) and several sessions — see the next section for why they are
this wide while the instruction column is not.

The first and third rows differ **only** in where the branch lives. With the
whole function out of line, every site still pays a call, a returned `String` to
construct and drop, and an `is_empty` the caller cannot see through — barely
half the win. Splitting each claim into an `#[inline]` gate over an out-of-line
scan recovers most of the rest. `#[inline(always)]` on top of that measured
**zero** further change (8.4276 G vs 8.4275 G): once the branch folds, nothing
is left to fold.

The last two rows are the standing residual, and worth naming because it is
**not** recoverable at this design: a runtime flag cannot delete the branch or
the empty `String` the way a compile-time constant can, and the whole-source
scan itself costs ~0.3% of the CSS print phase. Roughly ~2% of that phase is
therefore intrinsic to having the claims at all, and the remaining ~2.3% belongs
to the *other* work in that family — the parser's own boundary skipping, the
blank-line rule, the declaration-tail scan — not to the claims. **A CSS baseline
comparison that reaches back past this family should expect ~+2% print and read
it as paid-for correctness, not as a fresh regression.**

Two rules fall out:

- **Grade a precondition on the counter that shows the fold.** `instructions:u`
  separates the three rows above cleanly; the wall clock on the middle row was
  contaminated (next section).
- **A cheap scan can look expensive when it is attributed by subtraction.**
  Reading the first fold's shortfall as "the document scan must cost 5%" was
  wrong by an order of magnitude — `perf report` put the scan at 0.14% of the
  run, inlined into the printer's constructor. Attribute with the profile, not
  with the difference between two other numbers.

### A phase column is a control only for code the other phase never runs

`tsv_debug profile` splits parse from format, which invites reading the parse
column as a free control for a printer change. It is not one. A CSS-printer-only
change repeatedly moved the **CSS parse** column by −2 to −4% across rebuilds —
impossible as work, and reproducible within a build (cv < 0.6%, consistent in
every one of 8 interleaved rounds). Changing the printer's inlining moves code
the parser shares a binary with; one build's parser lands better than another's,
and a phase column that runs alternately with the changed phase inherits its
i-cache footprint too.

The drift is also **per session**, not just per build: the same three binaries
read `old → head` parse at +1.6%, +2.7% and +4.1% in three sittings, while the
print column held +8.5..+9.6% across all of them. `total` inherits whichever way
the parse column happened to land, so it is not the safe headline either.

What to do about it:

- **Keep the control in a different language.** A pure-`.ts` corpus (§Measurement
  corpora) exercises none of the CSS path — and it makes the artifact visible in
  the other direction, since its *own* parse column moved +2.8..+4.1% for all
  three CSS binaries, none of which touch `tsv_ts`. Its `total` read +0.05%,
  which is what makes the CSS number attributable.
- **Interleave every binary in ONE session, against a common baseline.** A
  two-way A/B run twice cannot tell a session's drift from the change; a
  three-way rotation measures both deltas against the same round of the same
  baseline, and the drift cancels.
- **Headline the phase the change is in**, and say what the neighbouring column
  did rather than folding it in silently.
- **Distrust a phase delta the change cannot explain, in either direction.** A
  parse column that *improves* on a printer-only change is the same artifact as
  one that regresses, and taking the flattering half is how a layout win gets
  banked as an optimization.

### An `inline(never)` leaf's real cost is paid by its caller

`#[inline(never)]` on a hot leaf is often correct — it is how the fixed-width
copy above kept its win without blowing a WASM bound. But the call is not the
whole price. **An opaque call de-registerizes the caller's state around every
call site**: anything the callee might touch has to be re-loaded from memory
afterwards. When such a leaf is invoked repeatedly *between* other operations on
the same object, that reload tax is charged to the surrounding code, where no
profile line attributes it to the leaf.

The worked case is the node header. It emitted 16 `Vec` appends per AST node, six
of them out-of-line integer calls. The obvious model — "an append is a capacity
check plus a store, call it three instructions" — makes 16 appends look cheap
against any alternative that ends in a `memmove`. That model was wrong by a large
factor, because each opaque integer call forced the *following* append to re-load
the buffer's pointer, length and capacity. Assembling the header in a
writer-owned scratch and appending it **once** measured **−8.2..−10.0%
instructions and −5.7..−6.6% cycles across four corpora**.

The instructive part is the ledger, not the headline:

| | before | after | delta |
| --- | --- | --- | --- |
| run cycles | 3158M | 2944M | **−214M** |
| libc (the single flush `memmove`) | 219M | 399M | +180M |
| everything else | 2939M | 2545M | −395M |

**The memmove cost exactly what the pessimistic prediction said it would** —
libc nearly doubled. The prediction failed on the *benefit* side. So:

- **When a lever is predicted to lose, check whether the prediction priced the
  benefit or only the cost.** A correct cost estimate is not a correct verdict.
- **Read the instruction stream, not just the source-line board.** The reload tax
  is visible as the object's length/pointer reappearing in a `mov` between
  fragments that should have kept it in a register; no source line names it.
- Staging is the shape that buys both: the leaf inlines at **one** site (so the
  code-size bound survives) while the destination becomes a fixed-base,
  constant-bounded, register-resident cursor. Keep the scratch a **struct field,
  not a local** — a per-call `[0; N]` is a memset LLVM cannot prove dead when
  only a dynamic prefix is read, and that alone can eat the win.

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

Two failure modes that pass a CPU-idle check: a run started within minutes of a
long all-core compile can read 20–30% slow on a laptop even with the governor at
`performance` and nothing else running (package heat clamps sustained boost, and
recovery is minutes, not seconds); and two sessions' "quiet machines" are not
the same machine (recorded anchors have disagreed ~5% with a same-binary rerun).
So a cross-session anchor comparison is a *hypothesis generator only* — any
regression or win claim needs an interleaved same-session A/B of two binaries,
which cancels both effects.
