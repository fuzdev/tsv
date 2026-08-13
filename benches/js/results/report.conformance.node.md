# tsv conformance benchmark results (parse)

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** conformance — fixtures-only corpus (disjoint from perf; Svelte set minus svelte/compiler-rejected files), parse groups only; per-tool Coverage lines only (coverage-only run — timed throughput skipped)

**Date:** 2026-08-13T20:15:33.197Z — tsv 0.2.0 (1ce605a3)

**Corpus:** 4556 Svelte (1.1 MB), 52321 TypeScript (69.0 MB), 22641 CSS (7.7 MB) — 79518 files, 77.8 MB total

**Sources:** ../prettier-plugin-svelte/test (318), ../prettier/tests/format/typescript (793), ../prettier/tests/format/js (1103), ../prettier/tests/format/css (228), ../prettier/tests/format/html (84), ../svelte/packages/svelte/tests (4472), benches/js/.cache/wpt_css (22310), benches/js/.cache/test262_files.json (42113), benches/js/.cache/ts_repo_files.json (8097)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.140.0, oxfmt@0.60.0, yuku-parser@0.8.1, @biomejs/wasm-bundler@2.5.4, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.26, @rsvelte/fmt@0.7.4, @rsvelte/vite-plugin-svelte-native@0.3.4 (targets svelte@5.56.8), @swc/core@1.15.47

**Excluded here:** yuku-parser (N-API) — its native binding faults the host process on this corpus (test262 escaped-identifier fixtures), so it cannot be measured against it. The WASM binding runs the same engine and carries the row; both are measured on the perf corpus.

**Added here:** tsc — the TypeScript compiler’s own parser, a verdict rather than a speed, so it carries no row on the throughput surface. Its parser is error-recovering (`createSourceFile` never throws), so an accept means zero `parseDiagnostics`. On the tsc corpus it is the ORACLE that selected those files — 100% by construction, like svelte/compiler on the Svelte set — and an independent parser on every other source, which is what the per-source tables below are for. Coverage counts accepts and so cannot show over-acceptance; that axis is `deno task ts-repo:over-acceptance`.

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

**Coverage:** svelte/compiler 4556/4556 (100%), tsv-json 4556/4556 (100%), tsv_wasm-json 4556/4556 (100%), tsv-json-no-locations 4556/4556 (100%), tsv_wasm-json-no-locations 4556/4556 (100%), tsv-internal 4556/4556 (100%), tsv_wasm-internal 4556/4556 (100%), rsvelte-parse 4555/4556 (99%), rsvelte-parse-skip-expr-loc 4555/4556 (99%)

## parse/typescript

**Coverage:** acorn-typescript 51425/52321 (98%), tsv-json 52046/52321 (99%), tsv_wasm-json 52046/52321 (99%), tsv-json-no-locations 52046/52321 (99%), tsv_wasm-json-no-locations 52046/52321 (99%), tsv-internal 52046/52321 (99%), tsv_wasm-internal 52046/52321 (99%), oxc-parser 52014/52321 (99%), oxc-parser-wasm 52014/52321 (99%), tsc 52107/52321 (99%), yuku-parser-wasm 52010/52321 (99%), swc 51762/52321 (98%)

## parse/css

**Coverage:** svelte/compiler 22402/22641 (98%), tsv-json 22457/22641 (99%), tsv_wasm-json 22457/22641 (99%), tsv-internal 22457/22641 (99%), tsv_wasm-internal 22457/22641 (99%), postcss 22533/22641 (99%)

### parse/svelte by corpus source

| Source | Files | svelte/compiler | tsv-json | tsv_wasm-json | tsv-json-no-locations | tsv_wasm-json-no-locations | tsv-internal | tsv_wasm-internal | rsvelte-parse | rsvelte-parse-skip-expr-loc |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `../prettier-plugin-svelte/test` | 318 | 318 (100%) | 318 (100%) | 318 (100%) | 318 (100%) | 318 (100%) | 318 (100%) | 318 (100%) | 318 (100%) | 318 (100%) |
| `../prettier/tests/format/html` | 84 | 84 (100%) | 84 (100%) | 84 (100%) | 84 (100%) | 84 (100%) | 84 (100%) | 84 (100%) | 84 (100%) | 84 (100%) |
| `../svelte/packages/svelte/tests` | 4154 | 4154 (100%) | 4154 (100%) | 4154 (100%) | 4154 (100%) | 4154 (100%) | 4154 (100%) | 4154 (100%) | 4153 (99%) | 4153 (99%) |

### parse/typescript by corpus source

| Source | Files | acorn-typescript | tsv-json | tsv_wasm-json | tsv-json-no-locations | tsv_wasm-json-no-locations | tsv-internal | tsv_wasm-internal | oxc-parser | oxc-parser-wasm | tsc | yuku-parser-wasm | swc |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `../prettier/tests/format/typescript` | 793 | 714 (90%) | 779 (98%) | 779 (98%) | 779 (98%) | 779 (98%) | 779 (98%) | 779 (98%) | 767 (96%) | 767 (96%) | 789 (99%) | 780 (98%) | 756 (95%) |
| `../prettier/tests/format/js` | 1103 | 860 (77%) | 922 (83%) | 922 (83%) | 922 (83%) | 922 (83%) | 922 (83%) | 922 (83%) | 933 (84%) | 933 (84%) | 948 (85%) | 935 (84%) | 906 (82%) |
| `../prettier/tests/format/css` | 78 | 78 (100%) | 78 (100%) | 78 (100%) | 78 (100%) | 78 (100%) | 78 (100%) | 78 (100%) | 78 (100%) | 78 (100%) | 78 (100%) | 78 (100%) | 78 (100%) |
| `../svelte/packages/svelte/tests` | 137 | 137 (100%) | 137 (100%) | 137 (100%) | 137 (100%) | 137 (100%) | 137 (100%) | 137 (100%) | 137 (100%) | 137 (100%) | 137 (100%) | 137 (100%) | 137 (100%) |
| `benches/js/.cache/test262_files.json` | 42113 | 41864 (99%) | 42113 (100%) | 42113 (100%) | 42113 (100%) | 42113 (100%) | 42113 (100%) | 42113 (100%) | 42099 (99%) | 42099 (99%) | 42058 (99%) | 42089 (99%) | 41953 (99%) |
| `benches/js/.cache/ts_repo_files.json` | 8097 | 7772 (95%) | 8017 (99%) | 8017 (99%) | 8017 (99%) | 8017 (99%) | 8017 (99%) | 8017 (99%) | 8000 (98%) | 8000 (98%) | 8097 (100%) | 7991 (98%) | 7932 (97%) |

### parse/css by corpus source

| Source | Files | svelte/compiler | tsv-json | tsv_wasm-json | tsv-internal | tsv_wasm-internal | postcss |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `../prettier/tests/format/css` | 150 | 120 (80%) | 123 (82%) | 123 (82%) | 123 (82%) | 123 (82%) | 142 (94%) |
| `../svelte/packages/svelte/tests` | 181 | 181 (100%) | 181 (100%) | 181 (100%) | 181 (100%) | 181 (100%) | 181 (100%) |
| `benches/js/.cache/wpt_css` | 22310 | 22101 (99%) | 22153 (99%) | 22153 (99%) | 22153 (99%) | 22153 (99%) | 22210 (99%) |

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_wasm | 2.4 MB | 873.2 KB | — | — |
| biome (wasm) | 38.6 MB | 9.3 MB | 16.0x | 10.6x |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.7x | 1.3x |
| oxc-parser (wasm) | 1.5 MB | 495.2 KB | 0.6x | 0.6x |
| yuku-parser (wasm) | 674.6 KB | 200.3 KB | 0.3x | 0.2x |
| malva (wasm) | 1.5 MB | 414.0 KB | 0.6x | 0.5x |
| tsv (ffi) | 3.4 MB | 1.5 MB | 0.9x | 0.9x |
| tsv (napi) | 3.7 MB | 1.6 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.5 MB | 3.0x | 2.8x |
| oxc-parser (napi) | 2.4 MB | 954.8 KB | 0.6x | 0.6x |
| oxfmt (napi) | 8.8 MB | 3.6 MB | 2.4x | 2.2x |
| yuku-parser (napi) | 741.3 KB | 311.3 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 7.9 MB | 3.2 MB | 2.1x | 2.0x |
| rsvelte compiler (napi) | 14.2 MB | 5.9 MB | 3.8x | 3.7x |
| swc (napi) | 31.6 MB | 11.9 MB | 8.5x | 7.4x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Skipped Files

3094 unique file+error combinations — Svelte 1, TypeScript 2562, CSS 531.

**Per-benchmark skip counts:**
- parse/typescript: acorn-typescript: 896
- parse/typescript: swc: 559
- parse/typescript: yuku-parser-wasm: 311
- parse/typescript: oxc-parser: 307
- parse/typescript: oxc-parser-wasm: 307
- parse/typescript: tsv-json: 275
- parse/typescript: tsv_wasm-json: 275
- parse/typescript: tsv-json-no-locations: 275
- parse/typescript: tsv_wasm-json-no-locations: 275
- parse/typescript: tsv-internal: 275
- parse/typescript: tsv_wasm-internal: 275
- parse/css: svelte/compiler: 239
- parse/typescript: tsc: 214
- parse/css: tsv-json: 184
- parse/css: tsv_wasm-json: 184
- parse/css: tsv-internal: 184
- parse/css: tsv_wasm-internal: 184
- parse/css: postcss: 108
- parse/svelte: rsvelte-parse: 1
- parse/svelte: rsvelte-parse-skip-expr-loc: 1

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
