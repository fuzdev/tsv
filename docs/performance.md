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
*regression* gets missed).

**Build a genuine `.css`-only corpus, from authored CSS, in four steps.** This is
the recipe that produced the ~1.1 MB / 638-file corpus the CSS numbers below are
taken on — enough to hold a sub-percent read steady:

1. **Extract every `<style>` body from a `.svelte` corpus.** This is where the
   ecosystem's CSS actually lives, and the size is itself worth knowing: 1,695
   components yield **560 blocks totalling only 265 KB**, so real app CSS is a
   small fraction even of a Svelte repo. (Skip a body containing `${` — that is a
   `<style>` inside a JS template literal, not CSS.)
2. **Add every standalone `.css` that `tsv format --list` finds** across the
   ecosystem repos — the product's own gitignore-aware scope rule, never a bare
   `find`, which pulls in minified build bundles.
3. **Add `benches/js/.cache/svelte_styles/`** (`deno task
   bench:harvest:svelte-styles`), whose per-repo concatenations cover repos that
   may not be checked out.
4. **Add vendored-but-authored stylesheets** — icon fonts, map widgets,
   `fuz_css/dist/{theme,style}.css`.

⚠️ **Three exclusions, each of which measurably distorts the result.**
**(a) `*.min.css`**: a minified bundle is comment-free and newline-free, so one
file can dominate a 1 MB corpus and re-weight every share on the board — the same
mechanism by which a handful of hashed SvelteKit bundles once made a lever read
four times too small. **(b) The spec and test checkouts** (`../csswg-drafts`,
`../wpt`): `../wpt` alone contributes ~390 files of CSS test data, and it is the
*same* data as the `wpt_css` grading cache, so a board built from it double-counts
the corpus the change is graded against. **(c) `tests/` inside this repo**, which
holds encoding and charset fixture data rather than authored CSS.

`benches/js/.cache/wpt_css` remains the right corpus for **grammar breadth and
byte-identity sweeps** — 22,310 files — but not for a board: at a 246-byte median
it measures per-file setup rather than the printer.

For attribution in the other direction, a **pure-`.ts`** corpus (no `.svelte` — a
`.svelte` file's `<style>` block routes through the CSS parser) is the control
that must read ~0.000% for a CSS-only change.

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
perf annotate --stdio -s 'tsv_lang::doc::arena::DocArena::subtree_layout_fill'

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

⭐ **For "which lines of THIS function are hot", prefer `perf report` over
`annotate` entirely.** It answers in source lines rather than instruction
addresses, needs no multi-megabyte dump and no slicing, and does not crash:

```bash
perf report --stdio -q -g none \
  --symbols='tsv_lang::doc::arena::DocArena::subtree_layout_fill' --sort=srcline
#   7.67%  arena.rs:2502   <- the Concat/Fill child loop's recursive call
#   3.42%  arena.rs:2427   <- the `match &nodes[id.index()]` dispatch
#   1.70%  arena.rs:2559
```

⚠️ The line numbers in that sample are from one build and go stale on the next edit — read the
*shape* (which source construct dominates), not the digits, and re-take it against your own
binary. The two columns are children% and self%.

⭐⭐ **When the top source line is a big `match`, go one instrument further down — to
`objdump`.** A srcline board cannot tell four instructions from fourteen, so a dispatch line
that reads "the jump table, nothing to do here" may be carrying real work. Get that line's
instruction pointers out of `perf script` and disassemble around them:

```bash
perf script -i flat.data -F ip,sym,srcline \
  | awk '/<symbol>/{ip=$1; getline; if ($0 ~ /<file>.rs:<line>/) print ip}' \
  | sort | uniq -c | sort -rn        # the line's hot IPs (srcline prints on its own line)
nm -C --print-size target/profiling/tsv_debug | grep '<symbol>'   # the symbol's file vaddr
objdump -d --start-address=0x… --stop-address=0x… target/profiling/tsv_debug
```

The runtime IPs are PIE-relocated, so match them to the file by their low 12 bits (page offset
is preserved by `mmap`) and confirm by the block's shape. **The whole basic block's sample total
is sound, and it is an EV model**: block share ÷ instructions in the block = what one removed
instruction is worth, computable before any code is written.

⚠️ **Do not read the distribution *within* a basic block as instruction counts.** Straight-line
instructions that execute the same number of times routinely draw wildly different sample counts
— that is skid clustering after a long-latency operation, not work. Read the block total.

Reach for `annotate` when the question is genuinely *per-instruction* — which
store, which compare — and for that, cross-check it against this view: the two
agreeing is what rules out a symbol-resolution artifact. `--sort=srcline` alone
(no `--symbols`) ranks source lines across the whole profile.

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

A near-flat instruction delta (e.g. ≤0.1%) paired with a larger cycles delta and a
drop in instructions-per-cycle is usually read as a code-placement artifact — added
code (a new monomorphization, more inlining) shifting hot functions across cache
lines. That reading is often right, and **this recipe cannot establish it**: it
compares one binary against one binary, and a build's code layout is worth
~1.2–1.8% of cycles on its own (§Reading `cycles:u` below). Below that, the recipe
reports a layout draw whichever way it comes out — a candidate that reads +1.2%
cycles at flat instructions and a *baseline rebuilt against itself* are the same
measurement. Resolve it with the layout-group protocol below; do not resolve it by
assumption in either direction, and do not treat a single pair's IPC drop as
evidence of anything.

For a printer-only edit, **parse is a built-in control**: its code is unchanged, so
any instruction movement there is pure layout. A real algorithmic change instead
shows up as more *instructions* — but the converse does not hold, since a change
that moves only bytes and cache lines shows up in neither.

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

⭐ The `Static` row doubles as the **layout probe** for the address-keyed static
cache (§A cache keyed on an address…): run the same binary twice with ASLR on and
a moving `Static` count is a collision draw, not a code change. Under
`setarch -R` the whole report is deterministic, so the `Static` delta between two
binaries prices their layout term without touching the PMU.

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
*formatted* and three printer-buffer populations are sampled at their construction
chokepoints (`tsv_ts::printer::buffer_stats`), so inline-`N` claims are measured
data, not doc-comment prose: `ChainNodeVec` (nodes per linearized chain),
`ChainGroupVec` (groups per `group_chain_nodes` call), and the leading-comment
`CommentVec` (per `collect_leading_comments` call — the type's dominant site).
A `ChainGroup` owns no buffer to size — it is a borrowed sub-slice of the
linearized chain — so it carries no population of its own, and neither does the
peeled trailing member tail, which is a pair of runs over that same buffer. Covers
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

### 10. `tsv_debug ast_census` + `tsv_debug type_sizes` — the density pair

The two inputs every AST-density decision needs, and the reason they are one
section: a lever that narrows a type is worth **population × bytes saved**, and
neither factor can be reasoned out. Width is not population — the widest node
enum in the tree can be two percent of the traffic — and population is not
width, so ranking by either alone mis-ranks the ladder.

`type_sizes` prints the `size_of` / `align_of` board for every public AST type
in `tsv_ts` / `tsv_svelte` / `tsv_css` plus the foundation types, widest first,
with `size_of::<Result<T, ParseError>>()` beside each (a payload of at least 16
bytes whose `Result` is wider has no niche for the error and pays a word on
every fallible return — those rows are starred). `--min` / `--top` / `--group`
trim it; `--json` for scripting. It complements the in-crate
`const _: () = assert!(size_of::<T>() == N)` guards rather than replacing them:
those pin the few widths a change must not move, and this makes the rest
*visible*, so a type that grows shows up as a row that moved.

`ast_census` counts, per node kind, what a parse over a corpus actually builds.
`--bytes` joins each row against that board by name and prints `count × size`,
which is the slot-megabyte column a density lever is ranked by, plus the
corpus's total slot bytes against its source bytes. `--slots` adds a
`Parent.field -> Child` tally, which is what separates one struct's two
populations — an object literal's `Property` from a destructuring pattern's, the
same Rust type and so the same row without it. `--min` / `--top` / `--json` as
above.

```bash
cargo run -p tsv_debug type_sizes --min 96              # the wide end of the board
cargo run --release -p tsv_debug -- ast_census ../fuz_app/src --bytes --top 20
cargo run --release -p tsv_debug -- ast_census ../fuz_app/src --slots
```

⚠️ **Counts come off the wire AST**, which the writer emits by walking the
internal AST once, one object per node — so a count is the parser's own
construction count for every node the wire names. The blind spot is an internal
node the writer does not name: `ParenthesizedExpression` and `JsdocCast` are
`Expression` variants it prints *through* (so a census of `Expression` values is
a lower bound, short by exactly the parenthesized ones), and the slot enums
(`ForInit`, `ArrowFunctionBody`, `AttributeValue`) are field types rather than
emitted objects, reachable only through `--slots`. Every container the density
ladder asks about is a named wire node and is counted exactly.

⚠️ **A census counts values; a `?`-ladder charges per level.** The `count ×
size` column ranks a type by where its values come to *rest*, which under-counts
one the parser threads up a deep precedence ladder — a `TSType` is returned
through every level of the type parser's `?`-chain, so its width is paid once
per level and not once per node. Narrowing `TSType` 112 → 80 B outmeasured a
`Statement` rung carrying ~3× its slot megabytes, by ~1.6× on every corpus. The
instrument that reads that channel directly is the recursion-depth probe
([cli.md §Recursion Depth](./cli.md#recursion-depth)), worth a look before
ranking two rungs a slot census puts close together.

The same query answers a non-perf question: pointed at `tests/fixtures`, the
census says which node kinds the fixture tree never exercises.

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

### The structure you already keep is usually the census

Before adding counters to a hot path, ask whether something the program already
maintains records the answer as a side effect. Adding atomics to the very
functions under study perturbs their codegen — the thing an instruction A/B is
most sensitive to — so a probe that costs nothing at all is worth hunting for.

A **memoization cache is the strongest instance**: a slot is populated *iff* that
node was visited by that fill, so the cache's population, read once where it is
cleared, is an exact visit census — no counters, no feature flag, no perturbation.
The doc engine used to keep two such caches for a document's lifetime — one for
the build-time forced-break verdict, one for the render-time flat width — and
cleared both in `DocArena::reset`, so a loop over the pair there answered "do
these two passes walk the same tree?" to the node. It measured 97–99% overlap,
and — by comparing the two *values* rather than just their presence — showed that
`will_break == true` implies a `BREAKS` flat width over 910K co-visited nodes.
That census is why there is now **one** cache and one walk (`layout_cache` /
`DocArena::subtree_layout_fill`): the implication turned out to be provable by
induction over the node kinds, so the two answers pack into one `u32` and the
second traversal became a cache read. The same trick reads off an arena's node
population (§7) and any dedup set or interning table.

⚠️ **Check the ordering assumption before a bottom-up pass.** Deriving a
per-subtree property by iterating ids upward is only valid because children are
allocated before parents; state that, rather than relying on it silently.

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

**A whitespace emitter is the same class, and its blind region is a *depth*.** The
indent run at the head of every line is output a corpus grades in full — up to the
depth real code stops at. Four app corpora never exceed **14 levels**, while the
parser accepts nests thousands deep, so an emitter that specializes the first few
depths and chunks a static run past them holds three code paths a 51,921-file
byte-identity diff never reaches. `write_indentation`'s equivalence tests
(`arena_render`'s `column_arithmetic_tests`) grade every depth to two full runs
against a reference spelling, for each indent string the render config can carry,
crossed with sub-tab alignment and the embed offset.

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

### A candidate scan can ask a wider question than its answer

`tsv_lang::printing`'s line-terminator scan is a *candidate* scan: it reports a
position and `line_terminator_len` classifies it, so a false positive costs a
classification and a byte step, not a wrong answer. That licence is worth half the
scan's operations. Its three exact SWAR needles (`\n`, `\r`, and the `0xE2` lead
of `<LS>` / `<PS>`) cost fourteen ALU operations a word; two **loose** ones cost
seven and cover the same class, because dropping `zero_lanes`'s `& !v` term admits
every non-ASCII lane — and `0xE2` is non-ASCII, so the third needle is subsumed
rather than spelled. The loop goes from twenty instructions per eight bytes to
fifteen.

Two rules come out of it, and the second is the one that decides the shape:

- **The lowest-lane guarantee is what survives the dropped term, so check it
  first.** A zero lane flags itself, a lane at or above `0x80` flags itself
  through the `| v`, and a lane in `0x01..=0x7F` flags only on a borrow-in —
  which requires a genuine zero below it. `crate::swar::zero_or_high_lanes`
  carries the argument; the exhaustive alignment test grades the caller against
  the class it always exported.
- **A loose class is a per-document tax, and no standing corpus varies the
  property it keys on.** Handing the loose candidate straight to the caller is
  the cheapest possible shape and measures a clean win on real source, which is
  0.03–0.32% non-ASCII — and roughly **+20%** on a document that is 98%
  non-ASCII, where every byte becomes a hit to step over. So the scan keeps its
  exported class exact: an all-ASCII word makes the loose mask identical to the
  exact one lane for lane, and only a word that holds a non-ASCII byte falls back
  to the three-needle scan. **Build the adversarial document from the mechanism
  before choosing between shapes** — here the file that decided it was not the
  98% blob but an ordinary i18n message table at 62%.

⚠️ The fallback is `#[cold] #[inline(never)]` and that is not a tuning detail:
inlining it grows the scan past the inline threshold, the *scan* is then emitted
out of line, and the whole win disappears. The free tell is the counter-intuitive
one — inlining more source makes `.text` **smaller**, because several inlined
copies collapse into one. Screen every candidate with `objcopy -O binary
--only-section=.text` and explain the sign before measuring.

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
| data-dependent branches removed | `branch-misses:u` |
| code size | the WASM bounds (see above) |

⚠️ **The third row is a diagnosis, not a verdict.** A branch-miss reduction is no more a
cycles claim than an instruction reduction is: a scan rewrite that removed **2.4% of the
whole program's branch misses** — essentially the entire share of the function it touched —
still lost 0.45 points of cycles (§And the converse, below). Read that counter to learn
whether the mechanism you believed in actually fired; grade on the one the change claims.

The worked case is the direct sequel to the fixed-width-copy change described
above. That copy loaded 16 bytes out of a stack scratch which the digit
generation had just filled with **2-byte stores** — a narrow-store → wide-load pattern that
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
- **The two counters are independent below about 1%.** The cleanest demonstration
  is the layout experiment below: four builds of one source, identical work, move
  `instructions:u` by 0.000% and `cycles:u` across a 1.17% range. So a small lever
  must be graded on the counter it actually claims to move — bytes moved, nodes
  allocated and lines dirtied are cycles claims; ALU, branch and call overhead are
  instruction claims. ⚠️ But the counters are not equally resolvable: instructions
  resolve to ~0.002% on one binary pair, while a cycles claim below ~1% needs a
  layout group per side *and* several replicates of the whole sweep. A cycles
  number quoted from a single binary pair, from a group that shares one layout, or
  from a single run of a layout group, measures a draw rather than the lever.

### Reading `cycles:u`: the offset belongs to the binary, and it is CODE LAYOUT

Cycles look like noise on a two-binary A/B, and the usual conclusion — "cycles are
unusable, grade on instructions" — is wrong. But so is the first repair. Three
terms are being added together, and only the third is the work:

- **Run to run, with the binary fixed**, cycles are steady: two byte-identical
  copies of one binary compare at a paired median of −0.08%.
- **Build to build, at fixed code layout**, a binary carries a small offset from
  whatever address-keyed state it holds. Rebuilding one source with a different
  constant in `DocArena::text`'s slot hash — no behaviour change, +0.03%
  instructions — moves cycles ~0.4%.
- **Build to build, across code layouts**, the offset is ~**1.2–1.8%**, and it
  swamps both of the above.

The third term is the one that matters, and it is easy to miss because the obvious
perturbations do not move it. Measured here: four builds of one source that differ
only in the *size of a function that is never called* — identical work, identical
`instructions:u` to **0.001%** — read cycles 1798.8M, 1817.3M, 1820.0M and 1824.5M,
a **1.17% spread**, stable per binary and reproducible to 0.01 points across
measurement sessions. Adding dead code re-lays every symbol emitted after it, and
that is all it takes.

⚠️ **So two "inert perturbation" recipes are traps.** Varying a *constant* — a
different multiplier in a hot hash — leaves code layout untouched (a `movabs`
immediate is the same length whatever its value), so four such builds are four
samples of **one** layout. And a null group of byte-identical *copies* shares one
layout by construction, so it is structurally blind to the very term it is meant
to license: it cannot fail. Together they make a broken instrument look calibrated
— a four-multiplier group of one source, compared against a four-layout group of
that same source, reads a **0.79% gap** on identical work.

**Sample the layout instead.** Build both sides three or four times with a
perturbation that genuinely re-places code — the cheapest is an
`#[inline(never)]` function that nothing calls, given a different body length per
build and kept alive with a `#[used]` static function pointer — measure all of
them interleaved with the sweep direction alternating between reps, and compare
**group means**. Two things then make the result trustworthy:

- a **null group that can fail**: a *second* set of layout draws of the baseline,
  passed in the candidate positions. A group gap the null also produces is not a
  result. (The layout term averages down as `1/sqrt(n)`, so a tighter bound on it
  costs builds.)
- **replicates of the whole sweep, not more execs inside one.** ⚠️ A single group
  run does not resolve a sub-1% cycles claim however many draws a side it has:
  re-running one twelve-binary sweep four times — same binaries, same corpus, same
  eight execs each, minutes apart on a quiet machine — moved the candidate's cycles
  delta across **−0.55% → +0.08%** (a **0.63-point** range) and the null's across
  0.37 points. Replicate 1 alone would have licensed "−0.55%, four times the null";
  replicate 2 alone says +0.08%. Run the sweep at least three times and pool. The
  within-binary spread is already flat at eight execs — the residual term lives
  *between* runs, so adding execs inside one buys nothing.
- **`task-clock` agrees and is cheaper to reason about.** Wall tracks cycles to
  within 0.05–0.3 points through all of this, including the layout draw itself, and
  it is the tighter of the two across replicates (0.27 points against 0.63). Run
  both; wall is the axis a user feels, and its agreement is the sanity check that
  the cycles reading is not a counter artifact.
- ⚠️ **Grade cycles on the largest corpus available.** The layout and run-level
  terms are relatively larger on a shorter run: on 900-file pure-`.css` (187M
  cycles) and pure-`.svelte` (516M) sets, the *null* read −0.70% and −0.44% —
  larger than the effect under test, where the same null on a 1,810M-cycle corpus
  averaged ~0.00%. A sub-1% cycles claim cannot be made on a 200M-cycle workload.

⚠️ **A cycles verdict belongs to a (codegen profile × binary × entry point)
triple, not to a source change.** All three have been observed to flip a sign
here. The same arm-list change reads **+0.33% instructions** built `--profile
corpus` (`lto = false, codegen-units = 16`) and **−0.11%** built `--release`
(`lto = true, codegen-units = 1`); and a lever that reads −0.43% cycles on
`tsv_debug profile` reads +0.50% on `tsv format --check` — two binaries, one
source, opposite verdicts, because each drew its own layout. Grade in the world
the artifact ships in, and re-read the baseline with the exact command the
candidate used.

⚠️ `.text` is not a shortcut for any of this. Of three spellings of one change, the
one that grew the binary by **96 bytes** measured the *worst* cycles regression and
the one that grew it by 5,344 bytes the mildest — an i-cache story has to be
measured, never inferred from the size delta.

⚠️ **And the cache counters do not disambiguate it either.** Two layout draws of one
source, 0.80% apart in cycles at 0.000% difference in instructions, read L1
d-cache misses flat, frontend stalls +0.8% (a quarter of the gap) and L1 **i-cache
misses 24% LOWER on the slower binary**. A change that does nothing produces the
full paradoxical signature, so reading that signature off an A/B and concluding
"neither cache story survives, it must be something subtle" is a category error —
what it identifies is a layout draw. Counters are for a gap you have already
established with layout groups, never for deciding whether one is real.

⭐ **`instructions:u` is the channel that survives all of this.** Across four
layout draws of one source it moves by **0.000%**; across four hash draws, by
≤0.03%. It is not a proxy for cycles (below ~1% the two are independent), but it
is the only one of the two that a small lever can actually be resolved on without
a build farm.

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

### A cache keyed on an address makes `instructions:u` re-draw on every exec

Retired user instructions are the arc's primary verdict because they are supposed
to be a property of the *code*: same binary, same input, same count. That holds
only while nothing the program does depends on **where** it was loaded. One thing
does — [`DocArena`'s `static_cache`](../crates/tsv_lang/src/doc/arena.rs), a
direct-mapped table whose index is a multiplicative hash of a `&'static str`'s
**runtime address** — and while it was small enough to collide, it made the whole
format board non-deterministic:

- The address is not a link-time constant on a PIE target: it is
  `image_base + link_offset`, and the base is re-randomized by ASLR on every
  `execve`. So the *slot* every static lands in is re-drawn per run. (Whether two
  statics collide is mostly fixed by their offset **difference**, which is a link
  constant — which is why a given binary usually lands in one or two modes rather
  than scattering, and why a one-line source edit anywhere re-rolls it.)
- A collision between two statics that are both hot means every call to either
  one misses, re-measures its width and **allocates a fresh doc node**. Sized at
  512 slots, one `tsv_debug` binary spread **0.53%** in `instructions:u` across
  14 execs of the same binary on the same input, bimodally; `arena_stats` over
  the same corpus put the `Static` node population anywhere from 18,669 (the
  no-collision floor) to 30,994.

The table is now sized so the draw stops mattering (per-exec spread ~0.01%), but
the shape generalizes to any address-keyed cache, so the reading rules stand:

- **A real effect moves min, max and mean together; a layout draw moves the
  spread.** The `alloc_children` 4-arm rung reads `+0.328 / +0.332 / +0.336%` on
  three corpora with min, mean and max within 0.01 point of each other — that is
  code. A binary sitting on a bad draw reads a *wide* distribution whose median
  is itself a coin flip (three 12-run medians of one comparison read −0.55%,
  −0.36%, −0.20%).
- **Report the per-side spread beside every delta.** It is the only thing that
  separates the two cases, and a median alone hides it.
- **`setarch -R` pins the draw** (and makes a run deterministic to ~0.001%), but
  it pins each binary to *its own* draw — so it removes the run-to-run half of
  the hazard and not the between-binary half. Two binaries can differ by 0.3% in
  retired instructions on a comment-only edit.
- **`arena_stats`'s `Static` node count is the free, PMU-free instrument for it.**
  It moves monotonically with the instruction count (~735 instructions per extra
  node on a 341-file corpus) and needs no quiet machine: run it under
  `setarch -R` on both binaries and the difference is the layout term.

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

### And the converse: a hot leaf that owns a symbol is a call

`#[inline]` is a hint against LLVM's cost model, not an instruction, and that
model prices *code*, not the hot path. Two of the format path's largest single
levers were functions that already carried `#[inline]` and were emitted out of
line anyway, then called from the innermost loop:

- `tsv_lang::doc::types::resolve_text` reads as a four-arm match handing back a
  `&str`. It compiles to roughly fifty instructions, because every `&s[a..b]` on
  a `str` carries two `is_char_boundary` probes and an edge to
  `slice_error_fail` — cold code the cost model still counts. It ran once per
  rendered `Text` node. `#[inline(always)]` measured **−1.4..−1.6%
  instructions** across five corpora.
- `tsv_ts::lexer::core::Lexer::advance` is **67 bytes** of code — one bounds
  probe and an add — in the lexer's innermost loop, and still had an out-of-line
  copy. `#[inline(always)]` measured **−0.33..−0.39%**.

The instrument is one command, run against the same binary the board came from:

```bash
# symbol sizes; cross these against the board's self column
nm -C --print-size --size-sort target/profiling/tsv_debug | grep ' [tT] '
```

**Small *and* hot is the signal.** A function with a self column is a function
being called; a small one is a call the caller could have absorbed. This is the
positive use of the `nm` check that §`perf` recommends for the opposite reason
(an inlining verdict expires — re-run it against the binary in front of you).

⚠️ **The discriminator is what the bytes *are*, not how many.** A few loads
wrapped in cold panic/bounds edges is a candidate; a byte scan is real work and
is not — `is_single_param_arrow_start` is 218 bytes, but those bytes are
`scan_identifier_then_arrow` already inlined into it. Read the callee's
disassembly before believing it is small.

⚠️ **The `.text` sign is neither predictable nor the verdict.** Forcing the
five-instruction leaf in made the binary **528 B smaller** — the out-of-line copy
and every call site's argument setup both went away — while the fifty-instruction
one cost **+640 B**. Measure the size axis either way
(§An instruction A/B is blind to code size).

⚠️ **Per-symbol A/B, never a sweep.** `#[inline(always)]` has an i-cache U-curve,
and the section directly above is the case where out-of-lining is the right
answer. The two verdicts coexist: out-line when the win is work removed *inside*
a body the caller was already calling, force in when the call *is* the cost.

### When the win is real and the uniform spelling is not: split the head

A third instance of the shape above answers the question the first two did not
have to: what to do when the callee is *hot, small, and called from everywhere*.
`DocArena::concat` is the doc builders' widest chokepoint — over 1,200 call
sites — and its 508 bytes are the canonical candidate profile: a length
dispatch, two `RefCell` borrows and two `Vec` appends on the hot path, with the
size coming from cold `panic_already_borrowed` / `do_reserve_and_handle` /
`grow_one` edges and five callee-saved registers spilled to serve them.

Forcing the whole body in works and is unaffordable: **−0.89% instructions** for
**+349 KB of `.text` (+12.1%)**, with `cycles:u` turning positive — the U-curve,
arriving on schedule. What ships instead keeps the *dispatch* inline and pushes
both allocating arms out of line behind `#[inline(never)]`, with the
two-children case taking its ids **by value** so a folded call site is a
register handoff instead of a stack array. That recovers **−0.40…−0.50%** across
four real corpora for **+9.7 KB** — 52% of the win at 2.8% of the size — and
`cycles:u` goes negative with it.

So the decision is not binary. Read it as three questions in order:

- **Is the callee's hot path a few loads behind cold edges?** If not (a byte
  scan, a loop), stop — `build_line_breaks_into` is a SWAR newline scan run once
  per document, ~1.2% of cycles and ~2% of retired instructions on a real-corpus
  board, and its cost is work rather than call overhead. ⚠️ That answers the
  *inlining* question only, and answering it is not the same as clearing the
  symbol. A scan whose cost is work is attacked by asking for less of it — see
  [§A candidate scan can ask a wider question than its
  answer](#a-candidate-scan-can-ask-a-wider-question-than-its-answer), where this
  same function's mask lost half its operations.
- **How many call sites?** Cost is roughly (inlined body) × (sites). A handful
  of sites makes the whole-body question moot; a thousand makes it the deciding
  term.
- **What part of the call is actually the cost?** Where argument marshalling and
  a dispatch the caller already knows the answer to dominate, an
  `#[inline(always)]` head over `#[inline(never)]` arms buys that half and
  leaves the body's bytes in one place.

⭐ This is also the clearest available argument for profile-guided
optimization: the uniform-inline win is real and only its *distribution* is
wrong, and choosing which call sites deserve an inline is exactly what a
profile-guided inliner does automatically — a hand sweep down a candidate table
is doing PGO's job one symbol at a time.

### And the trap on the other side: an edit that RE-SHAPES an inlined probe can un-inline it

The two sections above are about a callee LLVM declined to inline in the first
place. The nastier version is a callee it *was* inlining, until your edit
changed it — because the sign flips and nothing announces it.

The fused layout walk's memo probe (`DocArena::subtree_layout_memo`) is a cache
load, a compare, and a peel for the kinds it can answer itself. Adding a second
peel to its miss path pushed it past the cost model's threshold: the probe was
emitted out of line, appeared on the next board as **its own 4.29% symbol**, and
every one of its ~1.6 M calls — the warm ones, which are the whole reason the
probe exists — paid a real call. `instructions:u` **+2.085%**, for an edit whose
intent was a −0.4% win. `#[inline(always)]` on identical source: **−0.377%**.

- ⭐ **The tell is free and it is counter-intuitive: `.text` went DOWN.** A perf
  edit that adds source and *shrinks* the binary by 320 B has outlined something
  — the nine inlined copies collapsing into one costs more than the new code
  adds. Take `objcopy -O binary --only-section=.text` + `stat -c%s` on every
  candidate; a shrink you cannot explain is this.
- ⭐ **Confirm the attribute is buying inlining, not luck.** Applying
  `#[inline(always)]` to the *un-grown* probe rebuilt `.text` **byte-identical**
  to the baseline — proof the small body was already being inlined everywhere,
  so the attribute on the grown body is restoring the old behaviour rather than
  contributing a layout draw of its own. That control costs one build and no
  measurement.
- ⚠️ Then re-check the size axis anyway: the shipped shape is +1,968 B of
  `.text` and +2,081 B of WASM, nine copies of a bigger probe.
- ⚠️ **It is not only about growing, and that is the part the heading used to get
  wrong: the cost model turns on the body's SHAPE, not on its size alone.**
  Re-cutting the wire writer's integer emitter from a compare ladder plus a pair
  loop into three magnitude arms with a variable shift made `JsonWriter::stage_u32`
  *smaller* — and LLVM declined it where it had honoured the bigger body, because
  one linear fall-through chain estimates cheaper than branchy blocks. Left alone
  it read `instructions:u` **−0.796% with cycles +5.802%**: fewer instructions, far
  more time, which is the signature of a staged emitter losing the register
  residency it exists for. `#[inline(always)]` on the same source: **−3.964%**. Two
  of those blocks were `panic_bounds_check` edges from a table index the compiler
  could not prove in range — so **a bounds check on a hot leaf is an inlining
  hazard, not just an instruction**, and masking the index to a power-of-two table
  was worth 3.0% of the path on its own.
- ⚠️⚠️ **The tell runs BOTH ways, so screen `.text` in both directions: a rise on
  an edit that DELETES code is a function collapsing into its callers.** Removing
  one conjunct from `tsv_css`'s `Printer::has_blank_line_between` took `.text`
  **up 592 B**, and `nm --print-size` between the two profiling builds showed
  *exactly three* symbols changed: `has_blank_line_between` **262 B → 0, gone**,
  with `print_css_nodes` (+838 B) and `print_css_block_children` (+176 B)
  absorbing it. That inlining event was **half the lever** — see the next section.
  An unexplained `.text` *fall* on an addition is an outlining event; an
  unexplained *rise* on a deletion is an inlining one; both are worth more than
  the source change that caused them.

### Price a redundant pass by RUNNING IT TWICE

`tsv_css` walks a declaration's value text more than once by design — the boundary
scan (`parser/decl_scan.rs`) finds its extent, then `ValueParser::fast_scan`
classifies its top-level separators over the very same bytes. Fusing the two owes a
model-agreement argument across two scanners whose nesting rules differ, which is
hours of design before a single number exists.

**So make the code run the pass a second time and measure that.** `ValueParser::parse`
was made to call `fast_scan` twice and use the second result — output-identical by
construction (the one arm with side effects keeps the first result), with
`std::hint::black_box` on an argument so LTO cannot CSE the pair. One build:
`instructions:u` **+6.402%**, cycles **+5.481%** — the entire cost of one pass, with
no correctness argument owed and nothing designed yet.

- ⭐⭐ **It beats a board row for this question.** A board says what a symbol costs
  *including* work the fusion would not remove — its prologue, its other arms, its
  callees. The doubling probe measures precisely the work that would be deleted. It
  is the deliberately-wrong-probe idea (§A cache keyed on an address…, and the
  batched-writer ceiling below) with the wrongness in the **repetition** rather than
  the order: state the invariant the wrongness must still satisfy — here, identical
  output — and read the number as a ceiling.
- ⭐⭐ **It hands over the conversion ratio of the work you are about to delete.** The
  probe read cycles ÷ instructions of **0.856**, where a scan-shaped lever in this
  repo usually converts near 0.4. The shipped lever landed at **1.54×**, so the probe
  *under*-reports cycles — its second call runs cache- and predictor-warm and the real
  one does not — which is the safe direction for a go/no-go.
- ⚠️⚠️ **Divide the probe by your census and compare the quotient to the
  disassembly.** The probe's +6.402% over the first census's population worked out to
  **24.5 instructions per byte** for a loop `objdump` shows is six. The census was
  wrong, not the probe: it had enumerated the entry points into the *program* and
  missed a second constructor of the hot type — `parse_function_arguments` builds a
  fresh `ValueParser` per function argument list, so `var(--x)` and `rgb(…)` re-enter
  the recursion. **Enumerate the entry points into the FUNCTION**, i.e. `grep` every
  constructor of the type it hangs off, not just the function's own name. The
  corrected population halved the reachable share, 80% of calls → 44%, and the lever
  landed at −1.884% against a corrected ceiling of ~2.7%.
- ⭐⭐ **The oracle for a replaced computation is the computation it replaces.** The
  new fact could have joined a struct whose debug oracle is a full token walk, which
  would have meant implementing the classification a second time — in the harder
  dialect — to grade the first. Carried *beside* that struct instead, it is graded by
  a `debug_assert` against the skipped function itself, and every CSS fixture
  re-proves it. That works only because the arms taking the shortcut do not recurse;
  the one that does still runs the real pass.
- ⭐ **A withheld answer needs its own spelling.** `Option<ValueSeparator>`, where
  `None` is *cannot say* and `Some(ValueSeparator::None)` is *no separator*. A `bool`
  merges them silently, and every construct the donating scan steps over **whole**
  while the replaced one walks it byte by byte — here an unquoted `url(…)`, whose
  interior can hold a `(`, a quote or a `/*` — becomes a wrong answer.
- ⭐ **Census the decline the way the code declines.** A first eligibility pass
  excluded every value containing a `\` and read 18.19% ineligible; but strings are
  consumed opaquely by `string_end`, so a backslash inside one never reaches the
  declining arm. Modelling the opaque regions took it to **0.31%** — 99.4% of value
  bytes carry a class.

### A function's stated POSTCONDITION is a free precondition for its caller

`ValueParser::build_leaf` was the third-largest symbol on the CSS board, and its
hottest line was the matching-paren byte loop inside `extract_function_parts` — the
walk that decides whether a leaf value like `var(--x)` is a function at all. That
function's own doc already said what it returns on: *"`Some` means the whole of `s` is
the function — the matching close paren is its last byte."*

Read as a claim about the **input**, that sentence is a gate: a value whose last byte
is not `)` cannot possibly be a function, so both walks — the search for the opening
`(` and the matching-paren scan from it — are answering a question one byte comparison
has already answered. A census over 638 files of real CSS put **71.4%** of the calls
(27,740 of 38,845) and **81%** of the search's bytes on exactly those values: `red`,
`0`, `1px`, `#fff`, the ordinary contents of a stylesheet. Gating on
`s.as_bytes().last()` is `instructions:u` **−0.567%**, cycles **−0.329 pts** and wall
**−0.368 pts** against a twelve-binary layout group's null, 3/3 signs in both.

- ⭐⭐ **This is the mirror of §A conjunct's cheap half may IMPLY its expensive half.**
  There, a comment justifying a filter's *correctness* turned out to say the filter
  could never refuse anything. Here, a comment stating a function's *postcondition*
  turned out to describe a precondition the caller can test in one instruction. **Both
  are prose that nobody had read as a claim about cost** — re-read a hot leaf's doc
  comments before you re-read its disassembly.
- ⭐⭐ **Hold the implication with a `debug_assert`, not with prose.** The gate's
  soundness is "if `s` does not end in `)`, the scan would have returned `None`", and
  that is checkable by *running the scan* in debug builds — the same oracle rule as
  §Price a redundant pass by RUNNING IT TWICE. Every CSS fixture then re-proves the
  gate on every `cargo test` run.
- ⚠️ **The same reading does NOT extend to the walk that remains**, and the reason is
  worth knowing before anyone tries. The obvious next step is to donate the matching
  paren's offset from `fast_scan`, which already tracks paren depth over the same
  bytes — but the two models disagree: `fast_scan`'s depth is quote-aware and
  `extract_function_parts`'s is not, so `url("a(b")` is a function to one and an opaque
  identifier to the other. That is a **behaviour** difference, not a fusion, so it
  cannot ride a perf change.

### A conjunct's cheap half may IMPLY its expensive half — and then deleting beats reordering

`tsv_css`'s `Printer::has_blank_line_between` asked an O(log n) `partition_point`
over the document's whole line-break table AND-ed with the byte walk that actually
answers the question, with the search placed *first* as a fast negative gate. On a
CSS board that one `partition_point` was **1.59% of the whole run** at a single
`core::hint` srcline.

The gate could never fire. The walk's terminator class `{<LF>, <CR>, <CR><LF>}` is
a **subset** of the table's ECMAScript class, so the table refuses nothing the walk
accepts — and the function's own doc comment already said so, as a *correctness*
argument. **A comment justifying a filter's soundness is worth re-reading as a
claim about whether the filter does anything at all.**

- ⭐ **Census both answers per call, and their disagreements in each direction.**
  Temporary counters over eight corpora — ~330 K calls, including the 22,310-file
  WPT CSS set and this repo's own fixture tree — found **zero disagreements either
  way**. That is what licenses deleting a conjunct rather than reordering it.
- ⭐⭐ **Reordering — the edit that owes no soundness argument at all — was
  measurably worthless here.** Putting the cheap walk first skips the search on the
  87% of calls it rejects: `instructions:u` **−0.496%** and cycles *exactly at the
  null* over five replicates. Deleting the search outright: **−0.758%** and cycles
  **−0.740 points**. Removing 87% of the searches bought nothing; removing 100%
  bought 0.74 points.
- ⭐⭐ **A non-linearity like that is a codegen event, not a cost model, and it owes
  an `nm` diff before it owes a theory.** Only the deletion shrinks the body past
  the inline threshold, so the shipped lever removes the **call frame at four
  sites** as well as the search. The reorder keeps two predicates and a branch, so
  the function stays outlined — it even *grows*, 262 → 305 B.
- ⚠️ **The obvious mechanism was measured and refuted.** A rarely-true cheap test
  in front of a long dependent chain ought to turn a predictable unconditional
  search into a mispredicted conditional one — but branch misses moved +0.861% for
  the reorder and +0.576% for the deletion, the same tiny amount. A channel that
  fails to *separate* two candidates does not explain either.
- ⭐ **Keep the implication under test rather than in prose.** The surviving
  relation is a `debug_assert`, so every `cargo test` run re-proves it across the
  whole fixture corpus; a `.text` `cmp` confirms it is release-inert.

### An inline constant-width copy is the BASELINE target's; libc's `memcpy` is the machine's

`JsonWriter::stage_flush` appends a ~118-byte node header with a **runtime**-length
`Vec::extend_from_slice`, which reaches libc `memmove` and is the biggest single
row on the parse→JSON board. Making the length a constant looks free — it deletes
a call and four size compares — and it costs **+2.204% of cycles** (instructions
−0.075%; Δmin and Δmax moved with the mean, so not a layout draw).

`objdump` names it in one line. LLVM lowers the constant copy for the build's
**baseline** target: x86-64 baseline is SSE2, so it emitted **eight 16-byte
`movups` load/store pairs**. glibc's `memmove` is an **IFUNC** — the dynamic
linker resolves it at load time to `__memmove_avx_unaligned_erms`, which moves the
same 128 bytes in **four 32-byte `vmovdqu` pairs**. On a portable binary, inlining
a copy trades a call for a *wider but worse* instruction sequence.

- ⭐ **Any "inline this `memcpy`" idea owes `objdump -d | grep -c movups` before it
  owes an A/B.** One command, and it answers the question. The argument does not
  touch a copy small enough to be scalar — `JsonWriter::u32` blits 8 bytes, one
  `mov`, no vector question — and it does not touch WASM, where there is no IFUNC
  and `simd128` is in the target features.
- ⭐⭐ **And the row was mostly not the call anyway — price a copy row with a probe
  that removes the CALL and keeps the COPY.** That `memmove` is 10.96% of the whole
  parse→JSON run when bucketed by its first tsv caller frame, which reads like a
  redesign waiting to happen (buffer the writer, flush once per few KB instead of
  once per node). Its **ceiling** was measured first, for one build: raise the
  scratch, flush on a high-water mark, make `stage_flush` a no-op. That probe's
  output is deliberately scrambled — the writer's other appends still go straight
  to the buffer — but the byte *volume* is identical, and checking that is what
  makes the number mean something. Ceiling: **instructions −0.810%, cycles
  −0.547%**. So ~93% of the row is the bytes moving, which every batching or
  single-copy scheme still pays. **A fold-by-caller attributes a cost to a site; it
  does not say the cost is the site's to remove.**
- ⭐ **A deliberately-wrong probe is a legitimate instrument when the wrongness is
  in the ORDER and not in the WORK.** State the invariant it must still satisfy,
  check it, then read the number as a ceiling and nothing else.

### Where a peel's SET boundary lands decides the cost of the path it does NOT take

The same lever, built twice over different kind sets, measures −0.173% and
−0.377%. The difference is not what was peeled — both peel the same dominant
kinds — but whether the peeled set is a **contiguous run of discriminants**.

`DocNode`'s tags run `Text` 0..=3 (the niche), then `MultilineText`, `Line`,
`Indent`, `Dedent`, `AlignRoot`, `Align`, `Group`, `IfBreak`, `IndentIfBreak`,
`Concat`, … The first attempt peeled the *semantic* set — every kind that
forwards one child's value — which reaches over `IfBreak` to take
`IndentIfBreak` and `GatedState`. That set has holes, so LLVM lowers it to a
**jump table**, and the fall-through — `Concat`, 64% of the calls that still
reach the outlined fill — pays a table load and an **indirect jump** before
landing on the default. Dropping the two outliers and adding the constant-answer
kinds that sit inside the range instead (`MultilineText`, `Line`, a pre-broken
`Group`) makes the set `tag <= Group`: one unsigned compare, a direct
well-predicted branch, and a residual path that costs the same as before the
peel existed.

- **Price the fall-through, not just the peel.** The peel fires on a quarter of
  the calls; the fall-through fires on the rest, so a 4-instruction indirect
  jump there outweighs a 25-instruction frame saved here.
- **Variant declaration order is therefore a performance surface**, and one that
  no test can pin. Where a hot `match` peels a prefix, note it on the enum.
- Companion to §A per-site precondition is only as cheap as its fold, and to the
  niche-fold rule in `docs/architecture.md`: both say the same thing from the
  other end — a probe ahead of a switch is only free when the switch was going
  to compute it anyway.

### A RARE arm of a hot switch is priced on every path through it

`render_doc_core`'s dispatch is the hottest `match` in the formatter. Its
`MultilineText` arm is taken a few thousand times a run against millions of
`Text` / `Concat` visits, and its body is a loop with its own locals — a first
line, a remainder iterator, a per-line hardline call. Folded into the switch,
those locals are part of the register allocator's problem on **every** path
through it, including the ones that never reach the arm.

Three spellings of the same body, measured on `instructions:u`:

| spelling | pure CSS | fuz_app/src |
| --- | --- | --- |
| inline in the arm | **+0.124%** | −0.493% |
| `#[inline(never)]` | +0.046% | −0.773% |
| `#[cold] #[inline(never)]` | **+0.001%** | −0.758% |

- **The control is a document that never takes the arm.** A CSS file has no
  multi-line text node at all, so its column cannot be reporting the arm's own
  work — only the arm's effect on its neighbours. That is what makes +0.124%
  legible as register pressure rather than noise, and what grades the fix. Look
  for the input class that provably skips the code you are editing; it is a
  sharper control than any second phase of the same run.
- **Holding the body out is worth more than the body contains.** The same edit
  that removes the arm's own work (a `str::split('\n')` → `printing::next_lf`)
  measures −0.493% inline and −0.773% outlined: the outlining is not overhead
  paid back, it is a second, larger lever on the *caller*.
- `#[cold]` on top costs 0.015 points where the arm is taken and buys the last
  0.045 back where it is not. Take it when the arm is genuinely rare — this is
  the mirror of §Where a peel's SET boundary lands: there the unpriced path is a
  branch the peel does not take, here it is a *register* the arm does not need.

### A per-line `memchr` is mostly not a word scan

`str::split('\n')` re-enters `core::slice::memchr` once per line, and that
function is only word-at-a-time in its middle: it computes an alignment offset,
walks the unaligned prefix a byte at a time, runs a two-word body loop only
while `offset <= len - 16`, then walks the remainder a byte at a time again. On
a block-comment body — ten lines averaging forty-five bytes, measured over
`fuz_app/src` — about half of what the search retires is that byte loop, and the
per-line setup is paid ten times over a stretch one continuous scan crosses. The
`char` pattern adds its own layer on top: two bounds-checked subslices per hit
and a re-verification of the found bytes against the needle re-encoded as UTF-8,
for a one-byte ASCII needle.

`printing::next_lf` is the answer — the LF-only member of this module's SWAR
scan family, entered once per line but with no setup to redo. `split_lf` wears
it as the iterator the call sites want; the comment builder skips even that and
takes offsets directly, since it never wanted a `str` per line.

- **The win scales with how comment-dense the document is, and it is not small:**
  `instructions:u` −0.25 to −0.76% across the arc's corpora, **−7.4%** on a
  documentation-heavy file set, **−23.5%** where the comment lines are short.
- **The line-length axis is the one to check, and it comes back clean.** A
  hand-rolled one-word loop retires more per byte than `memchr`'s two-word body,
  so a document whose comment lines are hundreds of bytes long is where the
  trade could invert. Built and measured (400-byte comment lines): **−0.241%**.
  The per-line setup dominates even there.

### And the converse: once a scan IS a word loop, widening it does not pay

The section above is a byte-at-a-time scan becoming word-at-a-time, and it converts. The
same module's line-terminator scan — already word-at-a-time, and the board's top
branch-miss outlier at **2.5% of the program's branch misses against 1.5% of its
instructions** — was widened two independent ways, and neither did:

- **Two words per bound check and per branch.** Mask both, OR the masks, and ask which
  word fired only once one has.
- **Answer every flagged lane before leaving the block.** A "where is the NEXT candidate"
  entry point gets re-entered at the byte after each hit, so a caller that wants *every*
  terminator re-loads and re-masks words it has already read, once per line. Draining the
  mask in place asks each word exactly once.

Both are real instruction savings — `instructions:u` **−0.166 to −0.191%** across seven
corpora at a per-side spread of 0.001% — and the first also removes **2.4% of the whole
program's branch misses**, essentially the entire share of the function it touched. Both
**lost on cycles**: twelve binaries, three pooled replicates, **+0.45** and **+0.29** points
against the null, wall agreeing at **+0.52** and **+0.21**, every replicate positive on both
channels. L1d misses and frontend stalls were flat across all of them; IPC fell with the
instruction count.

**The scan is latency-bound on its per-hit chain, not throughput-bound on its word loop.**
That chain is `trailing_zeros` → the hit offset → the load that classifies the byte → the
sequence length → the next hop's start address, and the machine was already running the word
loop several iterations ahead of it. Halving the loop's own bookkeeping buys nothing it was
waiting on — and widening *lengthens* the chain: a select over which of two words fired sits
ahead of the `trailing_zeros`, and a drain adds a compare and a `max` between the classify
and the next block's address.

So the direction that converts is **byte → word**, which removes iterations *and* shortens
the chain. **Word → wider word** only halves an overhead the machine was already hiding.
Name the per-hit chain before widening a scan, and ask whether the loop was ever the
bottleneck.

- **Two shapes, two adversarial documents, opposite rankings — and neither column alone
  would have chosen.** Widening taxes short LINES, because a hop shorter than the block
  still masks the whole block: the blocked scan is neutral (**−0.009%**) on real files
  averaging seventeen bytes a line, exactly where the drain is at its best (**−0.302%**).
  The drain taxes non-ASCII DENSITY, because it routes the exact-needle fallback per block
  rather than per word: **+0.094%** on a 62%-non-ASCII message table, where the blocked scan
  reads **−0.126%**.
- **What the residual looks like when you stop.** The scan's own instruction cost is now
  mostly the SWAR masks themselves; the remaining candidates all trade chain length for
  operation count, which is the trade that just failed.

### The complement: a per-byte value almost nobody reads should be derived on demand

The two sections above both restructure a *loop*. This one deletes a *value*, and it is the
cheaper question to ask first: **what does this loop compute on every byte that its
consumers ask for almost never?**

The depth-tracking source scans — the printer's paren scan, the Svelte brace matcher, the TS
arrow-vs-paren lookahead — each carried an `operand_end` anchor, the position just past the
last significant non-whitespace byte, so that a `/` could be told from a division. They
maintained it eagerly: an `is_ascii_whitespace` test and a select on every significant byte.
The anchor is read only where the scan meets a `/`, which in real source is rare — so the
maintenance ran hundreds of times per read.

`OperandAnchor` carries instead the two facts the value can be rebuilt from — its value as
of the last boundary the scan crossed, and where the run of significant bytes since that
boundary began — and walks backward from the `/` when one is actually reached. Both fields
move only at a boundary, so the per-byte cost is nothing at all.

**The backward walk is sound precisely because it is bounded.** An unbounded lookback from
the `/` is the hazard `is_regex_start_after`'s own doc warns about: a block comment before
the slash puts the `/` of its `*/` in the lookback slot. This walk stops at the run's start,
so every byte it reads is one the forward scan already classified as significant, and the
comment case is answered by the stored boundary value rather than by looking at all.

**It converts far beyond its instruction count, which is the point worth keeping.**
`instructions:u` reads **−0.058 to −0.171%** across every real corpus — and a pure-CSS
corpus, which reaches none of these scans, reads exactly **+0.000%**. Twelve binaries over
three pooled replicates put cycles at **−0.451** points against the null and wall at
**−0.594**, every replicate negative on both channels: roughly seven times the instruction
saving. L1d misses are flat and IPC *rises* (1.940 → 1.958), the mirror image of the widened
scan above, where IPC fell with the instruction count. The eager anchor was a loop-carried
dependency holding a register; deleting it shortens the recurrence rather than the operation
count.

⚠️ **It is a trade, not a free win, and the axis is density.** The cost moves from per byte
scanned to per anchor read, so the break-even sits at roughly three or four scanned bytes per
`/` or comment inside a scanned region. Ordinary code is far above that — a division-heavy
geometry module still reads **−0.058%** — but a document contrived to pack several divisions
and inline block comments into one parenthesized expression reads **+0.9%**, and widening the
whitespace runs between them takes that to **+6.6%**. The realistic worst case found was
division-dense Svelte template expressions, whose brace regions are short: **+0.021%**.

**The instrument that finds this shape costs nothing.** `perf report -g
--symbol-filter=<symbol>` shows a hot symbol's *inlined children* — which is where the eager
anchor was, running per byte under a function whose own source lines said nothing about it.
A flat srcline board does not name it, and neither does the symbol board. Run that view over
the top of the board and ask of each loop: **who reads what this computes, and how often?**

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
  issues; doc-node counts are already covered by `arena_stats`, §7, and AST-node
  populations by `ast_census`, §10 — reach for those before wiring a throwaway
  counter table into the parser)

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
