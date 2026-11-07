# tsv conformance benchmark results (parse)

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** conformance — fixtures-only corpus (disjoint from perf; a suite with a validity oracle or harness filtered to what it calls valid), parse groups only; per-tool Coverage lines only (coverage-only run — timed throughput skipped)

**Date:** 2026-11-07T07:38:20.031Z — tsv 0.8.1 (872e4d3a)

**Corpus:** 4689 Svelte (1.1 MB), 53323 TypeScript (70.8 MB), 22450 CSS (7.7 MB) — 80462 files, 79.6 MB total

**Native build:** the native tsv rows (N-API, under Node/Bun) measure the N-API addon's profile-guided build, trained on the corpus snapshot's `origin: own` collections — whose overlap with this run’s corpus is not computable here (no snapshot collection in it) (zzz, fuz_app, fuz_blog, fuz_code, fuz_css, docs.fuz.dev, fuz_repos, fuz_template, fuz_ui, fuz_util, mdz, gro, svelte-docinfo, tsv.fuz.dev, ryanatkn.com, earbetter, cosmicplayground). The N-API addon's is the shipped addon's: the addon itself — parse on every file and format --jobs 1 over the tree through the @fuzdev/tsv loader’s cli.js, the type strip and import records in process — the training the published addon gets. WASM rows are not profile-guided.

**Sources:** ../prettier-plugin-svelte/test (318), ../prettier/tests/format/typescript (591), ../prettier/tests/format/js (756), ../prettier/tests/format/css (139), ../prettier/tests/format/html (82), ../svelte/packages/svelte/tests (4613), benches/js/.cache/wpt_css (22127), benches/js/.cache/test262_files.json (43739), benches/js/.cache/ts_repo_files.json (8097)

**Excluded by cache:** canonical-reject (145), prettier-jsx (39)

**Versions:** svelte@5.57.3, acorn@8.19.0, acorn-typescript@1.0.13, prettier@3.9.10, prettier-plugin-svelte@4.1.1, oxc-parser@0.156.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.75.0, yuku-parser@0.19.0, @biomejs/wasm-bundler@2.6.4, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, dprint-plugin-markup@0.27.6, postcss@8.5.32, @rsvelte/fmt@0.7.25, @rsvelte/vite-plugin-svelte-native@0.3.17 (targets svelte@5.57.1), @swc/core@1.16.21, typescript@6.0.3

**Excluded here:** yuku-parser (N-API) — its native binding faults the host process on this corpus (test262 escaped-identifier fixtures), so it cannot be measured against it. The WASM binding runs the same engine and carries the row; both are measured on the perf corpus.

**Excluded here:** tsv+locations — the default span-only parse plus `loc` reconstructed in JS (`{locations: true}`), a consumer-cost row measured on the perf corpus. The parse it runs is tsv’s, so its coverage is that row’s.

**Excluded here:** tsv-wasm+locations — the same consumer-cost row over the WASM binding; its coverage is tsv-wasm’s.

**Added here:** tsc — the TypeScript compiler’s own parser, a verdict rather than a speed, so it carries no row on the throughput surface. Its parser is error-recovering (`createSourceFile` never throws), so an accept means zero `parseDiagnostics`. On the tsc corpus it is the ORACLE that selected those files — 100% by construction, like svelte/compiler on the Svelte set — and an independent parser on every other source, which is what the per-source tables below are for. Coverage counts accepts and so cannot show over-acceptance; that axis is `deno task ts-repo:over-acceptance`.

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

**Coverage:** svelte/compiler 4689/4689 (100%), tsv 4689/4689 (100%), tsv-wasm 4689/4689 (100%), tsv-internal 4689/4689 (100%), tsv-wasm-internal 4689/4689 (100%), rsvelte-parse 4689/4689 (100%), rsvelte-parse-skip-expr-loc 4689/4689 (100%)

## parse/typescript

**Coverage:** acorn-typescript 52614/53323 (98%), tsv 53217/53323 (99%), tsv-wasm 53217/53323 (99%), tsv-internal 53217/53323 (99%), tsv-wasm-internal 53217/53323 (99%), oxc-parser 53167/53323 (99%), oxc-parser-wasm 53169/53323 (99%), oxc-parser-raw 53166/53323 (99%), tsc 53247/53323 (99%), yuku-parser-wasm 53169/53323 (99%), swc 52797/53323 (99%)

## parse/css

**Coverage:** svelte/compiler 22243/22450 (99%), tsv 22296/22450 (99%), tsv-wasm 22296/22450 (99%), tsv-internal 22296/22450 (99%), tsv-wasm-internal 22296/22450 (99%), postcss 22362/22450 (99%)

### parse/svelte by corpus source

| Source | Files | svelte/compiler | tsv | tsv-wasm | tsv-internal | tsv-wasm-internal | rsvelte-parse | rsvelte-parse-skip-expr-loc |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `../prettier-plugin-svelte/test` | 318 | 318 (100%) | 318 (100%) | 318 (100%) | 318 (100%) | 318 (100%) | 318 (100%) | 318 (100%) |
| `../prettier/tests/format/html` | 82 | 82 (100%) | 82 (100%) | 82 (100%) | 82 (100%) | 82 (100%) | 82 (100%) | 82 (100%) |
| `../svelte/packages/svelte/tests` | 4289 | 4289 (100%) | 4289 (100%) | 4289 (100%) | 4289 (100%) | 4289 (100%) | 4289 (100%) | 4289 (100%) |

### parse/typescript by corpus source

| Source | Files | acorn-typescript | tsv | tsv-wasm | tsv-internal | tsv-wasm-internal | oxc-parser | oxc-parser-wasm | oxc-parser-raw | tsc | yuku-parser-wasm | swc |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `../prettier/tests/format/typescript` | 591 | 515 (87%) | 581 (98%), 1 at script | 581 (98%), 1 at script | 581 (98%), 1 at script | 581 (98%), 1 at script | 569 (96%) | 570 (96%) | 569 (96%) | 590 (99%) | 581 (98%), 1 at script | 562 (95%), 1 at script |
| `../prettier/tests/format/js` | 756 | 709 (93%), 34 at script | 740 (97%), 11 at script | 740 (97%), 11 at script | 740 (97%), 11 at script | 740 (97%), 11 at script | 750 (99%), 1 at script | 750 (99%), 1 at script | 750 (99%), 1 at script | 743 (98%) | 751 (99%), 1 at script | 736 (97%), 14 at script |
| `../svelte/packages/svelte/tests` | 140 | 140 (100%) | 140 (100%) | 140 (100%) | 140 (100%) | 140 (100%) | 140 (100%) | 140 (100%) | 140 (100%) | 140 (100%) | 140 (100%) | 140 (100%) |
| `benches/js/.cache/test262_files.json` | 43739 | 43478 (99%) | 43739 (100%) | 43739 (100%) | 43739 (100%) | 43739 (100%) | 43725 (99%) | 43725 (99%) | 43725 (99%) | 43677 (99%) | 43705 (99%) | 43425 (99%) |
| `benches/js/.cache/ts_repo_files.json` | 8097 | 7772 (95%) | 8017 (99%) | 8017 (99%) | 8017 (99%) | 8017 (99%) | 7983 (98%) | 7984 (98%) | 7982 (98%) | 8097 (100%) | 7992 (98%) | 7934 (97%) |

### parse/css by corpus source

| Source | Files | svelte/compiler | tsv | tsv-wasm | tsv-internal | tsv-wasm-internal | postcss |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `../prettier/tests/format/css` | 139 | 118 (84%) | 122 (87%) | 122 (87%) | 122 (87%) | 122 (87%) | 139 (100%) |
| `../svelte/packages/svelte/tests` | 184 | 184 (100%) | 184 (100%) | 184 (100%) | 184 (100%) | 184 (100%) | 184 (100%) |
| `benches/js/.cache/wpt_css` | 22127 | 21941 (99%) | 21990 (99%) | 21990 (99%) | 21990 (99%) | 21990 (99%) | 22039 (99%) |

**The test262 source is tsv-scope-filtered, and it favors tsv.** The cache the `test262` source reads is the expected-positive subset of the tests tsv’s own runner GRADES — `test262 --emit-manifest` (`crates/tsv_debug/src/test262/`) drops the tests outside tsv’s scope before the split, every Annex B `noStrict` positive among them, a grammar tsv declines as a non-browser host and that acorn, oxc, swc, tsc and yuku all parse. So tsv reads 100% on that source by construction, the way tsc does on the tsc corpus and svelte/compiler on the Svelte set, and a rival’s number there is its rate on tsv’s slice, not on test262. The positive/negative split itself is tool-neutral; the graded subset it starts from is not.

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv-format-wasm | 2.8 MB | 1.1 MB | 0.9x | 0.9x |
| tsv-parse-wasm | 1.1 MB | 427.1 KB | 0.3x | 0.4x |
| tsv-wasm | 3.1 MB | 1.2 MB | — | — |
| biome (wasm) | 64.1 MB | 16.7 MB | 20.5x | 14.0x |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.3x | 1.0x |
| oxc-parser (wasm) | 1.5 MB | 481.4 KB | 0.5x | 0.4x |
| yuku-parser (wasm) | 718.6 KB | 209.4 KB | 0.2x | 0.2x |
| malva (wasm) | 1.5 MB | 414.0 KB | 0.5x | 0.3x |
| markup_fmt (wasm) | 2.0 MB | 558.7 KB | 0.6x | 0.5x |
| tsv (ffi) | 4.8 MB | 2.4 MB | 0.9x | 0.9x |
| tsv format (ffi) | 3.6 MB | 1.7 MB | 0.7x | 0.7x |
| tsv parse (ffi) | 1.6 MB | 741.2 KB | 0.3x | 0.3x |
| tsv (napi) | 5.3 MB | 2.6 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.3 MB | 4.7 MB | 2.1x | 1.8x |
| oxc-parser (napi) | 2.2 MB | 906.0 KB | 0.4x | 0.3x |
| oxfmt (napi) | 9.1 MB | 3.8 MB | 1.7x | 1.4x |
| yuku-parser (napi) | 692.2 KB | 287.7 KB | 0.1x | 0.1x |
| rsvelte-fmt (binary) | 9.1 MB | 3.6 MB | 1.7x | 1.4x |
| rsvelte compiler (napi) | 17.8 MB | 7.5 MB | 3.3x | 2.9x |
| swc (napi) | 10.7 MB | 10.3 MB | 2.0x | 3.9x |
| svelte + acorn-typescript parsers (js bundle) | 500.5 KB | 124.9 KB | 0.2x | 0.1x |
| prettier + svelte plugin (js bundle) | 2.2 MB | 567.4 KB | 0.7x | 0.5x |
| prettier + parsers (js bundle) | 2.2 MB | 567.5 KB | 0.7x | 0.5x |

_`vs tsv` divides native rows by `tsv (napi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm and js-bundle rows by `tsv-wasm`, the portable artifact a JS bundle stands beside. Of tsv's native rows, `tsv (ffi)` and `tsv (napi)` are the profile-guided build the native rows time; `tsv format (ffi)` and `tsv parse (ffi)` are plain builds, sized only. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes. The `js bundle` rows are SYNTHESIZED, not shipped: the canonical tools publish no single artifact, so each is a minified, tree-shaken bundle of the minimum one capability needs (`benches/js/size_bundles/`), built by `deno bundle` during this run._

## Skipped Files

1422 files skipped, 2177 unique file+error combinations — Svelte 0, TypeScript 1199, CSS 223 files.

**Per-benchmark skip counts:**
- parse/typescript: acorn-typescript: 709
- parse/typescript: swc: 526
- parse/css: svelte/compiler: 207
- parse/typescript: oxc-parser-raw: 157
- parse/typescript: oxc-parser: 156
- parse/typescript: oxc-parser-wasm: 154
- parse/typescript: yuku-parser-wasm: 154
- parse/css: tsv: 154
- parse/css: tsv-wasm: 154
- parse/css: tsv-internal: 154
- parse/css: tsv-wasm-internal: 154
- parse/typescript: tsv: 106
- parse/typescript: tsv-wasm: 106
- parse/typescript: tsv-internal: 106
- parse/typescript: tsv-wasm-internal: 106
- parse/css: postcss: 88
- parse/typescript: tsc: 76

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
