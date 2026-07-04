# tsv benchmark results

**Runtime:** node

**Date:** 2026-07-03T21:04:21.052Z — tsv 0.1.0 (2136c09e)

**Corpus:** 1263 Svelte (1.8 MB), 4523 TypeScript (16.2 MB), 182 CSS (0.2 MB) — 5968 files, 18.2 MB total

**Versions:** svelte@5.56.1, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.134.0, oxfmt@0.53.0, @biomejs/wasm-bundler@2.4.16

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. The numbers are per-file, single-core latency/throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name         | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 2.23    | 12  | 448.15   | 451.28   | 452.15   | 455.32   | 458.41   | 442.70   | 459.19   | baseline                     |
| tsv-json          | 2.94    | 15  | 339.60   | 340.92   | 341.35   | 341.44   | 341.44   | 337.41   | 341.44   | 1.32x                        |
| tsv_wasm-json     | 2.61    | 13  | 383.70   | 384.61   | 385.66   | 386.48   | 387.23   | 382.60   | 387.42   | 1.17x                        |
| tsv-internal      | 43.97   | 196 | 22.71    | 22.92    | 23.33    | 23.63    | 23.97    | 22.07    | 24.17    | 19.7x                        |
| tsv_wasm-internal | 19.83   | 80  | 49.68    | 50.56    | 53.14    | 54.14    | 55.80    | 46.92    | 71.07    | 8.90x                        |

**Files (intersection):** 1215

**Throughput:** svelte/compiler 3.9 MB/s, tsv-json 5.2 MB/s, tsv_wasm-json 4.6 MB/s, tsv-internal 77.2 MB/s, tsv_wasm-internal 34.8 MB/s

**Coverage:** svelte/compiler 1216/1263 (96%), tsv-json 1225/1263 (96%), tsv_wasm-json 1225/1263 (96%), tsv-internal 1225/1263 (96%), tsv_wasm-internal 1225/1263 (96%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 14.9x tsv-internal, tsv_wasm-json 7.6x tsv_wasm-internal

## format/svelte

| Task Name  | ops/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.22    | 7  | 4551.15  | 4552.90  | 4562.19  | —        | —        | 4515.46  | 4575.94  | baseline              |
| tsv        | 11.75   | 56 | 84.87    | 85.97    | 88.47    | 89.48    | 90.36    | 83.39    | 90.51    | 53.4x                 |
| tsv_wasm   | 7.19    | 35 | 138.96   | 139.87   | 141.51   | 143.85   | 147.19   | 137.17   | 148.80   | 32.6x                 |
| oxfmt      | 0.23    | 7  | 4367.74  | 4390.24  | 4420.03  | —        | —        | 4316.63  | 4453.28  | 1.04x                 |
| biome-wasm | 1.46    | 8  | 686.43   | 688.47   | 691.26   | —        | —        | 682.11   | 695.00   | 6.61x                 |

**Files (intersection):** 1214

**Throughput:** prettier 0.4 MB/s, tsv 19.9 MB/s, tsv_wasm 12.2 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.5 MB/s

**Coverage:** prettier 1224/1263 (96%), tsv 1225/1263 (96%), tsv_wasm 1225/1263 (96%), oxfmt 1221/1263 (96%), biome-wasm 1261/1263 (99%)

## parse/typescript

| Task Name         | ops/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| ----------------- | ------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript  | 0.33    | 4  | 3.07    | 3.07    | 3.08    | —       | —       | 3.07    | 3.09    | baseline                      |
| tsv-json          | 0.48    | 5  | 2.08    | 2.08    | 2.08    | —       | —       | 2.07    | 2.08    | 1.48x                         |
| tsv_wasm-json     | 0.45    | 5  | 2.23    | 2.23    | 2.24    | —       | —       | 2.22    | 2.24    | 1.38x                         |
| tsv-internal      | 6.27    | 31 | 0.16    | 0.16    | 0.16    | 0.16    | 0.16    | 0.16    | 0.16    | 19.3x                         |
| tsv_wasm-internal | 4.46    | 22 | 0.22    | 0.22    | 0.23    | 0.23    | 0.23    | 0.22    | 0.23    | 13.7x                         |
| oxc-parser        | 0.78    | 5  | 1.27    | 1.28    | 1.28    | —       | —       | 1.27    | 1.28    | 2.41x                         |
| oxc-parser-wasm   | 0.75    | 5  | 1.33    | 1.33    | 1.33    | —       | —       | 1.33    | 1.33    | 2.31x                         |

**Files (intersection):** 4170

**Throughput:** acorn-typescript 5.2 MB/s, tsv-json 7.7 MB/s, tsv_wasm-json 7.2 MB/s, tsv-internal 100.8 MB/s, tsv_wasm-internal 71.7 MB/s, oxc-parser 12.6 MB/s, oxc-parser-wasm 12.1 MB/s

**Coverage:** acorn-typescript 4191/4523 (92%), tsv-json 4323/4523 (95%), tsv_wasm-json 4323/4523 (95%), tsv-internal 4323/4523 (95%), tsv_wasm-internal 4323/4523 (95%), oxc-parser 4331/4523 (95%), oxc-parser-wasm 4523/4523 (100%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 13.0x tsv-internal, tsv_wasm-json 9.9x tsv_wasm-internal

## format/typescript

| Task Name  | ops/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ---------- | ------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier   | 0.07    | 7 | 14.91   | 14.93   | 14.93   | —       | —       | 14.86   | 14.94   | baseline              |
| tsv        | 1.65    | 9 | 0.60    | 0.60    | 0.61    | —       | —       | 0.60    | 0.61    | 24.6x                 |
| tsv_wasm   | 1.20    | 7 | 0.83    | 0.83    | 0.83    | —       | —       | 0.83    | 0.83    | 17.9x                 |
| oxfmt      | 0.98    | 5 | 1.02    | 1.02    | 1.03    | —       | —       | 1.01    | 1.03    | 14.6x                 |
| biome-wasm | 0.24    | 5 | 4.21    | 4.21    | 4.22    | —       | —       | 4.20    | 4.22    | 3.54x                 |

**Files (intersection):** 4278

**Throughput:** prettier 1.1 MB/s, tsv 26.7 MB/s, tsv_wasm 19.4 MB/s, oxfmt 15.8 MB/s, biome-wasm 3.8 MB/s

**Coverage:** prettier 4357/4523 (96%), tsv 4323/4523 (95%), tsv_wasm 4323/4523 (95%), oxfmt 4334/4523 (95%), biome-wasm 4523/4523 (100%)

## parse/css

| Task Name         | ops/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 180.70  | 739  | 5.53     | 5.60     | 6.06     | 6.15     | 6.33     | 5.36     | 7.11     | baseline                     |
| tsv-json          | 109.03  | 499  | 9.16     | 9.24     | 9.32     | 9.53     | 10.38    | 9.06     | 10.90    | 0.60x                        |
| tsv_wasm-json     | 90.54   | 414  | 11.04    | 11.10    | 11.24    | 11.62    | 12.36    | 10.94    | 13.54    | 0.50x                        |
| tsv-internal      | 293.23  | 1261 | 3.41     | 3.43     | 3.47     | 3.50     | 3.65     | 3.38     | 3.94     | 1.62x                        |
| tsv_wasm-internal | 201.16  | 941  | 4.97     | 4.99     | 5.02     | 5.05     | 5.12     | 4.93     | 5.22     | 1.11x                        |

**Files (intersection):** 147

**Throughput:** svelte/compiler 32.6 MB/s, tsv-json 19.7 MB/s, tsv_wasm-json 16.3 MB/s, tsv-internal 52.9 MB/s, tsv_wasm-internal 36.3 MB/s

**Coverage:** svelte/compiler 152/182 (83%), tsv-json 153/182 (84%), tsv_wasm-json 153/182 (84%), tsv-internal 153/182 (84%), tsv_wasm-internal 153/182 (84%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 2.7x tsv-internal, tsv_wasm-json 2.2x tsv_wasm-internal

## format/css

| Task Name  | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 3.12    | 15  | 320.90   | 322.92   | 323.68   | 326.59   | 333.23   | 318.16   | 334.89   | baseline              |
| tsv        | 150.48  | 679 | 6.64     | 6.68     | 6.74     | 6.80     | 6.98     | 6.59     | 8.03     | 48.3x                 |
| tsv_wasm   | 108.74  | 515 | 9.18     | 9.24     | 9.31     | 9.37     | 9.52     | 9.11     | 9.94     | 34.9x                 |
| oxfmt      | 3.42    | 18  | 292.31   | 296.45   | 300.00   | 300.90   | 302.29   | 283.73   | 302.63   | 1.10x                 |
| biome-wasm | 18.20   | 88  | 54.82    | 55.31    | 56.05    | 56.25    | 56.90    | 54.19    | 57.30    | 5.84x                 |

**Files (intersection):** 153

**Throughput:** prettier 0.6 MB/s, tsv 28.7 MB/s, tsv_wasm 20.7 MB/s, oxfmt 0.7 MB/s, biome-wasm 3.5 MB/s

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
| tsv (ffi) | 3.4 MB | 1.4 MB | 1.0x | 1.0x |
| tsv format (ffi) | 3.0 MB | 1.3 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 730.7 KB | 0.5x | 0.5x |
| tsv (napi) | 3.5 MB | 1.5 MB | — | — |
| oxc-parser+oxfmt (napi) | 10.7 MB | 4.3 MB | 3.0x | 2.9x |
| oxc-parser (napi) | 2.7 MB | 1.0 MB | 0.8x | 0.7x |
| oxfmt (napi) | 8.0 MB | 3.2 MB | 2.3x | 2.2x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1214f) | **53.4x** prettier, **51.4x** oxfmt |
| format typescript (4278f) | **24.6x** prettier, **1.68x** oxfmt |
| format css (153f) | **48.3x** prettier, **44.0x** oxfmt |
| parse svelte (1215f) | **1.32x** svelte |
| parse typescript (4170f) | **1.48x** svelte, **0.61x** oxc-parser |
| parse css (147f) | **0.60x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1214f) | **32.6x** prettier, **4.94x** biome-wasm |
| format typescript (4278f) | **17.9x** prettier, **5.05x** biome-wasm |
| format css (153f) | **34.9x** prettier, **5.98x** biome-wasm |
| parse svelte (1215f) | **1.17x** svelte |
| parse typescript (4170f) | **1.38x** svelte, **0.60x** oxc-parser-wasm |
| parse css (147f) | **0.50x** svelte |

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
