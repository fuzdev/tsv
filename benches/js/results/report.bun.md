# tsv benchmark results

**Runtime:** bun

**Date:** 2026-06-26T16:14:57.136Z — tsv 0.1.0 (e4c23f8e)

**Corpus:** 1259 Svelte (1.8 MB), 4521 TypeScript (16.2 MB), 182 CSS (0.2 MB) — 5962 files, 18.2 MB total

**Versions:** svelte@5.56.1, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.8.3, prettier-plugin-svelte@4.1.1, oxc-parser@0.134.0, oxfmt@0.53.0

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. The numbers are per-file, single-core latency/throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name         | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 1.37    | 7   | 730.89   | 732.62   | 738.54   | —        | —        | 718.63   | 745.34   | baseline                     |
| tsv-json          | 2.12    | 11  | 469.33   | 478.26   | 480.43   | 483.05   | 485.14   | 460.57   | 485.66   | 1.54x                        |
| tsv_wasm-json     | 2.08    | 11  | 481.43   | 483.08   | 485.46   | 486.22   | 486.83   | 475.62   | 486.98   | 1.52x                        |
| tsv-internal      | 38.41   | 186 | 25.99    | 26.18    | 26.44    | 26.67    | 26.84    | 25.42    | 26.90    | 28.0x                        |
| tsv_wasm-internal | 27.38   | 129 | 36.47    | 36.76    | 37.19    | 37.44    | 60.31    | 35.96    | 78.75    | 20.0x                        |

**Files (intersection):** 1210

**Throughput:** svelte/compiler 2.4 MB/s, tsv-json 3.7 MB/s, tsv_wasm-json 3.6 MB/s, tsv-internal 67.2 MB/s, tsv_wasm-internal 47.9 MB/s

**Coverage:** svelte/compiler 1212/1259 (96%), tsv-json 1220/1259 (96%), tsv_wasm-json 1220/1259 (96%), tsv-internal 1220/1259 (96%), tsv_wasm-internal 1220/1259 (96%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 18.1x tsv-internal, tsv_wasm-json 13.2x tsv_wasm-internal

## format/svelte

| Task Name | ops/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.22    | 7  | 4.58    | 4.63    | 4.66    | —       | —       | 4.45    | 4.71    | baseline              |
| tsv       | 3.86    | 18 | 0.26    | 0.26    | 0.26    | 0.27    | 0.27    | 0.26    | 0.27    | 17.7x                 |
| tsv_wasm  | 2.83    | 14 | 0.35    | 0.36    | 0.36    | 0.36    | 0.37    | 0.35    | 0.37    | 13.0x                 |
| oxfmt     | 0.18    | 7  | 5.54    | 5.57    | 5.64    | —       | —       | 5.35    | 5.70    | 0.83x                 |

**Files (intersection):** 1211

**Throughput:** prettier 0.4 MB/s, tsv 6.7 MB/s, tsv_wasm 5.0 MB/s, oxfmt 0.3 MB/s

**Coverage:** prettier 1220/1259 (96%), tsv 1220/1259 (96%), tsv_wasm 1220/1259 (96%), oxfmt 1217/1259 (96%)

## parse/typescript

| Task Name         | ops/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| ----------------- | ------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript  | 0.16    | 7  | 6.30    | 6.35    | 6.37    | —       | —       | 6.20    | 6.39    | baseline                      |
| tsv-json          | 0.51    | 5  | 1.96    | 1.97    | 1.98    | —       | —       | 1.94    | 1.98    | 3.21x                         |
| tsv_wasm-json     | 0.46    | 4  | 2.15    | 2.15    | 2.16    | —       | —       | 2.15    | 2.16    | 2.92x                         |
| tsv-internal      | 5.27    | 25 | 0.19    | 0.19    | 0.19    | 0.19    | 0.20    | 0.19    | 0.20    | 33.2x                         |
| tsv_wasm-internal | 3.92    | 19 | 0.26    | 0.26    | 0.26    | 0.26    | 0.26    | 0.25    | 0.26    | 24.7x                         |
| oxc-parser        | 1.04    | 4  | 0.96    | 0.96    | 0.96    | —       | —       | 0.96    | 0.96    | 6.57x                         |

**Files (intersection):** 4120

**Throughput:** acorn-typescript 2.5 MB/s, tsv-json 8.2 MB/s, tsv_wasm-json 7.4 MB/s, tsv-internal 84.4 MB/s, tsv_wasm-internal 62.8 MB/s, oxc-parser 16.7 MB/s

**Coverage:** acorn-typescript 4189/4521 (92%), tsv-json 4260/4521 (94%), tsv_wasm-json 4260/4521 (94%), tsv-internal 4260/4521 (94%), tsv_wasm-internal 4260/4521 (94%), oxc-parser 4329/4521 (95%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 10.3x tsv-internal, tsv_wasm-json 8.4x tsv_wasm-internal

## format/typescript

| Task Name | ops/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.06    | 7 | 15.53   | 15.92   | 16.05   | —       | —       | 15.22   | 16.17   | baseline              |
| tsv       | 1.24    | 5 | 0.81    | 0.84    | 0.88    | —       | —       | 0.80    | 0.89    | 19.4x                 |
| tsv_wasm  | 0.96    | 4 | 1.04    | 1.04    | 1.12    | —       | —       | 1.04    | 1.17    | 15.0x                 |
| oxfmt     | 0.73    | 5 | 1.36    | 1.38    | 1.40    | —       | —       | 1.32    | 1.42    | 11.5x                 |

**Files (intersection):** 4219

**Throughput:** prettier 1.0 MB/s, tsv 19.9 MB/s, tsv_wasm 15.4 MB/s, oxfmt 11.8 MB/s

**Coverage:** prettier 4351/4521 (96%), tsv 4260/4521 (94%), tsv_wasm 4260/4521 (94%), oxfmt 4332/4521 (95%)

## parse/css

| Task Name         | ops/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 136.17  | 555  | 7.34     | 7.49     | 10.61    | 11.30    | 12.86    | 7.04     | 21.16    | baseline                     |
| tsv-json          | 87.89   | 431  | 11.39    | 11.81    | 12.10    | 12.29    | 15.70    | 10.54    | 24.16    | 0.65x                        |
| tsv_wasm-json     | 71.17   | 353  | 14.00    | 14.28    | 14.64    | 14.79    | 15.13    | 13.42    | 18.62    | 0.52x                        |
| tsv-internal      | 220.58  | 1007 | 4.53     | 4.57     | 4.63     | 4.72     | 4.91     | 4.45     | 5.32     | 1.62x                        |
| tsv_wasm-internal | 150.97  | 703  | 6.61     | 6.66     | 6.72     | 6.79     | 7.01     | 6.56     | 7.41     | 1.11x                        |

**Files (intersection):** 148

**Throughput:** svelte/compiler 24.9 MB/s, tsv-json 16.0 MB/s, tsv_wasm-json 13.0 MB/s, tsv-internal 40.3 MB/s, tsv_wasm-internal 27.6 MB/s

**Coverage:** svelte/compiler 152/182 (83%), tsv-json 154/182 (84%), tsv_wasm-json 154/182 (84%), tsv-internal 154/182 (84%), tsv_wasm-internal 154/182 (84%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 2.5x tsv-internal, tsv_wasm-json 2.1x tsv_wasm-internal

## format/css

| Task Name | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| --------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier  | 3.92    | 19  | 252.83   | 262.53   | 269.76   | 277.20   | 286.36   | 241.81   | 288.65   | baseline              |
| tsv       | 105.63  | 506 | 9.46     | 9.50     | 9.57     | 9.63     | 9.89     | 9.36     | 10.60    | 27.0x                 |
| tsv_wasm  | 77.99   | 361 | 12.82    | 12.88    | 13.00    | 13.16    | 13.56    | 12.69    | 14.08    | 19.9x                 |
| oxfmt     | 2.85    | 15  | 347.78   | 359.56   | 362.67   | 363.45   | 364.18   | 338.85   | 364.37   | 0.73x                 |

**Files (intersection):** 154

**Throughput:** prettier 0.8 MB/s, tsv 20.4 MB/s, tsv_wasm 15.0 MB/s, oxfmt 0.5 MB/s

**Coverage:** prettier 181/182 (99%), tsv 154/182 (84%), tsv_wasm 154/182 (84%), oxfmt 181/182 (99%)

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 735.1 KB | 0.8x | 0.8x |
| tsv_parse_wasm | 1.6 MB | 492.0 KB | 0.5x | 0.5x |
| tsv_wasm | 2.9 MB | 920.2 KB | — | — |
| oxc-parser (wasm) | 1.9 MB | 518.7 KB | 0.6x | 0.6x |
| tsv (ffi) | 3.7 MB | 1.6 MB | 1.0x | 1.0x |
| tsv format (ffi) | 2.9 MB | 1.3 MB | 0.8x | 0.8x |
| tsv parse (ffi) | 2.1 MB | 867.3 KB | 0.5x | 0.5x |
| tsv (napi) | 3.8 MB | 1.6 MB | — | — |
| oxc-parser+oxfmt (napi) | 10.7 MB | 4.3 MB | 2.8x | 2.7x |
| oxc-parser (napi) | 2.7 MB | 1.0 MB | 0.7x | 0.6x |
| oxfmt (napi) | 8.0 MB | 3.2 MB | 2.1x | 2.0x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1211f) | **17.7x** prettier, **21.2x** oxfmt |
| format typescript (4219f) | **19.4x** prettier, **1.69x** oxfmt |
| format css (154f) | **27.0x** prettier, **37.1x** oxfmt |
| parse svelte (1210f) | **1.54x** svelte |
| parse typescript (4120f) | **3.21x** svelte, **0.49x** oxc-parser |
| parse css (148f) | **0.65x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1211f) | **13.0x** prettier |
| format typescript (4219f) | **15.0x** prettier |
| format css (154f) | **19.9x** prettier |
| parse svelte (1210f) | **1.52x** svelte |
| parse typescript (4120f) | **2.92x** svelte |
| parse css (148f) | **0.52x** svelte |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no oxc counterpart — oxc exposes no comparably cheap mode (its JS API always serializes; experimentalLazy is setup-dominated). Format groups include parse time — each formatter parses internally._

## Skipped Files

1371 unique file+error combinations — Svelte 167, TypeScript 1144, CSS 60.

**Per-benchmark skip counts:**
- parse/typescript: acorn-typescript: 332
- parse/typescript: tsv-json: 261
- parse/typescript: tsv_wasm-json: 261
- parse/typescript: tsv-internal: 261
- parse/typescript: tsv_wasm-internal: 261
- format/typescript: tsv: 261
- format/typescript: tsv_wasm: 261
- parse/typescript: oxc-parser: 192
- format/typescript: oxfmt: 189
- format/typescript: prettier: 170
- parse/svelte: svelte/compiler: 47
- format/svelte: oxfmt: 42
- parse/svelte: tsv-json: 39
- parse/svelte: tsv_wasm-json: 39
- parse/svelte: tsv-internal: 39
- parse/svelte: tsv_wasm-internal: 39
- format/svelte: prettier: 39
- format/svelte: tsv: 39
- format/svelte: tsv_wasm: 39
- parse/css: svelte/compiler: 30
- parse/css: tsv-json: 28
- parse/css: tsv_wasm-json: 28
- parse/css: tsv-internal: 28
- parse/css: tsv_wasm-internal: 28
- format/css: tsv: 28
- format/css: tsv_wasm: 28
- format/css: prettier: 1
- format/css: oxfmt: 1

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
