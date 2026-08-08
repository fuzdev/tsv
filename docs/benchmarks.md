# Benchmarks

> What the published tsv benchmark numbers measure — and what they don't.

The harness itself — commands, corpus views, freshness guards, report files — is
[../benches/js/CLAUDE.md](../benches/js/CLAUDE.md). This doc is the reference
half: measurement fairness, the implementation catalog, binary sizes, and the
dependency / oracle-pin ritual. Profiling methodology for tsv's own code is
[performance.md](performance.md).

## Fairness caveats

Things the published numbers measure that aren't quite what they look like.

- **Single-threaded, per-file (universal).** The harness times one file at a
  time, sequentially (`await`ed in order, no `Promise.all` over files), so the
  numbers are per-file single-core latency, not multi-core batch throughput.
  Per-file compute is single-threaded for every impl: tsv (FFI + WASM) pulls in
  no threading crate (`rayon`/`num_cpus`/`threadpool`/`crossbeam` absent from
  every `Cargo.toml`; the workspace's `tokio` is dev/debug-only, not in the
  shipped `tsv_ffi`/`tsv_wasm` chain); prettier, `svelte/compiler`, and
  `oxc-parser.parseSync` are single-threaded JS. The lone nuance is `oxfmt`,
  whose programmatic `format` is an async napi call that may run the native work
  off the JS thread (its `tinypool` dep is CLI-only — `dist/cli.js` — not in the
  `format()` path); still one thread of compute per file, each call fully awaited
  before the next, so no fan-out is exploited. This deliberately excludes the
  multi-core batch throughput a CLI gets formatting many files at once (which
  most of these tools, tsv included, could provide) — a different benchmark.
- **Different tools produce different output — speed is not conditioned on
  correctness.** The timed work is "produce _this tool's own_ formatting," not
  "produce the same bytes," and no two of these tools emit identical output.
  Every formatter IS configured to the same layout targets to the extent its
  options allow — printWidth/lineWidth 100, tabs, single quotes, no trailing
  commas — for prettier (`canonical.ts` `PRETTIER_OPTIONS`), oxfmt (`oxc.ts`
  `format_async`), biome (`biome.ts` `applyConfiguration`), and dprint
  (`dprint.ts` `setConfig`; `quoteStyle: preferSingle` is the faithful analogue
  of prettier's `singleQuote: true`, which likewise switches quotes to avoid
  escaping, and `trailingCommas: never` fans out to dprint's 12 per-construct
  keys). Unmatched defaults (biome's width is 80; oxfmt and biome default to
  double quotes) would make rows wrap/rewrite different amounts of code,
  conflating config with engine speed. oxfmt's own width default is already 100 —
  pinned anyway so a default change can't silently skew the rows; the options
  provably reach its bundled-prettier Svelte fallback too. `prettier` is the
  reference and `oxfmt` also targets prettier conformance, so `prettier` vs
  `oxfmt` is the closest to a same-output race; `tsv` tracks prettier closely but
  _intentionally diverges_ in documented cases (the `_prettier_divergence` fixtures
  / the `conformance_prettier*.md` family; ~92% `corpus:compare:format` match,
  measured separately — not here); `biome` formats to its own style.
  Because residual layout decisions still differ, a format ratio is partly an
  output-shape difference, not pure engine speed — and nothing here verifies
  output validity, so a formatter emitting subtly wrong output fast would "win."
- **The format headline is cross-tier (native Rust vs JIT JS).** The `format`
  baseline is `prettier` (JS) and the flagship `tsv` row is the native FFI binary
  (AOT Rust) — a fair "what you get replacing prettier with tsv" number, not a
  language-neutral algorithm comparison. The same-tier reads are WASM-vs-WASM
  (`tsv_wasm` vs `biome-wasm` vs `dprint-wasm` vs `oxc-parser-wasm`) and
  native-vs-native (`tsv` vs `oxfmt`/`oxc-parser`); compare within a tier before
  attributing a gap to the formatter rather than the runtime.
- **Format groups include parse time.** Every formatter parses internally before
  printing, so format ratios are partly parser ratios. The numbers answer "how
  fast can X format my file end-to-end," which is what users care about.
  Documented in the report footnotes.
- **Self-corpus / representativeness.** The perf corpus is real-world code only
  (fixture suites live in the `gates`/`conformance` views — [corpus
  views](../benches/js/CLAUDE.md#corpus)), but it's dominated by
  the author's own fuz ecosystem plus svelte/kit source — the same code tsv is
  developed and fixture-tuned against. Throughput tracks the syntactic mix of
  _this_ corpus, so ratios are "N× on this corpus," not universal. CSS is by far
  the weakest sample: only a few dozen real standalone files exist in this
  ecosystem (most CSS is authored inside `.svelte` `<style>` blocks), so the
  corpus adds the `svelte_styles` harvest — those blocks extracted and
  concatenated per repo (~3× the standalone bytes, naturally-sized files). Those
  harvest bytes are also timed inside the svelte rows (rows are never summed, so
  this is disclosure, not distortion), and CSS per-file ratios stay the noisiest.
- **PGO native flagship (forthcoming — policy; no such row ships today).** The
  standalone native flagship (the bare `@fuzdev/tsv` binary) is planned to ship
  with profile-guided optimization: native-only, a measured ~17–19% wall-time
  win, **byte-identical** output, **Linux-only first** on that single-target
  build (the cross-platform prebuilt `.node` binaries stay standard-release until
  matrix PGO is a later step). When that row lands the policy is: **(1) both
  rows** — a standard-release native row *and* a PGO one, never PGO silently
  folded into the single native number; **(2) measure what ships** — publish PGO
  numbers only once a shipped artifact carries the recipe, labeled with which
  one; **(3) disjoint training corpus** — trained on a corpus disjoint from the
  measurement corpus, so a published number is never train-on-test (the profile
  generalizes, so disjointness costs nothing); **(4) byte-identical** — PGO
  changes code layout, not output. Fairness framing: against the JS reference
  tools PGO partly *closes a gap* (V8's JIT already does profile-guided runtime
  optimization for free); against native AOT competitors shipping standard
  release builds it's a build-config advantage they don't take — fair to report
  as "what tsv ships vs what they ship," disclosed so a native-vs-native read
  isn't mistaken for same-build-config. Never mix a PGO or instrumented binary
  into a regression anchor series.
- **Conformance-surface semantics (`BENCH_CORPUS=conformance`).** Parse-only by
  design, and the committed surface is **coverage-only** (per-tool preflight
  parse success over the fixtures-only conformance corpus) — the timed phase is
  skipped, so there is no committed throughput. The **Svelte** set has the
  `svelte/compiler`-rejected files removed, so Svelte coverage reads as fidelity
  on *valid* Svelte (svelte/compiler → 100%, the oracle) rather than raw success
  over the suite's deliberately-invalid error fixtures; a *higher* number is
  better, not "more permissive." The **tsc corpus** is filtered the same way and by
  the same argument, with tsc as the oracle that decides validity (its parser AND
  its `.errors.txt` baselines must agree) — and `tsc` is a row on this surface, so
  on that corpus it reads 100% by construction. The **prettier suites and the
  remaining CSS** keep the full set (acorn-ts trails modern TS, parseCss is lenient
  — neither is a validity oracle).

  Because those corpora answer different questions, the report splits each group's
  coverage **per corpus source** under the aggregate line. Read the source rows: a
  TypeScript parse gap is tenths of a point on a group that is mostly test262, and
  an oracle's 100% is not an achievement. The axis coverage cannot show at all —
  over-ACCEPTANCE — has its own tool, `diagnostics/ts_repo_over_acceptance.ts`
  (`deno task ts-repo:over-acceptance`), graded over the files tsc's parser rejects
  and read inverted.

  The ad-hoc timed variant
  (coverage flag unset) times the all-tools-pass intersection — an adversarial
  corpus's "easy" subset (`BENCH_MODE=union` audits what it hides). test262 files
  are parsed at the goal test262 **declares** (`SourceFile.goal`, from the
  harvest's per-file `module` flag → `module`, else strict `script`): tsv routes
  through its goal-aware bindings (native `*_with_goal`, WASM `goal` parse
  option), acorn takes `sourceType: goal`, oxc an explicit `sourceType` — so a
  script-goal `await`-identifier test is scored valid against every tool rather
  than counted as a module-goal failure. (Before this, everything parsed at
  module goal and those tests depressed tsv's and acorn's TS coverage alike;
  oxc's filename inference hid it, making tsv read ~2 files behind on a goal
  artifact.) Only the conformance-coverage preflight is goal-aware — the perf
  surface has no test262. The tsc corpus deliberately carries NO goal: tsc's
  module-vs-script reading is a semantic classification, not the ES `sourceType`
  switch, and mapping one onto the other scores a parser for syntax tsc itself
  accepts either way (`benches/js/harvest_ts_repo.ts` carries the measurement).
  The goal-aware per-test differential is
  `diagnostics/test262_compare.ts`; the graded pass/fail gates remain `tsv_debug
  test262` / `conformance:svelte-fixtures` — this surface measures coverage, it
  doesn't replace them.
- **Measurement-shape asymmetries (small, mostly self-cancelling).**
  (a) Every `tsv` FFI format call UTF-8-encodes the input and decodes the output
  back to a JS string (`lib/ffi.ts`, through persistent grow-only staging
  buffers, so the boundary cost is the encode/copy itself, not per-call
  allocation); `tsv_wasm` marshals strings across the JS↔WASM boundary. prettier
  pays no such tax — so the published `tsv` / `tsv_wasm` format numbers are
  _conservative_ (the parse analogue is the `tsv-internal` vs `tsv-json` gap).
  One nuance cuts the other way: the persistent buffers amortize across the warm
  loop, so a cold one-shot consumer pays a first-call allocation the warm
  per-call figure doesn't include — negligible next to process/module startup,
  but the warm number is a warm number.
  (b) The async impls (`prettier`, `oxfmt`) are `await`ed per file
  (`process_corpus_async`), carrying a per-file microtask cost the sync impls
  skip. The opt-in **`tsv-forced-async`** control row (`BENCH_FORCED_ASYNC=1` —
  the same native engine routed through the awaited path) quantifies it: the
  delta is within run-to-run noise even on a sub-ms-per-file engine, so the async
  impls' gaps vs `tsv` are engine differences, not harness tax. Scope caveat: the
  control models a *microtask* await (prettier's shape); `oxfmt`'s async is a
  napi promise whose native work may hop off the JS thread, which is part of
  oxfmt's binding boundary — the same way tsv's row includes its FFI boundary —
  not engine time, and this control doesn't isolate it. Off by default: a
  noise-level delta would only add a confusing duplicate-`tsv` row and feed
  spurious flags to the regression baseline. (Why a control and not a real sync
  row: `prettier` and `oxfmt` are async-only, so the tax can't be removed, only
  measured.)
  (c) Task return values are discarded uniformly for all impls; the FFI/WASM/async
  boundaries block dead-code elimination, so no impl's work is optimized away.
- **`tsv_wasm` is measured on the full build.** The WASM bench loads
  `pkg/all/deno` (the default both-features artifact, ~2.5 MB — what
  `@fuzdev/tsv_wasm` ships) for _both_ parse and format, while subset consumers
  ship the smaller `@fuzdev/tsv_format_wasm` (~2.3 MB, no convert layer) or
  `@fuzdev/tsv_parse_wasm` (~1.1 MB, no printers). Same story natively: the perf
  row loads the full `libtsv_ffi`, while the Binary Sizes table also lists the
  `tsv format (ffi)` / `tsv parse (ffi)` subset builds (no perf rows of their own
  — they exist only to size scope-matched against `oxfmt` and `oxc-parser`).
- **Intersection-corpus iteration (default).** Within each group every impl is
  timed on the same all-N intersection: the files every impl in the group
  processed during pre-flight. Ratios within a group are then apples-to-apples.
  Trade-off: one noisy impl shrinks the corpus for the whole group — if
  `biome-wasm` skips 60% of CSS files, `tsv`/`prettier`/`oxfmt` are timed on the
  remaining 40%. The Coverage section in `report.<runtime>.md` still discloses
  each impl's preflight skip rate, and `Throughput` + the `(Mf)` annotation
  reflect the iterated set, not the full corpus. **`BENCH_MODE=union`** is the
  opt-in escape hatch restoring per-impl iteration (ratios then reflect different
  file sets per impl, and `(Mf)` describes the self impl's count) — useful for
  auditing what intersection mode hides.
- **Ratio convention (universal).** Every `Nx` in the report is **speedup form**:
  `>1` means self is faster than the named opponent. Column headers spell this
  out (`vs prettier (speedup)`, `vs Best (speedup)`). The only exception is
  `JSON overhead` rows, explicitly labeled `json_ns / internal_ns` (higher = more
  cost) because overhead is inherently a slowdown ratio.
- **Per-iteration forced GC** — off by default (`BENCH_GC=1` makes the bench call
  `globalThis.gc()` between every iteration), and not a uniform bias. Measured on a BENCH_LIMIT=20 / 500ms / WARMUP=2 sample: low-
  allocation paths are penalized heavily (`tsv-internal` 1.4–1.7× slower with the
  hook on, `svelte/compiler` 2.8× — it allocates JS objects every call); format
  paths land 1.07–1.24× slower; CSS workloads on large inputs *reverse* the trend
  (up to 1.6× **faster**, since amortizing GC per-iteration avoids long mid-loop
  major-GC stalls). Default off because published ratios should reflect what
  users see in real code (opportunistic GC); enable it for the stability of
  forced GC on a noisy high-allocation workload. A report generated with the hook
  on has a narrower internal-vs-JSON spread, so don't diff numbers across the two
  configurations line-for-line.
- **`-json` parse rows are mechanism-matched but not payload-matched; the
  `oxc-parser` "lazy" story is a myth for the path we benchmark.** In
  oxc-parser's _default_ mode (what we call), the AST is serialized to a JSON
  string in Rust and deserialized in JS — the native package's `index.js`
  `wrap()` runs `JSON.parse` on `.program` access (verified: `typeof program ===
  'object'`), exactly the model `tsv-json` uses (Rust → JSON string → FFI →
  `JSON.parse`) and `tsv_wasm-json` uses (Rust → JSON string → boundary decode →
  engine `JSON.parse` via `js_sys`). So the rows are like-for-like
  full-materialization comparisons in _mechanism_ — but the _deliverables_
  differ: tsv emits the acorn/svelte drop-in AST with per-node `loc` line/column
  objects (measured: 46–48% of TS wire bytes and ~61% of its `JSON.parse` time —
  three nested objects per node), while oxc's default AST is span-only (no `loc`;
  it pads `decorators`/`optional`/`typeAnnotation` instead and still nets ~30%
  fewer wire bytes per source byte). Measured with `loc` stripped, tsv's wire is
  _smaller_ than oxc's and `JSON.parse`s _faster_, and the two Rust
  parse+serialize sides are at parity — so a large share of the row ratio is the
  richer deliverable the drop-in contract mandates, not engine speed. Three
  further non-obvious points:
  - **The WASI binding (`oxc-parser-wasm`) does _not_ wrap**, so `.program` is
    the raw unparsed JSON _string_ — `lib/oxc_wasm.ts` `JSON.parse`s it so the
    row materializes like the others. Before that fix it skipped the parse and
    looked artificially fast, even beating native oxc.
  - **Regex literals cost the opponents a `RegExp` compile the tsv rows skip.**
    `oxc-parser` and `yuku-parser` both set a regex `Literal`'s `value` to a real
    `RegExp`; tsv's wire is JSON, so it carries acorn's `"value": {}` beside the
    `regex: {pattern, flags}` object and a consumer constructs its own.
    `JSON.stringify` normalizes the two to the same bytes, so the payload
    comparison is unaffected — but the opponents do a little work per regex
    literal that tsv doesn't. Regex literals are sparse in the corpus (single
    digits per file at most), so the effect is well under the noise floor; it is
    recorded because it runs in tsv's favor, not because it moves a number.
  - **There is intentionally no `oxc-parser-lazy` row.** oxc's genuine lazy mode
    (`experimentalLazy` raw transfer, native-only — `rawTransferSupported()` is
    `false` on WASI) is _not_ a fast parse-only path: it eagerly copies the whole
    AST transfer buffer, so it's setup-dominated. Measured per-call on a 7.6 KB
    file: ~1.7 ms Node / ~2.1 ms Deno, vs ~0.7 ms eager-materialize and ~0.16 ms
    parse-only — lazy is _slower_ than the eager JSON path. Not a Deno artifact:
    the eager paths are byte-identical across Node and Deno (0.706/0.705 ms
    materialize, 0.165 ms parse-only), and only the lazy path is ~20% worse under
    Deno on top of an already-slow Node baseline. So `tsv-internal` /
    `tsv_wasm-internal` (parse-only, no JS materialization) have **no fair oxc
    counterpart** — oxc's JS API always serializes to cross into JS — and that
    asymmetry is left honest rather than papered over with a misleading row.
- **The `yuku-parser` rows need two corrections to be honest, and both are
  load-bearing.** yuku is payload-matched to oxc (span-only AST, same padding
  fields), so read it against `oxc-parser` / `tsv-json-no-locations` rather than
  `tsv-json`. **Both halves of that are measured, not inferred**: on a 15.7 KB TS
  file the forced tree is 1,318 plain objects at depth 24 with **zero accessor
  properties** anywhere — so nothing stays lazy behind `.program`, and a deep walk
  afterwards adds no measurable time (a lazily-decoded tree would pay exactly
  there) — and `JSON.stringify` of it is **128,567 chars against oxc's 128,570**,
  a payload ratio of 1.000. Its JS API carries two traps `lib/yuku.ts`
  `parse_yuku` defuses, reached by BOTH rows since one `YukuImplementation` drives
  both bindings:
  - **`parse()` is LAZY.** It returns memoized getters over the binary buffer the
    Zig side produced; the JS AST decodes only when `.program` is read. Forcing it
    costs **1.69x** (native) / **1.91x** (wasm) in the harness path — an unforced
    row would publish that much more throughput for a tree nobody built, and
    wouldn't be measuring the deliverable `oxc-parser` and `tsv-json` produce. The
    wrapper returns `result.program`; never "simplify" that to `return result`.
  - **The parser is ERROR-TOLERANT — it never throws.** An invalid file yields an
    empty AST plus `diagnostics`, so without reading them every file counts as
    accepted and the coverage row reads 100% regardless of what it parsed — the
    same fabricated-coverage failure the oxc WASI binding's consume-once `errors`
    getter produced. Caught here by construction and by `warn_variant_parity`,
    which pairs `yuku-parser` with `yuku-parser-wasm`. Only `severity: 'error'`
    rejects — treating a warning/hint as a failure would under-report coverage.

  Its options are pinned rather than defaulted, on the same rule the formatter
  rows follow: `sourceType: 'module'` (the goal tsv and acorn parse the perf
  corpus at — overridden per file when the harness threads a test262 goal), `lang:
  'ts'` (the corpus collapses `.js`/`.ts`, as tsv and the synthetic `file.ts`
  handed to oxc both do), `semanticErrors: false` (oxc's default; enabling it buys
  a second AST pass no opponent pays for), `attachComments: false` (payload match
  — neither oxc's `.program` nor tsv's wire AST carries comments), and
  `preserveParens: true`. That last is yuku's *and* oxc's default while acorn — and
  so tsv — effectively parses with it off; measured on this corpus it is
  immaterial (7–14 extra nodes out of ~5,600, inside the noise floor), so it is
  pinned to oxc's value to keep the two span-only rows like-for-like rather than
  re-baselining oxc's committed numbers over a rounding error. Because the module
  is consumed through a cast, `init()` **asserts the pins actually land** — a
  behavioral probe in the spirit of the `dprint` config-diagnostics check, since
  yuku reports nothing for an unrecognized option key. Only the two whose loss
  would be silent are probed (`lang`, `sourceType`); the other three match yuku's
  defaults, so a rename there is a no-op by construction. The two `sourceType`
  probes prove different things: `var await` must be REJECTED under the pinned
  options (pinning the default the perf path relies on, since `module` is also
  what a dropped key falls back to) and ACCEPTED under an explicit `sourceType:
  'script'` — only the second can catch an upstream rename.

  **One disclosed parser difference the goal probe turned up:** yuku's `script`
  goal is *permissive* about module syntax — `import`, `export`, and `import.meta`
  all parse cleanly at `sourceType: 'script'`, where tsv and acorn make them syntax
  errors. The goal lands correctly on the axis the harness threads it for (`await`
  is an ordinary identifier at `script`, reserved at `module`), and a script-goal
  positive carries no module syntax by definition — so this moves no published
  number. It does mean a yuku script-goal *accept* is a weaker claim than a tsv
  one, which would matter if the conformance surface ever graded negatives.

  **There is deliberately no `yuku-internal` row.** yuku's unforced `parse()` *is*
  a genuinely cheaper non-materializing mode — unlike oxc's `experimentalLazy`,
  which is setup-dominated — but it is not `tsv-internal`'s tier either: it has
  already serialized the AST into a binary buffer (and, in wasm, copied it out of
  linear memory) by the time it returns, where `tsv-internal` does no
  serialization at all. Publishing it beside `tsv-internal` would invite exactly
  the tier confusion the `-internal` rows exist to avoid.
- **One row is measured but not timed.** `rsvelte-fmt` is an accept rate with no
  timing, excluded from the timed loop, the group intersection, and the perf
  coverage invariant, so it moves no other number — see [Coverage-only
  rows](#coverage-only-rows).

## Implementations

Versions are read automatically from `benches/js/package.json` `dependencies` at
runtime (`lib/versions.ts`).

### Canonical (JS baseline)

`svelte` (parser, `svelte/compiler`) · `acorn` (JS parser base) ·
`@sveltejs/acorn-typescript` (TS extension for acorn) · `prettier` ·
`prettier-plugin-svelte`.

`canonical.ts` formats with a `filepath` hint (`file.ts` / `file.js` /
`file.svelte` / `file.css`) so prettier applies the same extension-specific
heuristics a real on-disk file gets — matching how `tsv_debug`'s sidecar invokes
prettier. Load-bearing on two axes:

- **`.ts` vs `.tsx`.** Without a filepath prettier can't tell them apart and
  force-adds the JSX-disambiguating trailing comma to single-type-param arrows
  (`<T,>`) that a real `.ts` run never emits — which once manufactured ~39 phantom
  corpus divergences against code tsv was formatting correctly.
- **`.js` vs `.ts` parser.** The corpus collapses `.js` and `.ts` into one
  `typescript` Language (tsv formats both through its TS path), but real
  prettier-on-`.js` uses the **babel** parser (preserves JSDoc `@type` casts) where
  prettier-on-`.ts` uses **typescript** (strips them). `format_async` takes the
  real source path and routes a `.js` file through `babel` so the oracle matches a
  real on-disk `.js` run — otherwise every `.js` file carrying a JSDoc cast reads
  as a phantom `jsdoc_type_cast_parens` divergence against tsv's (correct) uniform
  preservation. `corpus_compare_format.ts` passes `file.path` for this; the
  benchmark/smoke callers omit it and fall back to the synthetic `file.<ext>`.

### Alternative implementations

- **tsc (`typescript`)** — the TypeScript compiler's own parser; TypeScript, JS,
  parse-only, **conformance surface only**. Not a peer implementation but the
  DEFINITION the other TS rows are measured against, which is why it earns a row on
  the verdict surface and none on the throughput one. Two properties shape how it is
  driven (`lib/tsc.ts`): its parser is **error-recovering** — `createSourceFile`
  never throws, so an accept is defined as `parseDiagnostics.length === 0`, and a
  row scoring "didn't throw" would report a fabricated 100% — and it **infers** the
  parse goal from the file rather than accepting one, so the conformance corpus's
  declared `goal` is ignored for this row alone. 6.x is the last JS implementation;
  7.x is the Go port, whose npm package ships a binary with no in-process parser API.
- **oxc-parser (NAPI)** — fast TypeScript parser; TypeScript, JS.
- **oxfmt (NAPI)** — fast formatter; TypeScript, JS, CSS, Svelte (experimental).
  As of 0.57 the native Rust formatter handles **JS/TS *and* CSS**; only **Svelte**
  routes through a JS-side fallback into oxfmt's **bundled prettier**
  (`dist/apis-*.js` `formatFile` → `prettier.format`) plus a bundled svelte plugin,
  with `prettier-plugin-oxfmt` formatting the embedded `<script>` through the
  native `jsTextToDoc`. So `tsv` vs `oxfmt` is a native-vs-native engine race on
  **TypeScript AND CSS**; only the **svelte** oxfmt row is (mostly) a
  prettier-pipeline number in oxfmt packaging — read that one ratio accordingly.
  The report corroborates: oxfmt ≈ prettier on svelte (~1x), but ~6x prettier on
  css and ~14x on TS.
- **biome (WASM)** — formatter/linter; TypeScript, JS, CSS, and Svelte (via
  biome's experimental HTML-superset support, `html.experimentalFullSupportEnabled`;
  it formats the template **and** the embedded `<script>`/`<style>`, so it's
  comparable work to prettier-plugin-svelte / tsv, just on an experimental path).
- **dprint (WASM)** — formatter; **TypeScript, JS only**. This is the engine
  **`deno fmt` runs** for TS/JS (`dprint-plugin-typescript`), loaded in-process as
  its Wasm plugin. Deliberately NOT a `deno fmt` subprocess row: that would exist
  only under Deno (against the three-runtime design) and would time process spawn +
  IPC rather than format work, cold on every call against warm opponents. The row
  is named for what it measures — the engine — not the CLI, whose wrapping (config
  discovery, file IO, its own CSS/HTML/markdown plugins) is out of scope.
  `@dprint/typescript` matches `ts,tsx,js,jsx,mjs,cjs,mts,cts` and **rejects CSS and
  Svelte outright** (verified), so unlike oxfmt/biome it contributes no css or
  svelte row; dprint's CSS (malva) and HTML plugins are separate Wasm plugins, not
  wired up. Config is asserted to LAND: `lib/dprint.ts` fails init if
  `getConfigDiagnostics()` is non-empty, since dprint reports an unrecognized key as
  a diagnostic rather than throwing — without that check a renamed key would
  silently leave an option at its default and skew the row.
- **yuku-parser (NAPI) / @yuku-parser/wasm (WASM)** — a JS/TS parser written in
  Zig; **TypeScript, JS only** — no Svelte, no CSS, no formatter, so it contributes
  two rows to `parse/typescript` and nothing else. One engine behind two bindings,
  versioned in lockstep (bump both together). Its default AST is span-only and
  padded exactly like oxc's (`decorators: []` / `typeAnnotation: null` / `optional:
  false`, no per-node `loc`). That payload match, and the two JS-API traps
  `lib/yuku.ts` must defuse — `parse()` is **lazy**, the parser is
  **error-tolerant** — are in [Fairness caveats](#fairness-caveats). **One
  `YukuImplementation` drives both bindings** (constructed twice, the row name
  selecting the specifier): they expose the identical module surface, so a wrapper
  per binding would be a copy free to drift, which is exactly how the oxc WASI row
  broke. That's the difference from `oxc.ts`/`oxc_wasm.ts`, whose two packages
  genuinely differ. Unlike oxc's wasi binding the wasm package declares no
  `cpu`/`os`, so it installs as an ordinary dep everywhere and needs no
  force-fetch. The **N-API row is excluded from the conformance surface** — its
  native binding faults the host process on that corpus's escaped-identifier
  fixtures (../benches/js/CLAUDE.md §Known Issues); the wasm row carries the engine
  there, and both rows run on perf.
- **rsvelte-fmt (native binary)** — the other Rust-native Svelte formatter;
  **Svelte only**, and **coverage-only**. See below.

### Coverage-only rows

A coverage-only row (`BenchmarkTask.coverage_only`) is an impl the pre-flight runs
over the whole corpus — so its accept rate is measured and published — but that the
timed loop never touches. `rsvelte-fmt` is the only one.

**Why it can't be timed.** It ships no in-process format API in any package: the
npm package is a Node launcher that `spawnSync`s a prebuilt binary, and the sibling
`@rsvelte/vite-plugin-svelte-native` N-API addon is the *compiler* (`compile` /
`parse` / `svelte2tsx`, no format export). Driving it means a process per file.
Measured on a ~5 KB `.svelte` file the binary costs ~2.4 ms of which ~1.3 ms is the
bare spawn floor (`--version`), against tsv's ~0.09 ms in-process — a timed row
would rank `fork`/`exec` and report it as an engine gap. Same objection that keeps
`deno fmt` out as a subprocess row, with one difference: dprint had an in-process
engine to measure instead, and rsvelte-fmt has none, so the choice was a disclosed
non-number or no row.

**The shape that DOES suit a CLI already exists.** The separate hyperfine
comparison (`../oxc-bench-formatter`, published on tsv.fuz.dev) benches
rsvelte-fmt end-to-end — process spawn, discovery, IO, each tool's own
parallelism, plus peak memory — on a third-party `.svelte` corpus. That's where its
speed numbers live; this row answers only "what does it accept."

**Svelte only.** Its `.ts`/`.js` path is `oxc_formatter` and its CSS path
`oxc_formatter_css` — the same engine as the `oxfmt` row, by its own
`--no-native-js` / `--no-native-css` escape-hatch docs. A ts or css row would
re-measure oxfmt's acceptance through a spawn, adding no information.

**What the flag must be honored by** — four places in `bench.ts`, each
load-bearing:

1. The **timed loop** skips it (`group_setups` stores the timed tasks only).
2. The per-group **intersection** skips it — otherwise a file only it rejects would
   drop out of the set every real row is timed on, letting a non-participant move
   the published numbers.
3. The perf **100%-coverage hard-fail** skips it: that invariant governs tools whose
   throughput is published, and sub-100% here is the measurement rather than an
   erosion of one.
4. Its report row is **synthesized** (`build_coverage_entries(true)`) with null
   timing and `files_iterated: null`, since the bench library produced no result for
   it — without that its coverage would vanish for not being a speed.

The markdown renders it as a per-group `**Coverage-only (not timed):**` line
carrying its reason inline, so an untimed name in a throughput report is never
unexplained.

**Setup.** The binary comes from `@rsvelte/fmt`'s platform `optionalDependency` and
is exec'd directly, not through the published Node launcher (which would add a Node
cold start measuring npm packaging). `lib/rsvelte.ts` probes `--version` at init, so
a present-but-unexecutable package fails as a broken setup instead of reading as an
honest 0%. Under Deno the spawn needs `--allow-run`; all five published platform
paths are listed on `bench:deno:run` and `smoke` (see the `//rsvelte-allow-run` note
in `deno.json`).

### OXC package details

**oxc-parser** ships three package types:

- **Main** (`oxc-parser`): JS wrapper with platform detection; contains
  `src-js/wasm.js` for direct WASM usage. `NAPI_RS_FORCE_WASI` forces WASM.
- **Native bindings** (`@oxc-parser/binding-{platform}`): 20 platform-specific
  `.node` files, listed as `optionalDependencies` of main.
- **WASM binding** (`@oxc-parser/binding-wasm32-wasi`): official WASI build, also
  an optional dependency of main — it ships alongside native, not as a separate
  product. Depends on `@napi-rs/wasm-runtime` → `@emnapi/runtime`, `@emnapi/core`,
  `@tybys/wasm-util`. (`@oxc-parser/wasm` exists on npm but is **deprecated**.)
  Its default CJS entry uses `node:wasi`, which Deno doesn't support, so
  `lib/oxc_wasm.ts` imports the browser entry
  (`…/parser.wasi-browser.js`, `fetch()` + `WebAssembly` via
  `@napi-rs/wasm-runtime`) per runtime.

**oxfmt** ships native bindings only: main (`oxfmt`, a JS wrapper bundling Prettier
internals, depending on `tinypool` for the CLI only) and `@oxfmt/{platform}` (8
variants). **No WASM variant exists.** Svelte support is experimental (added in
v0.49); the bench enables it and lets the per-file try/catch + effective-corpus
report quantify coverage.

## Binary size reporting

Benchmark output includes a binary/WASM size comparison. Each row reports **raw
on-disk size** plus **gzipped size** (≈ npm-tarball wire size), grouped by kind
(WASM vs native) with ratios relative to `tsv` for both. Implementation:
`lib/binary_sizes.ts`; JSON output carries a per-entry `gzip_bytes: number | null`.

- **`tsv`**: native FFI (`.so`/`.dylib`/`.dll`), N-API addon (`.node`), and WASM.
  The FFI side ships three rows from one `tsv_ffi` crate via its `format`/`parse`
  features (matching the three WASM rows): full `libtsv_ffi` (`target/release`,
  both features — what the perf rows load), `tsv format (ffi)`
  (`target/ffi-format/release`, no convert layer — scope-matched to `oxfmt
  (napi)`), and `tsv parse (ffi)` (`target/ffi-parse/release`, printers dropped —
  scope-matched to `oxc-parser (napi)`). `tsv (napi)` is the Node/Bun native path.
  Native-kind labels name the binding (`ffi`/`napi`), not just "native". `deno task
  bench` builds all of them; subset rows are omitted if those builds haven't run.
- **biome**: WASM from node_modules.
- **dprint**: WASM (`@dprint/typescript`'s `plugin.wasm`). TS/JS-only scope, so it
  size-compares against the format-only tsv builds.
- **oxc-parser**: N-API binding + WASM (`binding-wasm32-wasi`).
- **oxfmt**: N-API binding (no WASM variant).
- **yuku-parser**: N-API binding + WASM, both parse-only artifacts — pair each
  against the parse-only tsv build (`tsv parse (ffi)` / `tsv_parse_wasm`); against a
  bundle carrying the printers it would size a scope difference and read as an
  engine one. As for `oxc-parser` and `dprint`, that pairing is the reader's to
  make: the emitted `vs tsv` ratio anchors every row on the full build.
- **rsvelte-fmt**: the standalone executable from its platform package — the one
  native row not scope-matched to a tsv artifact (it carries a CLI plus the whole
  oxc formatter for JS/TS/CSS beside its Svelte engine, where `tsv (ffi)` is a bare
  library). Read it as "what that tool ships."

The combined `oxc-parser+oxfmt (napi)` row sums both raw and gzipped sizes from the
parts; the gzipped sum slightly overstates wire size because the streams don't share
a dictionary, but it matches npm's two-tarball reality.

Compression is `gzip -c` (system default level 6), matching
`scripts/patch_npm_package.ts` — what `tar | gzip` and most npm publishers produce.
The tighter numbers cited in some perf-doc histories used `gzip -9` and run ~2–3%
smaller; both are recorded in [performance.md](performance.md) for the WASM
binaries. The gzipped column shows `—` when `gzip` isn't on PATH (raw size still
collects); `bench:deno:run` needs `--allow-run=git,gzip`, and gzip runs via
`node:child_process` `execFile` (portable across runtimes).

## Updating dependencies

**How resolution works on any machine.** `benches/js/package.json` pins the npm dep
versions (the single source of truth, consumed by both runtimes) and
`package-lock.json` pins their integrity. **Run `deno task bench:install`** to
populate `node_modules` (`npm install` plus the force-fetch of the oxc wasi
binding). Deno reads that `node_modules` via `"nodeModulesDir": "manual"`; Node
reads it directly. The Rust artifacts the bench builds (`tsv_ffi`, `tsv_napi`,
`tsv_wasm`) are pinned via `Cargo.lock`. A plain `npm install` prunes the oxc wasi
binding — re-run `bench:install`.

**Routine refresh** (alternative impls + infra — no fixture impact):

```bash
cd benches/js && npm outdated   # current vs latest
# bump the version in benches/js/package.json, then:
deno task bench:install   # re-install at the new pins (+ re-fetch the oxc wasi binding)
deno task smoke           # confirm every impl still loads + formats (40 checks)
deno check --config benches/js/deno.json benches/js/bench.ts benches/js/lib/biome.ts benches/js/lib/dprint.ts benches/js/lib/yuku.ts
deno task bench           # regenerate report.{deno,node,bun}.* + combined report.{json,md}
# commit package.json + package-lock.json + results/report.*
```

These packages are free to bump independently — they're measured against, not baked
into fixtures. A **major** bump (e.g. `@biomejs/js-api` 4→6) can change a package's
*type* surface without breaking the runtime path smoke exercises, so the `deno
check` step is the guard for those.

⚠ **The oxc wasm binding is not a regular dep.** It's pure-wasm but its metadata
declares `cpu: wasm32`, so it lives in neither `dependencies` nor
`optionalDependencies` (both break or get pruned). `install_deps.ts` force-fetches
it at the `oxc-parser` version (oxc ships all bindings in lockstep), so bumping
`oxc-parser` carries it automatically. `binary_sizes.ts` reads it from
`node_modules` (flat, no version dir).

### Canonical baseline is coupled

**Do NOT bump it as routine.** The five canonical packages (`prettier`, `svelte`,
`acorn`, `@sveltejs/acorn-typescript`, `prettier-plugin-svelte`) are also pinned, as
literals, in `crates/tsv_debug/src/deno/sidecar.ts` — the sidecar that generates
every fixture's `expected.json` and `output_prettier.svelte`. The two pin sets
**must stay identical**: the bench has to measure against the same
parser/formatter that defines fixture correctness. Agreement across all pin sites
(sidecar `VERSIONS` + its `npm:` imports, `benches/js/package.json`, actor.rs's
acorn import-map pin) is enforced by `deno task pins:audit`
(`scripts/check_canonical_pins.ts --pins`, gated in `deno task check`).

**Checkout alignment** is the same script's other mode (`deno task
pins:audit:checkouts`), gated in `deno task conformance` and reported by `doctor`: a
present `../svelte` / `../acorn-typescript` checkout whose version differs from its
pin FAILS (absent checkouts are skipped, so a machine without the clones still
passes). Align the checkout to the pinned tag, or bump the pins deliberately. The
two modes are split because they assert different KINDS of fact (full reference:
[audits.md §Canonical-Pin Agreement Audit](audits.md#canonical-pin-agreement-audit-pinsaudit)
and [§Checkout-Alignment Audit](audits.md#checkout-alignment-audit-pinsauditcheckouts)): pin agreement is a
**repo** fact that invalidates the fixture grading `cargo test` does, so it gates the
committed tree; alignment is an **environment** fact about suites nothing in `deno
task check` reads, so a skew there would halt that chain without invalidating a
single committed-tree verdict. `../prettier` is not gated (its suites' oracle output
is computed live per file and the checkout rides `-dev` versions); `doctor` reports
it.

Bumping any of the five re-baselines the entire fixture corpus. Do it deliberately:
edit `package.json` and `sidecar.ts` in lockstep (the `//canonical-sync` note in
package.json restates this), run `deno task fixtures:update`, and review the
resulting churn.

**Fixture churn is only one of three ways an oracle bump lands, and the third is
ungated.** Read each upstream commit's source diff *and* the regression fixture it
ships, then run those constructs through tsv, the new oracle, and real tsc —
comparing wire **key order**, not just accept/reject:

- an upstream **bug fix** retires a tsv correction — divergence fixtures collapse,
  and `fixtures:update` shows you exactly which;
- an upstream **widening** exposes a tsv over-rejection — `conformance:ts-fixtures`
  names it, because the fix ships its own test-suite entry (which also moves that
  gate's `scanned`/`both_accept` pins, re-measured per its update ritual);
- an upstream **loosening** converts a construct both sides used to reject into a
  live divergence. **Nothing sees this** — there is no suite entry for a rejection
  that merely stopped happening, so no gate has an input for it. The only way to
  find it is a hand sweep of the fix's feature area.

A bump can equally make a construct newly *reachable* in tsv, which is how one trips
`gaps:audit` with a NEW comment-gap shape. Treat that as the real drop or
double-print it reports, not as a prompt to re-pin: the fixture didn't find a
pre-existing bug, the parser change put a printer seam in reach for the first time.

**Then grep the repo for the OLD version string** — `rg '<old>' --glob
'!benches/js/results/**'`. Nothing gates this, and it is the step that gets skipped.
Prose that restates the pin ("pinned at svelte X", "valid at the X pin", "the pinned
oracle (svelte X) throws") is a duplicate of a value that just moved, and it goes
silently wrong; one 5.56.4 → 5.56.8 bump left five such claims behind across `docs/`
and two crates. A **past**-version mention is different and stays true — "Prettier
3.9.5 tightened it", a fixture README explaining which release changed a behavior —
so this cannot be a lint, only a read. Prefer pointing at `sidecar.ts`'s `VERSIONS`
over restating the number.

Two things a bump can invalidate that `deno task check` does **not** cover, because
both are sidecar-dependent: `deno task compile:validation` (the ratchet's
`ORACLE-ERROR` line is a claim about oracle behavior, explicitly held "until the pin
moves") and `deno task bench:harvest` (`SVELTE_REJECTS_PIN` counts what the oracle
rejects). Run both. Svelte-source line anchors in
[checklist_svelte_compiler.md](checklist_svelte_compiler.md) are the third — nothing
gates a line number, so spot-check a few.
