# tsv benchmark results

**Runtime:** deno

**Date:** 2026-07-03T20:54:20.010Z — tsv 0.1.0 (2136c09e)

**Corpus:** 1262 Svelte (1.8 MB), 4523 TypeScript (16.2 MB), 182 CSS (0.2 MB) — 5967 files, 18.2 MB total

**Versions:** svelte@5.56.1, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.134.0, oxfmt@0.53.0, @biomejs/wasm-bundler@2.4.16

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. The numbers are per-file, single-core latency/throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name         | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 2.27    | 12  | 441.71   | 444.10   | 445.20   | 445.53   | 445.83   | 434.54   | 445.90   | baseline                     |
| tsv-json          | 2.91    | 15  | 343.35   | 346.17   | 348.25   | 349.33   | 351.20   | 335.29   | 351.67   | 1.29x                        |
| tsv_wasm-json     | 2.41    | 9   | 414.91   | 420.16   | 427.80   | —        | —        | 411.81   | 428.18   | 1.07x                        |
| tsv-internal      | 35.92   | 159 | 27.98    | 28.72    | 30.31    | 38.00    | 53.19    | 25.16    | 55.23    | 15.9x                        |
| tsv_wasm-internal | 26.04   | 131 | 38.41    | 38.81    | 39.28    | 39.63    | 40.29    | 37.18    | 40.42    | 11.5x                        |

**Files (intersection):** 1214

**Throughput:** svelte/compiler 4.0 MB/s, tsv-json 5.1 MB/s, tsv_wasm-json 4.2 MB/s, tsv-internal 63.1 MB/s, tsv_wasm-internal 45.7 MB/s

**Coverage:** svelte/compiler 1215/1262 (96%), tsv-json 1224/1262 (96%), tsv_wasm-json 1224/1262 (96%), tsv-internal 1224/1262 (96%), tsv_wasm-internal 1224/1262 (96%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 12.3x tsv-internal, tsv_wasm-json 10.8x tsv_wasm-internal

## format/svelte

| Task Name  | ops/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.22    | 7  | 4471.61  | 4513.54  | 4538.68  | —        | —        | 4415.24  | 4567.76  | baseline              |
| tsv        | 9.85    | 47 | 100.14   | 104.93   | 111.68   | 114.49   | 120.94   | 95.69    | 124.13   | 44.1x                 |
| tsv_wasm   | 7.43    | 37 | 134.04   | 136.00   | 139.15   | 140.77   | 143.02   | 131.34   | 143.90   | 33.3x                 |
| oxfmt      | 0.24    | 7  | 4228.02  | 4240.82  | 4263.51  | —        | —        | 4192.41  | 4280.41  | 1.06x                 |
| biome-wasm | 1.28    | 5  | 779.98   | 781.87   | 784.69   | —        | —        | 779.02   | 787.97   | 5.74x                 |

**Files (intersection):** 1213

**Throughput:** prettier 0.4 MB/s, tsv 16.7 MB/s, tsv_wasm 12.6 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.2 MB/s

**Coverage:** prettier 1223/1262 (96%), tsv 1224/1262 (96%), tsv_wasm 1224/1262 (96%), oxfmt 1220/1262 (96%), biome-wasm 1260/1262 (99%)

## parse/typescript

| Task Name         | ops/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| ----------------- | ------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript  | 0.34    | 5  | 2.90    | 2.90    | 2.90    | —       | —       | 2.89    | 2.90    | baseline                      |
| tsv-json          | 0.51    | 4  | 1.96    | 1.96    | 1.97    | —       | —       | 1.95    | 1.98    | 1.48x                         |
| tsv_wasm-json     | 0.45    | 4  | 2.22    | 2.23    | 2.23    | —       | —       | 2.22    | 2.23    | 1.30x                         |
| tsv-internal      | 5.57    | 28 | 0.18    | 0.19    | 0.19    | 0.19    | 0.19    | 0.17    | 0.19    | 16.1x                         |
| tsv_wasm-internal | 3.85    | 20 | 0.26    | 0.26    | 0.26    | 0.26    | 0.26    | 0.26    | 0.26    | 11.2x                         |
| oxc-parser        | 0.84    | 5  | 1.19    | 1.20    | 1.21    | —       | —       | 1.15    | 1.21    | 2.44x                         |
| oxc-parser-wasm   | 0.78    | 5  | 1.29    | 1.29    | 1.29    | —       | —       | 1.28    | 1.30    | 2.25x                         |

**Files (intersection):** 4170

**Throughput:** acorn-typescript 5.5 MB/s, tsv-json 8.2 MB/s, tsv_wasm-json 7.2 MB/s, tsv-internal 89.5 MB/s, tsv_wasm-internal 61.9 MB/s, oxc-parser 13.6 MB/s, oxc-parser-wasm 12.5 MB/s

**Coverage:** acorn-typescript 4191/4523 (92%), tsv-json 4323/4523 (95%), tsv_wasm-json 4323/4523 (95%), tsv-internal 4323/4523 (95%), tsv_wasm-internal 4323/4523 (95%), oxc-parser 4331/4523 (95%), oxc-parser-wasm 4523/4523 (100%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 10.9x tsv-internal, tsv_wasm-json 8.6x tsv_wasm-internal

## format/typescript

| Task Name  | ops/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ---------- | ------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier   | 0.08    | 7 | 13.08   | 13.10   | 13.12   | —       | —       | 13.01   | 13.14   | baseline              |
| tsv        | 1.52    | 8 | 0.65    | 0.66    | 0.66    | —       | —       | 0.65    | 0.66    | 19.9x                 |
| tsv_wasm   | 1.06    | 6 | 0.94    | 0.94    | 0.95    | —       | —       | 0.94    | 0.95    | 13.9x                 |
| oxfmt      | 0.96    | 5 | 1.04    | 1.05    | 1.05    | —       | —       | 1.03    | 1.05    | 12.5x                 |
| biome-wasm | 0.21    | 7 | 4.80    | 4.81    | 4.82    | —       | —       | 4.79    | 4.83    | 2.72x                 |

**Files (intersection):** 4278

**Throughput:** prettier 1.2 MB/s, tsv 24.6 MB/s, tsv_wasm 17.1 MB/s, oxfmt 15.5 MB/s, biome-wasm 3.4 MB/s

**Coverage:** prettier 4357/4523 (96%), tsv 4323/4523 (95%), tsv_wasm 4323/4523 (95%), oxfmt 4334/4523 (95%), biome-wasm 4523/4523 (100%)

## parse/css

| Task Name         | ops/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 196.09  | 787  | 5.06     | 5.38     | 5.60     | 5.72     | 5.92     | 4.94     | 6.42     | baseline                     |
| tsv-json          | 104.92  | 413  | 9.51     | 9.85     | 11.20    | 13.10    | 14.10    | 9.33     | 16.26    | 0.54x                        |
| tsv_wasm-json     | 85.67   | 340  | 11.66    | 11.88    | 12.23    | 12.39    | 12.96    | 11.56    | 14.86    | 0.44x                        |
| tsv-internal      | 254.04  | 1157 | 3.93     | 4.02     | 4.14     | 4.46     | 6.32     | 3.68     | 21.93    | 1.30x                        |
| tsv_wasm-internal | 180.15  | 872  | 5.54     | 5.60     | 5.67     | 5.73     | 5.90     | 5.43     | 6.29     | 0.92x                        |

**Files (intersection):** 147

**Throughput:** svelte/compiler 35.4 MB/s, tsv-json 18.9 MB/s, tsv_wasm-json 15.4 MB/s, tsv-internal 45.8 MB/s, tsv_wasm-internal 32.5 MB/s

**Coverage:** svelte/compiler 152/182 (83%), tsv-json 153/182 (84%), tsv_wasm-json 153/182 (84%), tsv-internal 153/182 (84%), tsv_wasm-internal 153/182 (84%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 2.4x tsv-internal, tsv_wasm-json 2.1x tsv_wasm-internal

## format/css

| Task Name  | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 3.25    | 15  | 307.79   | 310.72   | 319.28   | 330.29   | 340.28   | 302.36   | 342.78   | baseline              |
| tsv        | 129.20  | 562 | 7.67     | 8.05     | 8.71     | 9.27     | 16.35    | 7.27     | 19.10    | 39.7x                 |
| tsv_wasm   | 96.24   | 427 | 10.37    | 10.49    | 10.69    | 10.90    | 11.87    | 10.29    | 17.64    | 29.6x                 |
| oxfmt      | 3.68    | 18  | 271.89   | 274.40   | 278.49   | 285.93   | 302.56   | 264.57   | 306.71   | 1.13x                 |
| biome-wasm | 15.83   | 80  | 62.96    | 63.71    | 64.46    | 64.94    | 65.54    | 61.95    | 65.61    | 4.87x                 |

**Files (intersection):** 153

**Throughput:** prettier 0.6 MB/s, tsv 24.6 MB/s, tsv_wasm 18.3 MB/s, oxfmt 0.7 MB/s, biome-wasm 3.0 MB/s

**Coverage:** prettier 181/182 (99%), tsv 153/182 (84%), tsv_wasm 153/182 (84%), oxfmt 181/182 (99%), biome-wasm 182/182 (100%)

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.3 MB | 778.5 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.2 MB | 436.6 KB | 0.5x | 0.5x |
| tsv_wasm | 2.6 MB | 894.3 KB | — | — |
| biome (wasm) | 34.4 MB | 8.2 MB | 13.1x | 9.2x |
| oxc-parser (wasm) | 1.9 MB | 518.7 KB | 0.7x | 0.6x |
| tsv (ffi) | 3.4 MB | 1.4 MB | — | — |
| oxc-parser+oxfmt (napi) | 10.7 MB | 4.3 MB | 3.2x | 2.9x |
| tsv format (ffi) | 3.0 MB | 1.3 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 730.7 KB | 0.5x | 0.5x |
| tsv (napi) | 3.5 MB | 1.5 MB | 1.0x | 1.0x |
| oxc-parser (napi) | 2.7 MB | 1.0 MB | 0.8x | 0.7x |
| oxfmt (napi) | 8.0 MB | 3.2 MB | 2.4x | 2.2x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1213f) | **44.1x** prettier, **41.7x** oxfmt |
| format typescript (4278f) | **19.9x** prettier, **1.59x** oxfmt |
| format css (153f) | **39.7x** prettier, **35.1x** oxfmt |
| parse svelte (1214f) | **1.29x** svelte |
| parse typescript (4170f) | **1.48x** svelte, **0.61x** oxc-parser |
| parse css (147f) | **0.54x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1213f) | **33.3x** prettier, **5.80x** biome-wasm |
| format typescript (4278f) | **13.9x** prettier, **5.10x** biome-wasm |
| format css (153f) | **29.6x** prettier, **6.08x** biome-wasm |
| parse svelte (1214f) | **1.07x** svelte |
| parse typescript (4170f) | **1.30x** svelte, **0.58x** oxc-parser-wasm |
| parse css (147f) | **0.44x** svelte |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no oxc counterpart — oxc exposes no comparably cheap mode (its JS API always serializes; experimentalLazy is setup-dominated). Format groups include parse time — each formatter parses internally._

## Skipped Files

1308 unique file+error combinations — Svelte 168, TypeScript 1079, CSS 61.

**Per-benchmark skip counts:**
- parse/typescript: acorn-typescript: 332
- parse/typescript: tsv-json: 200
- parse/typescript: tsv_wasm-json: 200
- parse/typescript: tsv-internal: 200
- parse/typescript: tsv_wasm-internal: 200
- format/typescript: tsv: 200
- format/typescript: tsv_wasm: 200
- parse/typescript: oxc-parser: 192
- format/typescript: oxfmt: 189
- format/typescript: prettier: 166
- parse/svelte: svelte/compiler: 47
- format/svelte: oxfmt: 42
- format/svelte: prettier: 39
- parse/svelte: tsv-json: 38
- parse/svelte: tsv_wasm-json: 38
- parse/svelte: tsv-internal: 38
- parse/svelte: tsv_wasm-internal: 38
- format/svelte: tsv: 38
- format/svelte: tsv_wasm: 38
- parse/css: svelte/compiler: 30
- parse/css: tsv-json: 29
- parse/css: tsv_wasm-json: 29
- parse/css: tsv-internal: 29
- parse/css: tsv_wasm-internal: 29
- format/css: tsv: 29
- format/css: tsv_wasm: 29
- format/svelte: biome-wasm: 2
- format/css: prettier: 1
- format/css: oxfmt: 1

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
