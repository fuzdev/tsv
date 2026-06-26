# tsv benchmark results

**Runtime:** node

**Date:** 2026-06-26T16:05:52.967Z — tsv 0.1.0 (e4c23f8e)

**Corpus:** 1259 Svelte (1.8 MB), 4521 TypeScript (16.2 MB), 182 CSS (0.2 MB) — 5962 files, 18.2 MB total

**Versions:** svelte@5.56.1, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.8.3, prettier-plugin-svelte@4.1.1, oxc-parser@0.134.0, oxfmt@0.53.0, @biomejs/wasm-bundler@2.4.16

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. The numbers are per-file, single-core latency/throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name         | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 2.17    | 11  | 460.42   | 463.33   | 465.07   | 468.10   | 470.52   | 452.89   | 471.12   | baseline                     |
| tsv-json          | 1.82    | 10  | 549.19   | 554.15   | 561.71   | 562.05   | 562.31   | 538.35   | 562.38   | 0.84x                        |
| tsv_wasm-json     | 1.84    | 9   | 543.61   | 544.63   | 547.04   | —        | —        | 537.45   | 563.04   | 0.85x                        |
| tsv-internal      | 35.94   | 179 | 27.65    | 28.33    | 28.79    | 29.10    | 29.37    | 26.67    | 29.91    | 16.6x                        |
| tsv_wasm-internal | 25.94   | 120 | 38.38    | 39.48    | 41.71    | 43.31    | 46.58    | 36.63    | 48.32    | 11.9x                        |

**Files (intersection):** 1210

**Throughput:** svelte/compiler 3.8 MB/s, tsv-json 3.2 MB/s, tsv_wasm-json 3.2 MB/s, tsv-internal 62.9 MB/s, tsv_wasm-internal 45.4 MB/s

**Coverage:** svelte/compiler 1212/1259 (96%), tsv-json 1220/1259 (96%), tsv_wasm-json 1220/1259 (96%), tsv-internal 1220/1259 (96%), tsv_wasm-internal 1220/1259 (96%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 19.8x tsv-internal, tsv_wasm-json 14.1x tsv_wasm-internal

## format/svelte

| Task Name  | ops/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.23    | 7  | 4431.40  | 4437.88  | 4450.12  | —        | —        | 4376.03  | 4459.00  | baseline              |
| tsv        | 4.13    | 18 | 242.44   | 246.25   | 251.44   | 254.72   | 258.23   | 236.31   | 259.10   | 18.2x                 |
| tsv_wasm   | 2.83    | 15 | 352.51   | 356.39   | 367.24   | 369.19   | 372.51   | 333.20   | 373.35   | 12.5x                 |
| oxfmt      | 0.23    | 7  | 4329.09  | 4374.76  | 4392.43  | —        | —        | 4310.72  | 4401.93  | 1.02x                 |
| biome-wasm | 1.44    | 8  | 696.02   | 698.73   | 700.92   | —        | —        | 691.42   | 702.57   | 6.34x                 |

**Files (intersection):** 1209

**Throughput:** prettier 0.4 MB/s, tsv 7.0 MB/s, tsv_wasm 4.8 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.4 MB/s

**Coverage:** prettier 1220/1259 (96%), tsv 1220/1259 (96%), tsv_wasm 1220/1259 (96%), oxfmt 1217/1259 (96%), biome-wasm 1257/1259 (99%)

## parse/typescript

| Task Name         | ops/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| ----------------- | ------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript  | 0.33    | 5  | 3.08    | 3.09    | 3.09    | —       | —       | 3.06    | 3.09    | baseline                      |
| tsv-json          | 0.37    | 5  | 2.69    | 2.72    | 2.72    | —       | —       | 2.68    | 2.73    | 1.14x                         |
| tsv_wasm-json     | 0.34    | 4  | 2.92    | 2.92    | 2.93    | —       | —       | 2.91    | 2.93    | 1.05x                         |
| tsv-internal      | 4.82    | 25 | 0.21    | 0.21    | 0.21    | 0.21    | 0.21    | 0.20    | 0.21    | 14.8x                         |
| tsv_wasm-internal | 3.77    | 19 | 0.26    | 0.27    | 0.27    | 0.27    | 0.27    | 0.26    | 0.27    | 11.6x                         |
| oxc-parser        | 0.78    | 5  | 1.28    | 1.29    | 1.30    | —       | —       | 1.27    | 1.30    | 2.39x                         |
| oxc-parser-wasm   | 0.75    | 5  | 1.33    | 1.34    | 1.35    | —       | —       | 1.33    | 1.35    | 2.30x                         |

**Files (intersection):** 4120

**Throughput:** acorn-typescript 5.2 MB/s, tsv-json 5.9 MB/s, tsv_wasm-json 5.5 MB/s, tsv-internal 77.1 MB/s, tsv_wasm-internal 60.4 MB/s, oxc-parser 12.5 MB/s, oxc-parser-wasm 12.0 MB/s

**Coverage:** acorn-typescript 4189/4521 (92%), tsv-json 4260/4521 (94%), tsv_wasm-json 4260/4521 (94%), tsv-internal 4260/4521 (94%), tsv_wasm-internal 4260/4521 (94%), oxc-parser 4329/4521 (95%), oxc-parser-wasm 4521/4521 (100%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 13.0x tsv-internal, tsv_wasm-json 11.0x tsv_wasm-internal

## format/typescript

| Task Name  | ops/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ---------- | ------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier   | 0.07    | 7 | 13.57   | 13.61   | 13.69   | —       | —       | 13.34   | 13.75   | baseline              |
| tsv        | 1.21    | 6 | 0.83    | 0.83    | 0.84    | —       | —       | 0.82    | 0.86    | 16.3x                 |
| tsv_wasm   | 0.95    | 5 | 1.05    | 1.05    | 1.05    | —       | —       | 1.05    | 1.05    | 12.9x                 |
| oxfmt      | 0.96    | 5 | 1.04    | 1.05    | 1.06    | —       | —       | 1.03    | 1.07    | 13.0x                 |
| biome-wasm | 0.24    | 5 | 4.23    | 4.23    | 4.24    | —       | —       | 4.21    | 4.24    | 3.20x                 |

**Files (intersection):** 4219

**Throughput:** prettier 1.2 MB/s, tsv 19.4 MB/s, tsv_wasm 15.3 MB/s, oxfmt 15.4 MB/s, biome-wasm 3.8 MB/s

**Coverage:** prettier 4351/4521 (96%), tsv 4260/4521 (94%), tsv_wasm 4260/4521 (94%), oxfmt 4332/4521 (95%), biome-wasm 4521/4521 (100%)

## parse/css

| Task Name         | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 176.21  | 717 | 5.68     | 5.78     | 6.08     | 6.18     | 6.45     | 5.45     | 6.91     | baseline                     |
| tsv-json          | 77.05   | 353 | 12.90    | 13.27    | 13.84    | 14.26    | 15.10    | 12.55    | 16.39    | 0.44x                        |
| tsv_wasm-json     | 65.96   | 307 | 15.13    | 15.36    | 15.68    | 16.13    | 16.85    | 14.80    | 22.95    | 0.37x                        |
| tsv-internal      | 207.33  | 957 | 4.81     | 4.86     | 4.94     | 5.03     | 5.18     | 4.74     | 5.44     | 1.18x                        |
| tsv_wasm-internal | 150.61  | 683 | 6.63     | 6.69     | 6.81     | 6.92     | 7.05     | 6.55     | 7.56     | 0.85x                        |

**Files (intersection):** 148

**Throughput:** svelte/compiler 32.2 MB/s, tsv-json 14.1 MB/s, tsv_wasm-json 12.0 MB/s, tsv-internal 37.8 MB/s, tsv_wasm-internal 27.5 MB/s

**Coverage:** svelte/compiler 152/182 (83%), tsv-json 154/182 (84%), tsv_wasm-json 154/182 (84%), tsv-internal 154/182 (84%), tsv_wasm-internal 154/182 (84%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 2.7x tsv-internal, tsv_wasm-json 2.3x tsv_wasm-internal

## format/css

| Task Name  | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 3.79    | 18  | 263.30   | 267.58   | 270.83   | 275.67   | 297.18   | 256.52   | 302.56   | baseline              |
| tsv        | 101.07  | 423 | 9.88     | 10.04    | 10.29    | 10.45    | 11.05    | 9.78     | 11.68    | 26.6x                 |
| tsv_wasm   | 77.36   | 352 | 12.92    | 12.97    | 13.07    | 13.26    | 13.87    | 12.85    | 21.92    | 20.4x                 |
| oxfmt      | 3.46    | 18  | 289.28   | 294.13   | 296.02   | 298.47   | 302.33   | 278.49   | 303.30   | 0.91x                 |
| biome-wasm | 18.25   | 90  | 54.75    | 55.06    | 55.38    | 55.66    | 56.70    | 54.21    | 57.16    | 4.81x                 |

**Files (intersection):** 154

**Throughput:** prettier 0.7 MB/s, tsv 19.5 MB/s, tsv_wasm 14.9 MB/s, oxfmt 0.7 MB/s, biome-wasm 3.5 MB/s

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
| format svelte (1209f) | **18.2x** prettier, **17.9x** oxfmt |
| format typescript (4219f) | **16.3x** prettier, **1.26x** oxfmt |
| format css (154f) | **26.6x** prettier, **29.2x** oxfmt |
| parse svelte (1210f) | **0.84x** svelte |
| parse typescript (4120f) | **1.14x** svelte, **0.48x** oxc-parser |
| parse css (148f) | **0.44x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1209f) | **12.5x** prettier, **1.97x** biome-wasm |
| format typescript (4219f) | **12.9x** prettier, **4.03x** biome-wasm |
| format css (154f) | **20.4x** prettier, **4.24x** biome-wasm |
| parse svelte (1210f) | **0.85x** svelte |
| parse typescript (4120f) | **1.05x** svelte, **0.46x** oxc-parser-wasm |
| parse css (148f) | **0.37x** svelte |

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
