# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.3.14

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-07-22T09:27:00.825Z — tsv 0.1.0 (d063479a)

**Corpus:** 767 Svelte (1.9 MB), 2448 TypeScript (17.0 MB), 50 CSS (0.3 MB) — 3265 files, 19.2 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (665), ../fuz_blog/src (37), ../fuz_code/src (74), ../fuz_css/src (146), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (18), ../fuz_ui/src (216), ../fuz_util/src (145), ../mdz/src (71), ../gro/src (156), ../svelte-docinfo/src (99), ../tsv.fuz.dev/src (34), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (297), ../svelte/packages/svelte/src (415), ../svelte.dev/apps/svelte.dev/src (145), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.4, acorn@8.16.0, acorn-typescript@1.0.11, prettier@3.9.5, prettier-plugin-svelte@4.1.1, oxc-parser@0.140.0, oxfmt@0.60.0, @dprint/typescript@0.96.1

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| -------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler            | 1.41       | 7   | 708.87   | 714.79   | 727.57   | —        | —        | 693.86   | 746.80   | baseline                     |
| tsv-json                   | 6.02       | 31  | 166.28   | 168.67   | 169.92   | 170.69   | 173.36   | 161.43   | 174.27   | 4.26x                        |
| tsv-json-no-locations      | 8.43       | 42  | 118.25   | 120.52   | 121.78   | 123.61   | 126.02   | 114.30   | 127.55   | 5.96x                        |
| tsv_wasm-json              | 5.60       | 27  | 178.57   | 180.77   | 182.93   | 183.88   | 190.34   | 174.65   | 192.60   | 3.96x                        |
| tsv_wasm-json-no-locations | 7.63       | 39  | 130.72   | 133.94   | 135.03   | 136.89   | 137.24   | 126.87   | 137.40   | 5.40x                        |
| tsv-internal               | 52.80      | 264 | 18.78    | 19.36    | 19.52    | 19.61    | 19.77    | 18.45    | 19.93    | 37.4x                        |
| tsv_wasm-internal          | 37.67      | 182 | 26.35    | 27.04    | 27.26    | 27.39    | 27.53    | 26.06    | 27.77    | 26.7x                        |

**Files (intersection):** 767

**Throughput:** svelte/compiler 2.7 MB/s, tsv-json 11.4 MB/s, tsv-json-no-locations 16.0 MB/s, tsv_wasm-json 10.6 MB/s, tsv_wasm-json-no-locations 14.5 MB/s, tsv-internal 100.4 MB/s, tsv_wasm-internal 71.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 8.8x tsv-internal, tsv_wasm-json 6.7x tsv_wasm-internal

## format/svelte

| Task Name | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.26       | 7  | 3.85    | 3.92    | 3.98    | —       | —       | 3.77    | 4.02    | baseline              |
| tsv       | 14.47      | 72 | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 0.08    | 55.9x                 |
| tsv_wasm  | 10.52      | 52 | 0.09    | 0.10    | 0.10    | 0.10    | 0.10    | 0.09    | 0.10    | 40.7x                 |
| oxfmt     | 0.20       | 7  | 5.04    | 5.21    | 5.23    | —       | —       | 4.91    | 5.26    | 0.76x                 |

**Files (intersection):** 767

**Throughput:** prettier 0.5 MB/s, tsv 27.5 MB/s, tsv_wasm 20.0 MB/s, oxfmt 0.4 MB/s

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- |
| acorn-typescript           | 0.15       | 7  | 6843.31  | 6927.82  | 7016.61  | —        | —        | 6717.40  | 7123.09  | baseline                      |
| tsv-json                   | 0.72       | 5  | 1392.02  | 1392.19  | 1393.37  | —        | —        | 1387.07  | 1394.15  | 4.93x                         |
| tsv-json-no-locations      | 1.26       | 7  | 796.60   | 797.70   | 800.39   | —        | —        | 791.76   | 803.24   | 8.62x                         |
| tsv_wasm-json              | 0.68       | 4  | 1481.03  | 1483.13  | 1487.21  | —        | —        | 1479.50  | 1489.93  | 4.63x                         |
| tsv_wasm-json-no-locations | 1.15       | 6  | 868.63   | 870.87   | 872.80   | —        | —        | 857.30   | 874.47   | 7.91x                         |
| tsv-internal               | 8.28       | 34 | 120.68   | 121.73   | 123.29   | 123.66   | 123.80   | 119.89   | 123.80   | 56.8x                         |
| tsv_wasm-internal          | 5.90       | 24 | 169.29   | 170.73   | 173.32   | 173.41   | 174.08   | 168.36   | 174.33   | 40.5x                         |
| oxc-parser                 | 1.03       | 6  | 970.24   | 973.78   | 977.08   | —        | —        | 967.24   | 979.43   | 7.06x                         |

**Files (intersection):** 2445

**Throughput:** acorn-typescript 2.5 MB/s, tsv-json 12.2 MB/s, tsv-json-no-locations 21.3 MB/s, tsv_wasm-json 11.4 MB/s, tsv_wasm-json-no-locations 19.5 MB/s, tsv-internal 140.4 MB/s, tsv_wasm-internal 100.0 MB/s, oxc-parser 17.4 MB/s

**Coverage:** acorn-typescript 2445/2448 (99%), tsv-json 2448/2448 (100%), tsv-json-no-locations 2448/2448 (100%), tsv_wasm-json 2448/2448 (100%), tsv_wasm-json-no-locations 2448/2448 (100%), tsv-internal 2448/2448 (100%), tsv_wasm-internal 2448/2448 (100%), oxc-parser 2446/2448 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 11.5x tsv-internal, tsv_wasm-json 8.7x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.06       | 7  | 15.54   | 15.68   | 15.84   | —       | —       | 15.35   | 16.01   | baseline              |
| tsv         | 1.95       | 10 | 0.51    | 0.52    | 0.52    | 0.52    | 0.53    | 0.50    | 0.53    | 30.4x                 |
| tsv_wasm    | 1.48       | 7  | 0.68    | 0.68    | 0.69    | —       | —       | 0.67    | 0.70    | 23.1x                 |
| oxfmt       | 0.97       | 5  | 1.04    | 1.04    | 1.04    | —       | —       | 1.03    | 1.04    | 15.1x                 |
| dprint-wasm | 0.34       | 5  | 2.97    | 2.99    | 2.99    | —       | —       | 2.96    | 2.99    | 5.24x                 |

**Files (intersection):** 2446

**Throughput:** prettier 1.1 MB/s, tsv 33.1 MB/s, tsv_wasm 25.1 MB/s, oxfmt 16.4 MB/s, dprint-wasm 5.7 MB/s

**Coverage:** prettier 2448/2448 (100%), tsv 2448/2448 (100%), tsv_wasm 2448/2448 (100%), oxfmt 2446/2448 (99%), dprint-wasm 2448/2448 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 68.15      | 287  | 14.66    | 14.96    | 18.66    | 19.47    | 23.66    | 14.24    | 72.16    | baseline                     |
| tsv-json          | 74.46      | 346  | 13.30    | 13.80    | 14.57    | 14.95    | 17.95    | 12.72    | 20.71    | 1.09x                        |
| tsv_wasm-json     | 69.70      | 313  | 14.32    | 14.63    | 15.50    | 15.86    | 18.80    | 13.91    | 19.45    | 1.02x                        |
| tsv-internal      | 343.22     | 1579 | 2.91     | 2.93     | 2.97     | 2.99     | 3.04     | 2.86     | 6.31     | 5.04x                        |
| tsv_wasm-internal | 218.37     | 979  | 4.57     | 4.61     | 4.67     | 4.69     | 4.72     | 4.54     | 5.11     | 3.20x                        |

**Files (intersection):** 50

**Throughput:** svelte/compiler 22.6 MB/s, tsv-json 24.6 MB/s, tsv_wasm-json 23.1 MB/s, tsv-internal 113.6 MB/s, tsv_wasm-internal 72.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.6x tsv-internal, tsv_wasm-json 3.1x tsv_wasm-internal

## format/css

| Task Name | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| --------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier  | 1.98       | 8   | 508.41   | 512.01   | 536.17   | —        | —        | 490.49   | 541.62   | baseline              |
| tsv       | 153.79     | 746 | 6.50     | 6.57     | 6.70     | 6.76     | 6.97     | 6.26     | 10.64    | 77.8x                 |
| tsv_wasm  | 104.10     | 512 | 9.59     | 9.71     | 9.84     | 9.88     | 10.21    | 9.32     | 14.13    | 52.6x                 |
| oxfmt     | 46.70      | 234 | 21.27    | 23.13    | 24.80    | 25.52    | 26.60    | 17.69    | 28.02    | 23.6x                 |

**Files (intersection):** 50

**Throughput:** prettier 0.7 MB/s, tsv 50.9 MB/s, tsv_wasm 34.5 MB/s, oxfmt 15.5 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 787.9 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.1 MB | 388.7 KB | 0.4x | 0.4x |
| tsv_wasm | 2.4 MB | 867.8 KB | — | — |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.7x | 1.3x |
| oxc-parser (wasm) | 1.5 MB | 495.2 KB | 0.6x | 0.6x |
| tsv (ffi) | 3.3 MB | 1.4 MB | 1.0x | 1.0x |
| tsv format (ffi) | 3.1 MB | 1.3 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 703.2 KB | 0.5x | 0.5x |
| tsv (napi) | 3.5 MB | 1.5 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.5 MB | 3.2x | 3.0x |
| oxc-parser (napi) | 2.4 MB | 954.8 KB | 0.7x | 0.6x |
| oxfmt (napi) | 8.8 MB | 3.6 MB | 2.5x | 2.4x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (767f) | **55.9x** prettier, **73.7x** oxfmt |
| format typescript (2446f) | **30.4x** prettier, **2.02x** oxfmt |
| format css (50f) | **77.8x** prettier, **3.29x** oxfmt |
| parse svelte (767f) | **4.26x** svelte |
| parse typescript (2445f) | **4.93x** svelte, **0.70x** oxc-parser |
| parse css (50f) | **1.09x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (767f) | **40.7x** prettier |
| format typescript (2446f) | **23.1x** prettier, **4.40x** dprint-wasm |
| format css (50f) | **52.6x** prettier |
| parse svelte (767f) | **3.96x** svelte |
| parse typescript (2445f) | **4.63x** svelte |
| parse css (50f) | **1.02x** svelte |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no oxc counterpart — oxc exposes no comparably cheap mode (its JS API always serializes; experimentalLazy is setup-dominated). Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in `@fuzdev/tsv_parse_wasm` / `@fuzdev/tsv_wasm`) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

## Skipped Files

7 unique file+error combinations — Svelte 0, TypeScript 7, CSS 0.

**Per-benchmark skip counts:**
- parse/typescript: acorn-typescript: 3
- parse/typescript: oxc-parser: 2
- format/typescript: oxfmt: 2

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
