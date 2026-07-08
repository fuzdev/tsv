# tsv conformance benchmark results (parse)

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** conformance — fixtures-only corpus (disjoint from perf; Svelte set minus svelte/compiler-rejected files), parse groups only; per-tool Coverage lines only (coverage-only run — timed throughput skipped)

**Date:** 2026-07-08T00:33:27.632Z — tsv 0.1.0 (7211c76b)

**Corpus:** 4535 Svelte (1.1 MB), 44224 TypeScript (63.3 MB), 22641 CSS (7.7 MB) — 71400 files, 72.0 MB total

**Sources:** ../prettier-plugin-svelte/test (318), ../prettier/tests/format/typescript (793), ../prettier/tests/format/js (1103), ../prettier/tests/format/css (228), ../prettier/tests/format/html (84), ../svelte/packages/svelte/tests (4451), benches/js/.cache/wpt_css (22310), benches/js/.cache/test262_files.json (42113)

**Versions:** svelte@5.56.4, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.139.0, oxfmt@0.57.0, @biomejs/wasm-bundler@2.5.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

**Coverage:** svelte/compiler 4535/4535 (100%), tsv-json 4527/4535 (99%), tsv-json-no-locations 4527/4535 (99%), tsv_wasm-json 4527/4535 (99%), tsv_wasm-json-no-locations 4527/4535 (99%), tsv-internal 4527/4535 (99%), tsv_wasm-internal 4527/4535 (99%)

## parse/typescript

**Coverage:** acorn-typescript 43641/44224 (98%), tsv-json 44026/44224 (99%), tsv-json-no-locations 44026/44224 (99%), tsv_wasm-json 44026/44224 (99%), tsv_wasm-json-no-locations 44026/44224 (99%), tsv-internal 44026/44224 (99%), tsv_wasm-internal 44026/44224 (99%), oxc-parser 44014/44224 (99%), oxc-parser-wasm 44014/44224 (99%)

## parse/css

**Coverage:** svelte/compiler 22402/22641 (98%), tsv-json 22447/22641 (99%), tsv_wasm-json 22447/22641 (99%), tsv-internal 22447/22641 (99%), tsv_wasm-internal 22447/22641 (99%)

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 762.0 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.0 MB | 381.0 KB | 0.4x | 0.5x |
| tsv_wasm | 2.4 MB | 841.9 KB | — | — |
| biome (wasm) | 37.5 MB | 9.0 MB | 15.4x | 10.7x |
| oxc-parser (wasm) | 1.6 MB | 501.4 KB | 0.7x | 0.6x |
| tsv (ffi) | 3.3 MB | 1.4 MB | 1.0x | 1.0x |
| tsv format (ffi) | 3.1 MB | 1.3 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 691.2 KB | 0.4x | 0.5x |
| tsv (napi) | 3.5 MB | 1.5 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.5 MB | 4.6 MB | 3.3x | 3.1x |
| oxc-parser (napi) | 2.4 MB | 977.4 KB | 0.7x | 0.7x |
| oxfmt (napi) | 9.1 MB | 3.6 MB | 2.6x | 2.4x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Skipped Files

1432 unique file+error combinations — Svelte 8, TypeScript 991, CSS 433.

**Per-benchmark skip counts:**
- parse/typescript: acorn-typescript: 583
- parse/css: svelte/compiler: 239
- parse/typescript: oxc-parser: 210
- parse/typescript: oxc-parser-wasm: 210
- parse/typescript: tsv-json: 198
- parse/typescript: tsv-json-no-locations: 198
- parse/typescript: tsv_wasm-json: 198
- parse/typescript: tsv_wasm-json-no-locations: 198
- parse/typescript: tsv-internal: 198
- parse/typescript: tsv_wasm-internal: 198
- parse/css: tsv-json: 194
- parse/css: tsv_wasm-json: 194
- parse/css: tsv-internal: 194
- parse/css: tsv_wasm-internal: 194
- parse/svelte: tsv-json: 8
- parse/svelte: tsv-json-no-locations: 8
- parse/svelte: tsv_wasm-json: 8
- parse/svelte: tsv_wasm-json-no-locations: 8
- parse/svelte: tsv-internal: 8
- parse/svelte: tsv_wasm-internal: 8

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
