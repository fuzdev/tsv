# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.9.3

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-07-31T23:33:39.072Z — tsv 0.2.0 (a51ae509)

**Corpus:** 765 Svelte (1.9 MB), 2457 TypeScript (17.1 MB), 49 CSS (0.3 MB) — 3271 files, 19.3 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (665), ../fuz_blog/src (37), ../fuz_code/src (66), ../fuz_css/src (161), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (18), ../fuz_ui/src (216), ../fuz_util/src (145), ../mdz/src (69), ../gro/src (157), ../svelte-docinfo/src (99), ../tsv.fuz.dev/src (33), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (298), ../svelte/packages/svelte/src (415), ../svelte.dev/apps/svelte.dev/src (145), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.8, acorn@8.16.0, acorn-typescript@1.0.11, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.140.0, oxfmt@0.60.0, @biomejs/wasm-bundler@2.5.4, @dprint/typescript@0.96.1, @rsvelte/fmt@0.7.4

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| -------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler            | 2.22       | 12  | 448.77   | 453.98   | 456.42   | 457.86   | 459.23   | 442.19   | 459.57   | baseline                     |
| tsv-json                   | 5.03       | 25  | 198.38   | 200.22   | 200.94   | 201.77   | 205.54   | 196.81   | 206.71   | 2.26x                        |
| tsv-json-no-locations      | 7.88       | 40  | 126.61   | 127.83   | 128.26   | 128.70   | 128.97   | 125.05   | 129.07   | 3.54x                        |
| tsv_wasm-json              | 4.25       | 22  | 235.06   | 236.19   | 236.62   | 236.75   | 238.59   | 232.51   | 239.08   | 1.91x                        |
| tsv_wasm-json-no-locations | 6.34       | 31  | 157.28   | 159.29   | 160.41   | 161.88   | 169.60   | 154.60   | 172.94   | 2.85x                        |
| tsv-internal               | 50.49      | 253 | 19.76    | 20.10    | 20.43    | 20.74    | 21.29    | 18.92    | 21.45    | 22.7x                        |
| tsv_wasm-internal          | 32.88      | 164 | 30.33    | 30.82    | 31.16    | 31.43    | 31.95    | 29.53    | 33.05    | 14.8x                        |

**Files (intersection):** 765

**Throughput:** svelte/compiler 4.2 MB/s, tsv-json 9.6 MB/s, tsv-json-no-locations 15.0 MB/s, tsv_wasm-json 8.1 MB/s, tsv_wasm-json-no-locations 12.1 MB/s, tsv-internal 96.2 MB/s, tsv_wasm-internal 62.7 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 10.0x tsv-internal, tsv_wasm-json 7.7x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.22       | 7  | 4490.64  | 4554.62  | 4632.67  | —        | —        | 4370.42  | 4697.30  | baseline              |
| tsv        | 13.88      | 69 | 71.93    | 72.59    | 73.34    | 73.68    | 75.38    | 70.12    | 77.78    | 62.5x                 |
| tsv_wasm   | 8.84       | 44 | 113.05   | 113.75   | 114.69   | 115.17   | 117.78   | 110.94   | 119.22   | 39.8x                 |
| oxfmt      | 0.23       | 7  | 4412.25  | 4437.84  | 4465.83  | —        | —        | 4372.49  | 4480.89  | 1.02x                 |
| biome-wasm | 1.34       | 7  | 748.35   | 752.75   | 755.02   | —        | —        | 738.28   | 757.79   | 6.02x                 |

**Files (intersection):** 765

**Throughput:** prettier 0.4 MB/s, tsv 26.5 MB/s, tsv_wasm 16.9 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.5 MB/s

**Coverage-only (not timed):** rsvelte-fmt 765/765 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.33       | 5  | 3.00    | 3.00    | 3.01    | —       | —       | 2.99    | 3.01    | baseline                      |
| tsv-json                   | 0.55       | 5  | 1.82    | 1.82    | 1.83    | —       | —       | 1.82    | 1.83    | 1.65x                         |
| tsv-json-no-locations      | 1.13       | 6  | 0.89    | 0.89    | 0.89    | —       | —       | 0.88    | 0.89    | 3.39x                         |
| tsv_wasm-json              | 0.49       | 5  | 2.05    | 2.07    | 2.08    | —       | —       | 2.04    | 2.09    | 1.46x                         |
| tsv_wasm-json-no-locations | 0.95       | 3  | 1.06    | 1.06    | 1.06    | —       | —       | 1.06    | 1.07    | 2.84x                         |
| tsv-internal               | 7.76       | 39 | 0.13    | 0.13    | 0.13    | 0.13    | 0.13    | 0.13    | 0.13    | 23.3x                         |
| tsv_wasm-internal          | 5.16       | 25 | 0.19    | 0.19    | 0.20    | 0.20    | 0.20    | 0.19    | 0.21    | 15.5x                         |
| oxc-parser                 | 0.82       | 5  | 1.22    | 1.23    | 1.23    | —       | —       | 1.20    | 1.23    | 2.46x                         |
| oxc-parser-wasm            | 0.75       | 5  | 1.33    | 1.34    | 1.34    | —       | —       | 1.33    | 1.35    | 2.25x                         |

**Files (intersection):** 2454

**Throughput:** acorn-typescript 5.7 MB/s, tsv-json 9.4 MB/s, tsv-json-no-locations 19.3 MB/s, tsv_wasm-json 8.3 MB/s, tsv_wasm-json-no-locations 16.2 MB/s, tsv-internal 132.4 MB/s, tsv_wasm-internal 88.1 MB/s, oxc-parser 14.0 MB/s, oxc-parser-wasm 12.8 MB/s

**Coverage:** acorn-typescript 2454/2457 (99%), tsv-json 2457/2457 (100%), tsv-json-no-locations 2457/2457 (100%), tsv_wasm-json 2457/2457 (100%), tsv_wasm-json-no-locations 2457/2457 (100%), tsv-internal 2457/2457 (100%), tsv_wasm-internal 2457/2457 (100%), oxc-parser 2455/2457 (99%), oxc-parser-wasm 2455/2457 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 14.1x tsv-internal, tsv_wasm-json 10.6x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.08       | 7  | 12.75   | 12.89   | 12.95   | —       | —       | 12.64   | 12.97   | baseline              |
| tsv         | 1.96       | 10 | 0.51    | 0.51    | 0.51    | 0.51    | 0.51    | 0.51    | 0.51    | 25.1x                 |
| tsv_wasm    | 1.26       | 7  | 0.79    | 0.80    | 0.80    | —       | —       | 0.79    | 0.80    | 16.1x                 |
| oxfmt       | 1.15       | 6  | 0.87    | 0.87    | 0.88    | —       | —       | 0.86    | 0.88    | 14.7x                 |
| biome-wasm  | 0.23       | 5  | 4.35    | 4.37    | 4.37    | —       | —       | 4.31    | 4.37    | 2.94x                 |
| dprint-wasm | 0.28       | 5  | 3.58    | 3.59    | 3.60    | —       | —       | 3.57    | 3.60    | 3.57x                 |

**Files (intersection):** 2455

**Throughput:** prettier 1.3 MB/s, tsv 33.5 MB/s, tsv_wasm 21.6 MB/s, oxfmt 19.6 MB/s, biome-wasm 3.9 MB/s, dprint-wasm 4.8 MB/s

**Coverage:** prettier 2457/2457 (100%), tsv 2457/2457 (100%), tsv_wasm 2457/2457 (100%), oxfmt 2455/2457 (99%), biome-wasm 2457/2457 (100%), dprint-wasm 2457/2457 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 94.53      | 468  | 10.54    | 10.83    | 11.14    | 11.38    | 12.00    | 9.97     | 12.81    | baseline                     |
| tsv-json          | 66.94      | 320  | 14.87    | 15.18    | 15.69    | 16.11    | 17.24    | 14.38    | 24.80    | 0.71x                        |
| tsv_wasm-json     | 52.44      | 260  | 19.03    | 19.35    | 19.66    | 20.10    | 20.64    | 18.23    | 25.05    | 0.55x                        |
| tsv-internal      | 328.36     | 1592 | 3.04     | 3.08     | 3.13     | 3.17     | 3.33     | 2.96     | 3.49     | 3.47x                        |
| tsv_wasm-internal | 185.16     | 910  | 5.39     | 5.46     | 5.54     | 5.59     | 5.76     | 5.27     | 9.60     | 1.96x                        |

**Files (intersection):** 49

**Throughput:** svelte/compiler 31.4 MB/s, tsv-json 22.3 MB/s, tsv_wasm-json 17.4 MB/s, tsv-internal 109.2 MB/s, tsv_wasm-internal 61.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.9x tsv-internal, tsv_wasm-json 3.5x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.92       | 10  | 523.35   | 525.75   | 527.65   | 528.22   | 528.68   | 514.23   | 528.79   | baseline              |
| tsv        | 151.59     | 715 | 6.57     | 6.71     | 6.91     | 7.06     | 7.41     | 6.36     | 13.04    | 79.1x                 |
| tsv_wasm   | 88.96      | 418 | 11.22    | 11.36    | 11.62    | 11.91    | 12.38    | 10.91    | 12.90    | 46.4x                 |
| oxfmt      | 53.58      | 262 | 18.56    | 19.17    | 19.84    | 20.17    | 23.09    | 16.88    | 25.36    | 28.0x                 |
| biome-wasm | 9.81       | 50  | 101.98   | 102.52   | 103.10   | 103.45   | 104.40   | 100.18   | 105.10   | 5.12x                 |

**Files (intersection):** 49

**Throughput:** prettier 0.6 MB/s, tsv 50.4 MB/s, tsv_wasm 29.6 MB/s, oxfmt 17.8 MB/s, biome-wasm 3.3 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 784.8 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 904.8 KB | 354.2 KB | 0.4x | 0.4x |
| tsv_wasm | 2.4 MB | 870.9 KB | — | — |
| biome (wasm) | 38.6 MB | 9.3 MB | 16.1x | 10.7x |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.7x | 1.3x |
| oxc-parser (wasm) | 1.5 MB | 495.2 KB | 0.6x | 0.6x |
| tsv (ffi) | 3.4 MB | 1.5 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.5 MB | 3.3x | 3.1x |
| tsv format (ffi) | 3.1 MB | 1.3 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.5 MB | 650.3 KB | 0.4x | 0.4x |
| tsv (napi) | 3.5 MB | 1.5 MB | 1.0x | 1.0x |
| oxc-parser (napi) | 2.4 MB | 954.8 KB | 0.7x | 0.7x |
| oxfmt (napi) | 8.8 MB | 3.6 MB | 2.6x | 2.5x |
| rsvelte-fmt (binary) | 7.9 MB | 3.2 MB | 2.3x | 2.2x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (765f) | **62.5x** prettier, **61.3x** oxfmt |
| format typescript (2455f) | **25.1x** prettier, **1.71x** oxfmt |
| format css (49f) | **79.1x** prettier, **2.83x** oxfmt |
| parse svelte (765f) | **2.26x** svelte |
| parse typescript (2454f) | **1.65x** svelte, **0.67x** oxc-parser |
| parse css (49f) | **0.71x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (765f) | **39.8x** prettier, **6.62x** biome-wasm |
| format typescript (2455f) | **16.1x** prettier, **5.49x** biome-wasm, **4.52x** dprint-wasm |
| format css (49f) | **46.4x** prettier, **9.07x** biome-wasm |
| parse svelte (765f) | **1.91x** svelte |
| parse typescript (2454f) | **1.46x** svelte, **0.65x** oxc-parser-wasm |
| parse css (49f) | **0.55x** svelte |

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
