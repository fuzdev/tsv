# tsv conformance benchmark results (parse)

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** conformance — fixtures-only corpus (disjoint from perf; Svelte set minus svelte/compiler-rejected files), parse groups only; per-tool Coverage lines only (coverage-only run — timed throughput skipped)

**Date:** 2026-09-15T17:48:40.955Z — tsv 0.3.0 (29a107e9)

**Corpus:** 4571 Svelte (1.1 MB), 53952 TypeScript (70.9 MB), 22642 CSS (7.7 MB) — 81165 files, 79.7 MB total

**Sources:** ../prettier-plugin-svelte/test (318), ../prettier/tests/format/typescript (795), ../prettier/tests/format/js (1106), ../prettier/tests/format/css (228), ../prettier/tests/format/html (84), ../svelte/packages/svelte/tests (4488), benches/js/.cache/wpt_css (22310), benches/js/.cache/test262_files.json (43739), benches/js/.cache/ts_repo_files.json (8097)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2, typescript@6.0.3

**Excluded here:** yuku-parser (N-API) — its native binding faults the host process on this corpus (test262 escaped-identifier fixtures), so it cannot be measured against it. The WASM binding runs the same engine and carries the row; both are measured on the perf corpus.

**Added here:** tsc — the TypeScript compiler’s own parser, a verdict rather than a speed, so it carries no row on the throughput surface. Its parser is error-recovering (`createSourceFile` never throws), so an accept means zero `parseDiagnostics`. On the tsc corpus it is the ORACLE that selected those files — 100% by construction, like svelte/compiler on the Svelte set — and an independent parser on every other source, which is what the per-source tables below are for. Coverage counts accepts and so cannot show over-acceptance; that axis is `deno task ts-repo:over-acceptance`.

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

**Coverage:** svelte/compiler 4571/4571 (100%), tsv-json 4571/4571 (100%), tsv-wasm-json 4571/4571 (100%), tsv-json-no-locations 4571/4571 (100%), tsv-wasm-json-no-locations 4571/4571 (100%), tsv-internal 4571/4571 (100%), tsv-wasm-internal 4571/4571 (100%), rsvelte-parse 4571/4571 (100%), rsvelte-parse-skip-expr-loc 4571/4571 (100%)

## parse/typescript

**Coverage:** acorn-typescript 53043/53952 (98%), tsv-json 53670/53952 (99%), tsv-wasm-json 53670/53952 (99%), tsv-json-no-locations 53670/53952 (99%), tsv-wasm-json-no-locations 53670/53952 (99%), tsv-internal 53670/53952 (99%), tsv-wasm-internal 53670/53952 (99%), oxc-parser 53642/53952 (99%), oxc-parser-wasm 53644/53952 (99%), tsc 53730/53952 (99%), yuku-parser-wasm 53620/53952 (99%), swc 53074/53952 (98%)

## parse/css

**Coverage:** svelte/compiler 22402/22642 (98%), tsv-json 22457/22642 (99%), tsv-wasm-json 22457/22642 (99%), tsv-internal 22457/22642 (99%), tsv-wasm-internal 22457/22642 (99%), postcss 22534/22642 (99%)

### parse/svelte by corpus source

| Source | Files | svelte/compiler | tsv-json | tsv-wasm-json | tsv-json-no-locations | tsv-wasm-json-no-locations | tsv-internal | tsv-wasm-internal | rsvelte-parse | rsvelte-parse-skip-expr-loc |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `../prettier-plugin-svelte/test` | 318 | 318 (100%) | 318 (100%) | 318 (100%) | 318 (100%) | 318 (100%) | 318 (100%) | 318 (100%) | 318 (100%) | 318 (100%) |
| `../prettier/tests/format/html` | 84 | 84 (100%) | 84 (100%) | 84 (100%) | 84 (100%) | 84 (100%) | 84 (100%) | 84 (100%) | 84 (100%) | 84 (100%) |
| `../svelte/packages/svelte/tests` | 4169 | 4169 (100%) | 4169 (100%) | 4169 (100%) | 4169 (100%) | 4169 (100%) | 4169 (100%) | 4169 (100%) | 4169 (100%) | 4169 (100%) |

### parse/typescript by corpus source

| Source | Files | acorn-typescript | tsv-json | tsv-wasm-json | tsv-json-no-locations | tsv-wasm-json-no-locations | tsv-internal | tsv-wasm-internal | oxc-parser | oxc-parser-wasm | tsc | yuku-parser-wasm | swc |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `../prettier/tests/format/typescript` | 795 | 716 (90%) | 781 (98%) | 781 (98%) | 781 (98%) | 781 (98%) | 781 (98%) | 781 (98%) | 768 (96%) | 769 (96%) | 791 (99%) | 779 (97%) | 758 (95%) |
| `../prettier/tests/format/js` | 1106 | 862 (77%) | 920 (83%) | 920 (83%) | 920 (83%) | 920 (83%) | 920 (83%) | 920 (83%) | 935 (84%) | 935 (84%) | 950 (85%) | 937 (84%) | 908 (82%) |
| `../prettier/tests/format/css` | 78 | 78 (100%) | 78 (100%) | 78 (100%) | 78 (100%) | 78 (100%) | 78 (100%) | 78 (100%) | 78 (100%) | 78 (100%) | 78 (100%) | 78 (100%) | 78 (100%) |
| `../svelte/packages/svelte/tests` | 137 | 137 (100%) | 137 (100%) | 137 (100%) | 137 (100%) | 137 (100%) | 137 (100%) | 137 (100%) | 137 (100%) | 137 (100%) | 137 (100%) | 137 (100%) | 137 (100%) |
| `benches/js/.cache/test262_files.json` | 43739 | 43478 (99%) | 43739 (100%) | 43739 (100%) | 43739 (100%) | 43739 (100%) | 43739 (100%) | 43739 (100%) | 43725 (99%) | 43725 (99%) | 43677 (99%) | 43698 (99%) | 43261 (98%) |
| `benches/js/.cache/ts_repo_files.json` | 8097 | 7772 (95%) | 8015 (98%) | 8015 (98%) | 8015 (98%) | 8015 (98%) | 8015 (98%) | 8015 (98%) | 7999 (98%) | 8000 (98%) | 8097 (100%) | 7991 (98%) | 7932 (97%) |

### parse/css by corpus source

| Source | Files | svelte/compiler | tsv-json | tsv-wasm-json | tsv-internal | tsv-wasm-internal | postcss |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `../prettier/tests/format/css` | 150 | 120 (80%) | 124 (82%) | 124 (82%) | 124 (82%) | 124 (82%) | 142 (94%) |
| `../svelte/packages/svelte/tests` | 182 | 181 (99%) | 181 (99%) | 181 (99%) | 181 (99%) | 181 (99%) | 182 (100%) |
| `benches/js/.cache/wpt_css` | 22310 | 22101 (99%) | 22152 (99%) | 22152 (99%) | 22152 (99%) | 22152 (99%) | 22210 (99%) |

**The test262 source is tsv-scope-filtered, and it favors tsv.** The cache the `test262` source reads is the expected-positive subset of the tests tsv’s own runner GRADES — `test262 --emit-manifest` (`crates/tsv_debug/src/test262/`) drops the tests outside tsv’s scope before the split, every Annex B `noStrict` positive among them, a grammar tsv declines as a non-browser host and that acorn, oxc, swc, tsc and yuku all parse. So tsv reads 100% on that source by construction, the way tsc does on the tsc corpus and svelte/compiler on the Svelte set, and a rival’s number there is its rate on tsv’s slice, not on test262. The positive/negative split itself is tool-neutral; the graded subset it starts from is not.

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv-format-wasm | 2.5 MB | 928.4 KB | 0.9x | 0.9x |
| tsv-parse-wasm | 995.6 KB | 384.6 KB | 0.4x | 0.4x |
| tsv-wasm | 2.8 MB | 1.0 MB | — | — |
| biome (wasm) | 44.6 MB | 11.4 MB | 15.9x | 11.1x |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.5x | 1.1x |
| oxc-parser (wasm) | 1.5 MB | 481.4 KB | 0.5x | 0.5x |
| yuku-parser (wasm) | 743.3 KB | 222.8 KB | 0.3x | 0.2x |
| malva (wasm) | 1.5 MB | 414.0 KB | 0.5x | 0.4x |
| tsv (ffi) | 3.5 MB | 1.6 MB | 0.9x | 0.9x |
| tsv format (ffi) | 3.2 MB | 1.5 MB | 0.8x | 0.8x |
| tsv parse (ffi) | 1.6 MB | 684.0 KB | 0.4x | 0.4x |
| tsv (napi) | 3.9 MB | 1.8 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.6 MB | 2.9x | 2.6x |
| oxc-parser (napi) | 2.1 MB | 882.6 KB | 0.5x | 0.5x |
| oxfmt (napi) | 9.1 MB | 3.7 MB | 2.3x | 2.1x |
| yuku-parser (napi) | 819.2 KB | 338.1 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 8.9 MB | 3.5 MB | 2.3x | 2.0x |
| rsvelte compiler (napi) | 17.6 MB | 7.4 MB | 4.5x | 4.2x |
| swc (napi) | 32.7 MB | 12.2 MB | 8.4x | 6.9x |

_`vs tsv` divides native rows by `tsv (napi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm rows by `tsv-wasm`. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Skipped Files

1816 files skipped, 3467 unique file+error combinations — Svelte 0, TypeScript 1559, CSS 256 files.

**Per-benchmark skip counts:**
- parse/typescript: acorn-typescript: 909
- parse/typescript: swc: 878
- parse/typescript: yuku-parser-wasm: 332
- parse/typescript: oxc-parser: 310
- parse/typescript: oxc-parser-wasm: 308
- parse/typescript: tsv-json: 282
- parse/typescript: tsv-wasm-json: 282
- parse/typescript: tsv-json-no-locations: 282
- parse/typescript: tsv-wasm-json-no-locations: 282
- parse/typescript: tsv-internal: 282
- parse/typescript: tsv-wasm-internal: 282
- parse/css: svelte/compiler: 240
- parse/typescript: tsc: 222
- parse/css: tsv-json: 185
- parse/css: tsv-wasm-json: 185
- parse/css: tsv-internal: 185
- parse/css: tsv-wasm-internal: 185
- parse/css: postcss: 108

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
