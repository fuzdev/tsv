# tsv benchmark results

**Runtime:** deno

**Date:** 2026-06-26T15:56:20.597Z — tsv 0.1.0 (e4c23f8e)

**Corpus:** 1258 Svelte (1.8 MB), 4521 TypeScript (16.2 MB), 182 CSS (0.2 MB) — 5961 files, 18.2 MB total

**Versions:** svelte@5.56.1, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.8.3, prettier-plugin-svelte@4.1.1, oxc-parser@0.134.0, oxfmt@0.53.0, @biomejs/wasm-bundler@2.4.16

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. The numbers are per-file, single-core latency/throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name         | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 2.24    | 11  | 447.12   | 449.36   | 452.68   | 460.33   | 467.67   | 441.90   | 469.50   | baseline                     |
| tsv-json          | 1.73    | 9   | 575.15   | 580.79   | 587.90   | —        | —        | 563.71   | 593.13   | 0.77x                        |
| tsv_wasm-json     | 1.65    | 9   | 600.61   | 609.97   | 616.79   | —        | —        | 596.53   | 617.66   | 0.74x                        |
| tsv-internal      | 30.00   | 134 | 33.79    | 34.61    | 35.50    | 44.07    | 56.69    | 29.29    | 62.29    | 13.4x                        |
| tsv_wasm-internal | 22.32   | 112 | 44.55    | 45.63    | 46.15    | 46.45    | 47.14    | 43.54    | 47.22    | 9.97x                        |

**Files (intersection):** 1209

**Throughput:** svelte/compiler 3.9 MB/s, tsv-json 3.0 MB/s, tsv_wasm-json 2.9 MB/s, tsv-internal 52.5 MB/s, tsv_wasm-internal 39.0 MB/s

**Coverage:** svelte/compiler 1211/1258 (96%), tsv-json 1219/1258 (96%), tsv_wasm-json 1219/1258 (96%), tsv-internal 1219/1258 (96%), tsv_wasm-internal 1219/1258 (96%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 17.3x tsv-internal, tsv_wasm-json 13.5x tsv_wasm-internal

## format/svelte

| Task Name  | ops/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.24    | 7  | 4229.23  | 4256.98  | 4277.26  | —        | —        | 4151.83  | 4277.96  | baseline              |
| tsv        | 2.88    | 15 | 346.71   | 350.96   | 356.27   | 362.67   | 371.12   | 331.91   | 373.23   | 12.2x                 |
| tsv_wasm   | 2.58    | 12 | 386.35   | 396.92   | 406.58   | 409.54   | 412.83   | 378.66   | 413.65   | 10.9x                 |
| oxfmt      | 0.24    | 6  | 4245.75  | 4281.94  | 4355.66  | —        | —        | 4214.29  | 4419.65  | 0.99x                 |
| biome-wasm | 1.26    | 5  | 796.14   | 798.22   | 802.94   | —        | —        | 794.72   | 807.51   | 5.31x                 |

**Files (intersection):** 1208

**Throughput:** prettier 0.4 MB/s, tsv 4.9 MB/s, tsv_wasm 4.3 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.1 MB/s

**Coverage:** prettier 1219/1258 (96%), tsv 1219/1258 (96%), tsv_wasm 1219/1258 (96%), oxfmt 1216/1258 (96%), biome-wasm 1256/1258 (99%)

## parse/typescript

| Task Name         | ops/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| ----------------- | ------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript  | 0.34    | 5  | 2.95    | 2.95    | 2.95    | —       | —       | 2.93    | 2.95    | baseline                      |
| tsv-json          | 0.31    | 5  | 3.22    | 3.24    | 3.26    | —       | —       | 3.19    | 3.28    | 0.91x                         |
| tsv_wasm-json     | 0.34    | 5  | 2.97    | 2.98    | 2.98    | —       | —       | 2.92    | 2.98    | 0.99x                         |
| tsv-internal      | 4.39    | 23 | 0.22    | 0.24    | 0.24    | 0.24    | 0.25    | 0.22    | 0.25    | 12.9x                         |
| tsv_wasm-internal | 3.10    | 16 | 0.32    | 0.32    | 0.33    | 0.33    | 0.33    | 0.32    | 0.33    | 9.12x                         |
| oxc-parser        | 0.85    | 5  | 1.18    | 1.21    | 1.21    | —       | —       | 1.15    | 1.21    | 2.49x                         |
| oxc-parser-wasm   | 0.78    | 5  | 1.29    | 1.29    | 1.29    | —       | —       | 1.28    | 1.29    | 2.29x                         |

**Files (intersection):** 4120

**Throughput:** acorn-typescript 5.4 MB/s, tsv-json 5.0 MB/s, tsv_wasm-json 5.4 MB/s, tsv-internal 70.3 MB/s, tsv_wasm-internal 49.6 MB/s, oxc-parser 13.5 MB/s, oxc-parser-wasm 12.4 MB/s

**Coverage:** acorn-typescript 4189/4521 (92%), tsv-json 4260/4521 (94%), tsv_wasm-json 4260/4521 (94%), tsv-internal 4260/4521 (94%), tsv_wasm-internal 4260/4521 (94%), oxc-parser 4329/4521 (95%), oxc-parser-wasm 4521/4521 (100%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 14.2x tsv-internal, tsv_wasm-json 9.2x tsv_wasm-internal

## format/typescript

| Task Name  | ops/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ---------- | ------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier   | 0.08    | 7 | 12.52   | 12.61   | 12.71   | —       | —       | 12.35   | 12.76   | baseline              |
| tsv        | 1.07    | 6 | 0.93    | 0.94    | 0.94    | —       | —       | 0.93    | 0.94    | 13.5x                 |
| tsv_wasm   | 0.82    | 5 | 1.21    | 1.22    | 1.23    | —       | —       | 1.20    | 1.24    | 10.3x                 |
| oxfmt      | 0.92    | 5 | 1.09    | 1.09    | 1.11    | —       | —       | 1.05    | 1.12    | 11.6x                 |
| biome-wasm | 0.20    | 7 | 4.92    | 4.93    | 4.94    | —       | —       | 4.89    | 4.97    | 2.55x                 |

**Files (intersection):** 4219

**Throughput:** prettier 1.3 MB/s, tsv 17.2 MB/s, tsv_wasm 13.2 MB/s, oxfmt 14.8 MB/s, biome-wasm 3.3 MB/s

**Coverage:** prettier 4351/4521 (96%), tsv 4260/4521 (94%), tsv_wasm 4260/4521 (94%), oxfmt 4332/4521 (95%), biome-wasm 4521/4521 (100%)

## parse/css

| Task Name         | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 189.39  | 894 | 5.18     | 5.54     | 5.77     | 5.90     | 6.38     | 4.98     | 7.14     | baseline                     |
| tsv-json          | 66.50   | 283 | 14.99    | 15.56    | 17.41    | 19.98    | 20.72    | 14.37    | 21.71    | 0.35x                        |
| tsv_wasm-json     | 60.91   | 287 | 16.33    | 16.71    | 17.03    | 17.45    | 18.61    | 16.07    | 19.85    | 0.32x                        |
| tsv-internal      | 179.61  | 822 | 5.55     | 5.69     | 5.96     | 6.22     | 11.47    | 5.25     | 23.21    | 0.95x                        |
| tsv_wasm-internal | 130.70  | 598 | 7.64     | 7.74     | 7.91     | 8.18     | 8.65     | 7.51     | 10.21    | 0.69x                        |

**Files (intersection):** 148

**Throughput:** svelte/compiler 34.6 MB/s, tsv-json 12.1 MB/s, tsv_wasm-json 11.1 MB/s, tsv-internal 32.8 MB/s, tsv_wasm-internal 23.9 MB/s

**Coverage:** svelte/compiler 152/182 (83%), tsv-json 154/182 (84%), tsv_wasm-json 154/182 (84%), tsv-internal 154/182 (84%), tsv_wasm-internal 154/182 (84%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 2.7x tsv-internal, tsv_wasm-json 2.1x tsv_wasm-internal

## format/css

| Task Name  | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 4.00    | 19  | 248.38   | 253.84   | 257.92   | 266.49   | 278.28   | 240.71   | 281.23   | baseline              |
| tsv        | 79.21   | 373 | 12.49    | 13.38    | 14.05    | 14.67    | 28.25    | 10.88    | 30.50    | 19.8x                 |
| tsv_wasm   | 66.84   | 312 | 14.89    | 15.19    | 15.62    | 16.07    | 17.01    | 14.60    | 30.17    | 16.7x                 |
| oxfmt      | 3.60    | 16  | 277.00   | 282.22   | 289.82   | 295.25   | 297.75   | 271.86   | 298.37   | 0.90x                 |
| biome-wasm | 14.80   | 73  | 67.45    | 70.10    | 71.32    | 73.75    | 78.08    | 63.14    | 85.84    | 3.70x                 |

**Files (intersection):** 154

**Throughput:** prettier 0.8 MB/s, tsv 15.3 MB/s, tsv_wasm 12.9 MB/s, oxfmt 0.7 MB/s, biome-wasm 2.9 MB/s

**Coverage:** prettier 181/182 (99%), tsv 154/182 (84%), tsv_wasm 154/182 (84%), oxfmt 181/182 (99%), biome-wasm 182/182 (100%)

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 735.1 KB | 0.8x | 0.8x |
| tsv_parse_wasm | 1.6 MB | 492.0 KB | 0.5x | 0.5x |
| tsv_wasm | 2.9 MB | 920.2 KB | — | — |
| biome (wasm) | 34.4 MB | 8.2 MB | 11.9x | 8.9x |
| oxc-parser (wasm) | 1.9 MB | 518.7 KB | 0.6x | 0.6x |
| tsv (ffi) | 3.7 MB | 1.6 MB | — | — |
| oxc-parser+oxfmt (napi) | 10.7 MB | 4.3 MB | 2.9x | 2.7x |
| tsv format (ffi) | 2.9 MB | 1.3 MB | 0.8x | 0.8x |
| tsv parse (ffi) | 2.1 MB | 867.3 KB | 0.6x | 0.6x |
| tsv (napi) | 3.8 MB | 1.6 MB | 1.0x | 1.0x |
| oxc-parser (napi) | 2.7 MB | 1.0 MB | 0.7x | 0.7x |
| oxfmt (napi) | 8.0 MB | 3.2 MB | 2.2x | 2.1x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1208f) | **12.2x** prettier, **12.2x** oxfmt |
| format typescript (4219f) | **13.5x** prettier, **1.16x** oxfmt |
| format css (154f) | **19.8x** prettier, **22.0x** oxfmt |
| parse svelte (1209f) | **0.77x** svelte |
| parse typescript (4120f) | **0.91x** svelte, **0.37x** oxc-parser |
| parse css (148f) | **0.35x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1208f) | **10.9x** prettier, **2.05x** biome-wasm |
| format typescript (4219f) | **10.3x** prettier, **4.05x** biome-wasm |
| format css (154f) | **16.7x** prettier, **4.52x** biome-wasm |
| parse svelte (1209f) | **0.74x** svelte |
| parse typescript (4120f) | **0.99x** svelte, **0.44x** oxc-parser-wasm |
| parse css (148f) | **0.32x** svelte |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no oxc counterpart — oxc exposes no comparably cheap mode (its JS API always serializes; experimentalLazy is setup-dominated). Format groups include parse time — each formatter parses internally._

## Skipped Files

1373 unique file+error combinations — Svelte 169, TypeScript 1144, CSS 60.

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
- format/svelte: biome-wasm: 2
- format/css: prettier: 1
- format/css: oxfmt: 1

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
