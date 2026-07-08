# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.8.3

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-07-08T01:58:22.087Z — tsv 0.1.0 (99ac4c69)

**Corpus:** 762 Svelte (1.8 MB), 2395 TypeScript (16.4 MB), 50 CSS (0.3 MB) — 3207 files, 18.5 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (665), ../fuz_blog/src (33), ../fuz_code/src (63), ../fuz_css/src (124), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (16), ../fuz_ui/src (216), ../fuz_util/src (145), ../mdz/src (59), ../gro/src (156), ../svelte-docinfo/src (99), ../tsv.fuz.dev/src (28), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (297), ../svelte/packages/svelte/src (415), ../svelte.dev/apps/svelte.dev/src (144), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.4, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.139.0, oxfmt@0.57.0, @biomejs/wasm-bundler@2.5.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| -------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler            | 2.36       | 11  | 422.14   | 430.34   | 436.76   | 440.47   | 443.47   | 417.06   | 444.22   | baseline                     |
| tsv-json                   | 5.10       | 21  | 196.13   | 198.27   | 202.67   | 203.54   | 211.13   | 194.72   | 213.60   | 2.16x                        |
| tsv-json-no-locations      | 7.92       | 39  | 125.65   | 127.46   | 129.49   | 130.08   | 131.09   | 124.58   | 131.69   | 3.36x                        |
| tsv_wasm-json              | 4.22       | 17  | 237.14   | 238.53   | 241.75   | 244.11   | 244.15   | 235.87   | 244.16   | 1.79x                        |
| tsv_wasm-json-no-locations | 6.30       | 32  | 157.75   | 160.18   | 161.96   | 162.31   | 162.36   | 156.33   | 162.38   | 2.67x                        |
| tsv-internal               | 46.62      | 214 | 21.30    | 21.92    | 22.12    | 22.24    | 22.35    | 21.09    | 22.39    | 19.8x                        |
| tsv_wasm-internal          | 30.30      | 110 | 32.95    | 33.87    | 34.07    | 34.16    | 36.00    | 32.70    | 47.70    | 12.8x                        |

**Files (intersection):** 762

**Throughput:** svelte/compiler 4.4 MB/s, tsv-json 9.4 MB/s, tsv-json-no-locations 14.6 MB/s, tsv_wasm-json 7.8 MB/s, tsv_wasm-json-no-locations 11.6 MB/s, tsv-internal 86.0 MB/s, tsv_wasm-internal 55.9 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 9.1x tsv-internal, tsv_wasm-json 7.2x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.24       | 7  | 4174.00  | 4245.60  | 4248.54  | —        | —        | 4128.19  | 4252.00  | baseline              |
| tsv        | 12.86      | 52 | 77.56    | 79.40    | 79.75    | 79.81    | 80.56    | 77.05    | 81.60    | 54.0x                 |
| tsv_wasm   | 8.32       | 40 | 119.70   | 120.86   | 122.80   | 123.42   | 124.10   | 118.62   | 124.10   | 34.9x                 |
| oxfmt      | 0.25       | 5  | 4007.28  | 4022.34  | 4041.19  | —        | —        | 3972.73  | 4053.76  | 1.05x                 |
| biome-wasm | 1.49       | 7  | 672.87   | 675.40   | 685.39   | —        | —        | 669.38   | 696.25   | 6.23x                 |

**Files (intersection):** 762

**Throughput:** prettier 0.4 MB/s, tsv 23.7 MB/s, tsv_wasm 15.3 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.7 MB/s

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.36       | 5  | 2.75    | 2.75    | 2.76    | —       | —       | 2.75    | 2.76    | baseline                      |
| tsv-json                   | 0.57       | 5  | 1.77    | 1.77    | 1.77    | —       | —       | 1.76    | 1.77    | 1.56x                         |
| tsv-json-no-locations      | 1.17       | 6  | 0.85    | 0.86    | 0.86    | —       | —       | 0.85    | 0.86    | 3.22x                         |
| tsv_wasm-json              | 0.49       | 5  | 2.05    | 2.05    | 2.05    | —       | —       | 2.05    | 2.05    | 1.34x                         |
| tsv_wasm-json-no-locations | 0.96       | 5  | 1.05    | 1.05    | 1.05    | —       | —       | 1.04    | 1.05    | 2.63x                         |
| tsv-internal               | 7.36       | 32 | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.13    | 0.14    | 20.3x                         |
| tsv_wasm-internal          | 4.86       | 22 | 0.21    | 0.21    | 0.21    | 0.21    | 0.21    | 0.20    | 0.21    | 13.4x                         |
| oxc-parser                 | 0.86       | 5  | 1.17    | 1.17    | 1.18    | —       | —       | 1.15    | 1.18    | 2.36x                         |
| oxc-parser-wasm            | 0.79       | 5  | 1.26    | 1.27    | 1.28    | —       | —       | 1.25    | 1.28    | 2.18x                         |

**Files (intersection):** 2392

**Throughput:** acorn-typescript 5.9 MB/s, tsv-json 9.3 MB/s, tsv-json-no-locations 19.1 MB/s, tsv_wasm-json 8.0 MB/s, tsv_wasm-json-no-locations 15.6 MB/s, tsv-internal 120.3 MB/s, tsv_wasm-internal 79.5 MB/s, oxc-parser 14.0 MB/s, oxc-parser-wasm 12.9 MB/s

**Coverage:** acorn-typescript 2392/2395 (99%), tsv-json 2395/2395 (100%), tsv-json-no-locations 2395/2395 (100%), tsv_wasm-json 2395/2395 (100%), tsv_wasm-json-no-locations 2395/2395 (100%), tsv-internal 2395/2395 (100%), tsv_wasm-internal 2395/2395 (100%), oxc-parser 2393/2395 (99%), oxc-parser-wasm 2393/2395 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 13.0x tsv-internal, tsv_wasm-json 10.0x tsv_wasm-internal

## format/typescript

| Task Name  | sweeps/sec | n | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | - | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.08       | 7 | 12148.42 | 12225.10 | 12272.47 | —        | —        | 12057.42 | 12278.71 | baseline              |
| tsv        | 1.91       | 8 | 524.22   | 526.63   | 530.45   | —        | —        | 522.75   | 540.12   | 23.2x                 |
| tsv_wasm   | 1.23       | 6 | 815.08   | 817.99   | 822.00   | —        | —        | 813.49   | 826.26   | 14.9x                 |
| oxfmt      | 1.10       | 6 | 911.96   | 915.16   | 919.31   | —        | —        | 904.08   | 922.78   | 13.3x                 |
| biome-wasm | 0.25       | 5 | 3960.42  | 3964.89  | 3974.34  | —        | —        | 3955.64  | 3980.63  | 3.07x                 |

**Files (intersection):** 2393

**Throughput:** prettier 1.3 MB/s, tsv 31.2 MB/s, tsv_wasm 20.1 MB/s, oxfmt 17.9 MB/s, biome-wasm 4.1 MB/s

**Coverage:** prettier 2395/2395 (100%), tsv 2395/2395 (100%), tsv_wasm 2395/2395 (100%), oxfmt 2393/2395 (99%), biome-wasm 2395/2395 (100%)

## parse/css

| Task Name         | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 109.40     | 543 | 9.08     | 9.37     | 9.63     | 9.88     | 10.45    | 8.72     | 10.94    | baseline                     |
| tsv-json          | 59.57      | 267 | 16.77    | 17.05    | 17.48    | 18.43    | 19.21    | 16.51    | 27.46    | 0.54x                        |
| tsv_wasm-json     | 45.40      | 216 | 21.94    | 22.32    | 22.62    | 23.35    | 23.99    | 21.58    | 24.22    | 0.42x                        |
| tsv-internal      | 195.24     | 797 | 5.12     | 5.16     | 5.22     | 5.25     | 5.30     | 5.09     | 9.46     | 1.78x                        |
| tsv_wasm-internal | 118.15     | 457 | 8.45     | 8.55     | 8.62     | 8.64     | 8.69     | 8.40     | 8.82     | 1.08x                        |

**Files (intersection):** 50

**Throughput:** svelte/compiler 32.7 MB/s, tsv-json 17.8 MB/s, tsv_wasm-json 13.6 MB/s, tsv-internal 58.3 MB/s, tsv_wasm-internal 35.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 3.3x tsv-internal, tsv_wasm-json 2.6x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 2.05       | 10  | 485.68   | 493.33   | 496.88   | 504.99   | 511.47   | 479.49   | 513.09   | baseline              |
| tsv        | 108.97     | 541 | 9.17     | 9.25     | 9.35     | 9.40     | 9.58     | 9.04     | 9.95     | 53.0x                 |
| tsv_wasm   | 64.65      | 321 | 15.41    | 15.61    | 15.70    | 15.73    | 15.85    | 15.28    | 16.22    | 31.5x                 |
| oxfmt      | 11.70      | 59  | 85.39    | 88.14    | 89.84    | 90.43    | 90.97    | 79.64    | 91.46    | 5.70x                 |
| biome-wasm | 11.05      | 41  | 90.43    | 92.93    | 94.16    | 94.56    | 94.85    | 89.69    | 94.92    | 5.38x                 |

**Files (intersection):** 50

**Throughput:** prettier 0.6 MB/s, tsv 32.5 MB/s, tsv_wasm 19.3 MB/s, oxfmt 3.5 MB/s, biome-wasm 3.3 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 763.4 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.0 MB | 381.2 KB | 0.4x | 0.5x |
| tsv_wasm | 2.4 MB | 843.0 KB | — | — |
| biome (wasm) | 37.5 MB | 9.0 MB | 15.4x | 10.7x |
| oxc-parser (wasm) | 1.6 MB | 501.4 KB | 0.7x | 0.6x |
| tsv (ffi) | 3.3 MB | 1.4 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.5 MB | 4.6 MB | 3.4x | 3.2x |
| tsv format (ffi) | 3.1 MB | 1.3 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 691.8 KB | 0.5x | 0.5x |
| tsv (napi) | 3.5 MB | 1.5 MB | 1.0x | 1.0x |
| oxc-parser (napi) | 2.4 MB | 977.4 KB | 0.7x | 0.7x |
| oxfmt (napi) | 9.1 MB | 3.6 MB | 2.7x | 2.5x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (762f) | **54.0x** prettier, **51.6x** oxfmt |
| format typescript (2393f) | **23.2x** prettier, **1.74x** oxfmt |
| format css (50f) | **53.0x** prettier, **9.31x** oxfmt |
| parse svelte (762f) | **2.16x** svelte |
| parse typescript (2392f) | **1.56x** svelte, **0.66x** oxc-parser |
| parse css (50f) | **0.54x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (762f) | **34.9x** prettier, **5.60x** biome-wasm |
| format typescript (2393f) | **14.9x** prettier, **4.86x** biome-wasm |
| format css (50f) | **31.5x** prettier, **5.85x** biome-wasm |
| parse svelte (762f) | **1.79x** svelte |
| parse typescript (2392f) | **1.34x** svelte, **0.62x** oxc-parser-wasm |
| parse css (50f) | **0.42x** svelte |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no oxc counterpart — oxc exposes no comparably cheap mode (its JS API always serializes; experimentalLazy is setup-dominated). Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in `@fuzdev/tsv_parse_wasm` / `@fuzdev/tsv_wasm`) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

## Skipped Files

7 unique file+error combinations — Svelte 0, TypeScript 7, CSS 0.

**Per-benchmark skip counts:**
- parse/typescript: acorn-typescript: 3
- parse/typescript: oxc-parser: 2
- parse/typescript: oxc-parser-wasm: 2
- format/typescript: oxfmt: 2

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
