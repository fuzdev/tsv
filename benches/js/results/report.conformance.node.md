# tsv conformance benchmark results (parse)

**Runtime:** node

**Corpus kind:** conformance — full fixtures-included corpus, parse groups only; the headline is the per-tool Coverage lines (parse success over the full set), with throughput measured on the all-tools-pass intersection

**Date:** 2026-07-05T15:06:16.483Z — tsv 0.1.0 (ade06e02)

**Corpus:** 5355 Svelte (2.7 MB), 46708 TypeScript (78.5 MB), 22672 CSS (7.8 MB) — 74735 files, 89.1 MB total

**Sources:** ../zzz/src (325), ../fuz_app/src (664), ../fuz_blog/src (32), ../fuz_code/src (64), ../fuz_css/src (124), ../fuz_docs/src (64), ../fuz_gitops/src (203), ../fuz_mastodon/src (24), ../fuz_template/src (15), ../fuz_ui/src (215), ../fuz_util/src (144), ../gro/src (187), ../svelte-docinfo/src (297), ../tsv.fuz.dev/src (28), ../kit/packages/kit/src (318), ../svelte/packages/svelte/src (375), ../svelte.dev/apps/svelte.dev/src (135), ../svelte.dev/packages/repl/src (48), ../svelte.dev/packages/site-kit/src (65), ../prettier-plugin-svelte/test (309), ../prettier/tests/format/typescript (789), ../prettier/tests/format/js (1103), ../prettier/tests/format/css (228), ../prettier/tests/format/html (124), ../svelte/packages/svelte/tests (4432), benches/js/.cache/wpt_css (22310), benches/js/.cache/test262_files.json (42113)

**Versions:** svelte@5.56.1, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.134.0, oxfmt@0.53.0, @biomejs/wasm-bundler@2.4.16

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. The numbers are per-file, single-core latency/throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name         | ops/sec | n | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | - | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 1.40    | 5 | 714.22   | 715.41   | 726.96   | —        | —        | 696.74   | 734.65   | baseline                     |
| tsv-json          | 2.81    | 5 | 354.47   | 356.07   | 358.83   | —        | —        | 352.19   | 360.68   | 2.00x                        |
| tsv_wasm-json     | 2.59    | 3 | 386.42   | 386.44   | 389.71   | —        | —        | 386.37   | 391.88   | 1.84x                        |
| tsv-internal      | 26.43   | 4 | 38.02    | 38.13    | 39.17    | —        | —        | 37.40    | 39.86    | 18.8x                        |
| tsv_wasm-internal | 18.93   | 3 | 53.18    | 78.19    | 79.33    | —        | —        | 52.41    | 80.10    | 13.5x                        |

**Files (intersection):** 5209

**Throughput:** svelte/compiler 3.8 MB/s, tsv-json 7.6 MB/s, tsv_wasm-json 7.0 MB/s, tsv-internal 71.3 MB/s, tsv_wasm-internal 51.1 MB/s

**Coverage:** svelte/compiler 5217/5355 (97%), tsv-json 5241/5355 (97%), tsv_wasm-json 5241/5355 (97%), tsv-internal 5241/5355 (97%), tsv_wasm-internal 5241/5355 (97%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 9.4x tsv-internal, tsv_wasm-json 7.3x tsv_wasm-internal

## parse/typescript

| Task Name         | ops/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| ----------------- | ------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript  | 0.09    | 7 | 10.53   | 10.62   | 10.66   | —       | —       | 10.44   | 10.68   | baseline                      |
| tsv-json          | 0.17    | 7 | 5.76    | 5.77    | 5.81    | —       | —       | 5.71    | 5.85    | 1.83x                         |
| tsv_wasm-json     | 0.16    | 7 | 6.13    | 6.14    | 6.15    | —       | —       | 6.09    | 6.16    | 1.72x                         |
| tsv-internal      | 2.22    | 5 | 0.45    | 0.45    | 0.45    | —       | —       | 0.45    | 0.45    | 23.4x                         |
| tsv_wasm-internal | 1.57    | 3 | 0.64    | 0.64    | 0.64    | —       | —       | 0.64    | 0.64    | 16.6x                         |
| oxc-parser        | 0.27    | 4 | 3.71    | 3.74    | 3.85    | —       | —       | 3.70    | 3.93    | 2.84x                         |
| oxc-parser-wasm   | 0.23    | 5 | 4.35    | 4.36    | 4.42    | —       | —       | 4.28    | 4.45    | 2.43x                         |

**Files (intersection):** 45959

**Throughput:** acorn-typescript 7.4 MB/s, tsv-json 13.5 MB/s, tsv_wasm-json 12.7 MB/s, tsv-internal 172.7 MB/s, tsv_wasm-internal 122.2 MB/s, oxc-parser 21.0 MB/s, oxc-parser-wasm 17.9 MB/s

**Coverage:** acorn-typescript 46030/46708 (98%), tsv-json 46464/46708 (99%), tsv_wasm-json 46464/46708 (99%), tsv-internal 46464/46708 (99%), tsv_wasm-internal 46464/46708 (99%), oxc-parser 46465/46708 (99%), oxc-parser-wasm 46708/46708 (100%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 12.8x tsv-internal, tsv_wasm-json 9.6x tsv_wasm-internal

## parse/css

| Task Name         | ops/sec | n | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | - | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 4.16    | 5 | 241.03   | 242.34   | 243.38   | —        | —        | 237.63   | 244.08   | baseline                     |
| tsv-json          | 2.20    | 5 | 453.95   | 455.88   | 457.33   | —        | —        | 450.32   | 458.30   | 0.53x                        |
| tsv_wasm-json     | 1.87    | 5 | 535.59   | 537.00   | 537.08   | —        | —        | 534.58   | 537.14   | 0.45x                        |
| tsv-internal      | 9.19    | 5 | 108.83   | 109.25   | 109.67   | —        | —        | 108.05   | 109.94   | 2.21x                        |
| tsv_wasm-internal | 5.91    | 5 | 168.85   | 169.89   | 170.43   | —        | —        | 167.81   | 170.80   | 1.42x                        |

**Files (intersection):** 22409

**Throughput:** svelte/compiler 32.0 MB/s, tsv-json 17.0 MB/s, tsv_wasm-json 14.4 MB/s, tsv-internal 70.7 MB/s, tsv_wasm-internal 45.5 MB/s

**Coverage:** svelte/compiler 22433/22672 (98%), tsv-json 22478/22672 (99%), tsv_wasm-json 22478/22672 (99%), tsv-internal 22478/22672 (99%), tsv_wasm-internal 22478/22672 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.2x tsv-internal, tsv_wasm-json 3.2x tsv_wasm-internal

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 754.3 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.0 MB | 375.8 KB | 0.4x | 0.5x |
| tsv_wasm | 2.4 MB | 833.6 KB | — | — |
| biome (wasm) | 34.4 MB | 8.2 MB | 14.3x | 9.8x |
| oxc-parser (wasm) | 1.9 MB | 518.7 KB | 0.8x | 0.6x |
| tsv (ffi) | 3.3 MB | 1.4 MB | 1.0x | 1.0x |
| tsv format (ffi) | 3.1 MB | 1.3 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.5 MB | 682.9 KB | 0.4x | 0.5x |
| tsv (napi) | 3.4 MB | 1.5 MB | — | — |
| oxc-parser+oxfmt (napi) | 10.7 MB | 4.3 MB | 3.1x | 2.9x |
| oxc-parser (napi) | 2.7 MB | 1.0 MB | 0.8x | 0.7x |
| oxfmt (napi) | 8.0 MB | 3.2 MB | 2.3x | 2.2x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| parse svelte (5209f) | **2.00x** svelte |
| parse typescript (45959f) | **1.83x** svelte, **0.64x** oxc-parser |
| parse css (22409f) | **0.53x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| parse svelte (5209f) | **1.84x** svelte |
| parse typescript (45959f) | **1.72x** svelte, **0.71x** oxc-parser-wasm |
| parse css (22409f) | **0.45x** svelte |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no oxc counterpart — oxc exposes no comparably cheap mode (its JS API always serializes; experimentalLazy is setup-dominated). Format groups include parse time — each formatter parses internally._

## Skipped Files

1850 unique file+error combinations — Svelte 252, TypeScript 1165, CSS 433.

**Per-benchmark skip counts:**
- parse/typescript: acorn-typescript: 678
- parse/typescript: tsv-json: 244
- parse/typescript: tsv_wasm-json: 244
- parse/typescript: tsv-internal: 244
- parse/typescript: tsv_wasm-internal: 244
- parse/typescript: oxc-parser: 243
- parse/css: svelte/compiler: 239
- parse/css: tsv-json: 194
- parse/css: tsv_wasm-json: 194
- parse/css: tsv-internal: 194
- parse/css: tsv_wasm-internal: 194
- parse/svelte: svelte/compiler: 138
- parse/svelte: tsv-json: 114
- parse/svelte: tsv_wasm-json: 114
- parse/svelte: tsv-internal: 114
- parse/svelte: tsv_wasm-internal: 114

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
