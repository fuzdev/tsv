# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.9.3

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-07-22T09:08:34.891Z — tsv 0.1.0 (d063479a)

**Corpus:** 767 Svelte (1.9 MB), 2448 TypeScript (17.0 MB), 50 CSS (0.3 MB) — 3265 files, 19.2 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (665), ../fuz_blog/src (37), ../fuz_code/src (74), ../fuz_css/src (146), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (18), ../fuz_ui/src (216), ../fuz_util/src (145), ../mdz/src (71), ../gro/src (156), ../svelte-docinfo/src (99), ../tsv.fuz.dev/src (34), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (297), ../svelte/packages/svelte/src (415), ../svelte.dev/apps/svelte.dev/src (145), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.4, acorn@8.16.0, acorn-typescript@1.0.11, prettier@3.9.5, prettier-plugin-svelte@4.1.1, oxc-parser@0.140.0, oxfmt@0.60.0, @biomejs/wasm-bundler@2.5.4, @dprint/typescript@0.96.1

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| -------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler            | 2.32       | 10  | 429.90   | 435.44   | 438.57   | 442.73   | 446.50   | 427.91   | 447.44   | baseline                     |
| tsv-json                   | 5.04       | 21  | 198.58   | 200.00   | 203.72   | 204.54   | 205.29   | 197.49   | 205.46   | 2.17x                        |
| tsv-json-no-locations      | 7.87       | 38  | 126.57   | 128.15   | 130.03   | 130.25   | 131.54   | 125.69   | 132.01   | 3.39x                        |
| tsv_wasm-json              | 4.11       | 18  | 243.13   | 244.11   | 248.65   | 249.58   | 249.89   | 240.84   | 249.96   | 1.77x                        |
| tsv_wasm-json-no-locations | 6.26       | 26  | 159.46   | 161.13   | 164.38   | 164.63   | 165.04   | 158.42   | 165.18   | 2.70x                        |
| tsv-internal               | 50.88      | 228 | 19.53    | 20.06    | 20.25    | 20.34    | 20.53    | 19.34    | 21.31    | 21.9x                        |
| tsv_wasm-internal          | 32.23      | 118 | 30.95    | 31.87    | 32.10    | 32.13    | 32.31    | 30.77    | 32.41    | 13.9x                        |

**Files (intersection):** 767

**Throughput:** svelte/compiler 4.4 MB/s, tsv-json 9.6 MB/s, tsv-json-no-locations 15.0 MB/s, tsv_wasm-json 7.8 MB/s, tsv_wasm-json-no-locations 11.9 MB/s, tsv-internal 96.7 MB/s, tsv_wasm-internal 61.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 10.1x tsv-internal, tsv_wasm-json 7.8x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.24       | 6  | 4201.99  | 4285.20  | 4374.17  | —        | —        | 4168.12  | 4460.57  | baseline              |
| tsv        | 14.71      | 65 | 67.64    | 69.46    | 69.93    | 70.08    | 70.99    | 67.15    | 72.82    | 62.0x                 |
| tsv_wasm   | 9.34       | 43 | 106.70   | 109.31   | 110.15   | 111.62   | 111.94   | 105.61   | 112.14   | 39.4x                 |
| oxfmt      | 0.24       | 5  | 4224.42  | 4226.68  | 4240.54  | —        | —        | 4170.48  | 4249.78  | 1.00x                 |
| biome-wasm | 1.41       | 8  | 707.50   | 714.65   | 723.03   | —        | —        | 702.74   | 728.01   | 5.93x                 |

**Files (intersection):** 767

**Throughput:** prettier 0.5 MB/s, tsv 28.0 MB/s, tsv_wasm 17.8 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.7 MB/s

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.35       | 5  | 2.86    | 2.86    | 2.86    | —       | —       | 2.85    | 2.86    | baseline                      |
| tsv-json                   | 0.55       | 5  | 1.81    | 1.81    | 1.81    | —       | —       | 1.80    | 1.81    | 1.58x                         |
| tsv-json-no-locations      | 1.14       | 5  | 0.88    | 0.88    | 0.88    | —       | —       | 0.87    | 0.88    | 3.26x                         |
| tsv_wasm-json              | 0.47       | 5  | 2.11    | 2.11    | 2.11    | —       | —       | 2.10    | 2.11    | 1.36x                         |
| tsv_wasm-json-no-locations | 0.93       | 4  | 1.07    | 1.07    | 1.07    | —       | —       | 1.07    | 1.07    | 2.67x                         |
| tsv-internal               | 7.63       | 37 | 0.13    | 0.13    | 0.13    | 0.13    | 0.13    | 0.13    | 0.13    | 21.8x                         |
| tsv_wasm-internal          | 4.96       | 23 | 0.20    | 0.20    | 0.21    | 0.21    | 0.21    | 0.20    | 0.21    | 14.2x                         |
| oxc-parser                 | 0.86       | 5  | 1.16    | 1.18    | 1.18    | —       | —       | 1.14    | 1.18    | 2.47x                         |
| oxc-parser-wasm            | 0.79       | 3  | 1.27    | 1.27    | 1.29    | —       | —       | 1.27    | 1.30    | 2.25x                         |

**Files (intersection):** 2445

**Throughput:** acorn-typescript 5.9 MB/s, tsv-json 9.4 MB/s, tsv-json-no-locations 19.4 MB/s, tsv_wasm-json 8.0 MB/s, tsv_wasm-json-no-locations 15.8 MB/s, tsv-internal 129.4 MB/s, tsv_wasm-internal 84.1 MB/s, oxc-parser 14.6 MB/s, oxc-parser-wasm 13.3 MB/s

**Coverage:** acorn-typescript 2445/2448 (99%), tsv-json 2448/2448 (100%), tsv-json-no-locations 2448/2448 (100%), tsv_wasm-json 2448/2448 (100%), tsv_wasm-json-no-locations 2448/2448 (100%), tsv-internal 2448/2448 (100%), tsv_wasm-internal 2448/2448 (100%), oxc-parser 2446/2448 (99%), oxc-parser-wasm 2446/2448 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 13.8x tsv-internal, tsv_wasm-json 10.4x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.08       | 7 | 12.10   | 12.19   | 12.29   | —       | —       | 12.04   | 12.39   | baseline              |
| tsv         | 2.00       | 8 | 0.50    | 0.50    | 0.51    | —       | —       | 0.50    | 0.51    | 24.3x                 |
| tsv_wasm    | 1.30       | 6 | 0.77    | 0.77    | 0.78    | —       | —       | 0.77    | 0.78    | 15.8x                 |
| oxfmt       | 1.20       | 6 | 0.84    | 0.84    | 0.84    | —       | —       | 0.83    | 0.85    | 14.5x                 |
| biome-wasm  | 0.24       | 4 | 4.14    | 4.14    | 4.15    | —       | —       | 4.14    | 4.16    | 2.94x                 |
| dprint-wasm | 0.29       | 5 | 3.40    | 3.40    | 3.40    | —       | —       | 3.40    | 3.40    | 3.57x                 |

**Files (intersection):** 2446

**Throughput:** prettier 1.4 MB/s, tsv 34.0 MB/s, tsv_wasm 22.0 MB/s, oxfmt 20.3 MB/s, biome-wasm 4.1 MB/s, dprint-wasm 5.0 MB/s

**Coverage:** prettier 2448/2448 (100%), tsv 2448/2448 (100%), tsv_wasm 2448/2448 (100%), oxfmt 2446/2448 (99%), biome-wasm 2448/2448 (100%), dprint-wasm 2448/2448 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 96.84      | 483  | 10.31    | 10.57    | 10.83    | 10.92    | 11.19    | 9.82     | 13.25    | baseline                     |
| tsv-json          | 67.16      | 312  | 14.85    | 15.11    | 15.40    | 16.48    | 17.04    | 14.56    | 23.77    | 0.69x                        |
| tsv_wasm-json     | 52.01      | 250  | 19.17    | 19.52    | 19.73    | 20.41    | 21.23    | 18.81    | 21.43    | 0.54x                        |
| tsv-internal      | 316.02     | 1493 | 3.16     | 3.18     | 3.21     | 3.23     | 3.27     | 3.12     | 7.17     | 3.26x                        |
| tsv_wasm-internal | 178.50     | 820  | 5.59     | 5.63     | 5.68     | 5.71     | 5.75     | 5.54     | 5.85     | 1.84x                        |

**Files (intersection):** 50

**Throughput:** svelte/compiler 32.1 MB/s, tsv-json 22.2 MB/s, tsv_wasm-json 17.2 MB/s, tsv-internal 104.6 MB/s, tsv_wasm-internal 59.1 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.7x tsv-internal, tsv_wasm-json 3.4x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.95       | 10  | 512.94   | 516.12   | 517.72   | 521.32   | 524.20   | 509.19   | 524.92   | baseline              |
| tsv        | 153.96     | 621 | 6.49     | 6.57     | 6.72     | 6.82     | 6.97     | 6.43     | 7.79     | 79.1x                 |
| tsv_wasm   | 88.06      | 424 | 11.32    | 11.46    | 11.56    | 11.59    | 11.98    | 11.21    | 12.20    | 45.3x                 |
| oxfmt      | 55.80      | 276 | 17.90    | 18.23    | 18.65    | 18.87    | 19.37    | 16.70    | 21.05    | 28.7x                 |
| biome-wasm | 10.13      | 49  | 98.36    | 99.83    | 100.73   | 101.17   | 101.68   | 97.58    | 101.78   | 5.21x                 |

**Files (intersection):** 50

**Throughput:** prettier 0.6 MB/s, tsv 51.0 MB/s, tsv_wasm 29.1 MB/s, oxfmt 18.5 MB/s, biome-wasm 3.4 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 787.9 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.1 MB | 388.7 KB | 0.4x | 0.4x |
| tsv_wasm | 2.4 MB | 867.8 KB | — | — |
| biome (wasm) | 38.6 MB | 9.3 MB | 15.8x | 10.7x |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.7x | 1.3x |
| oxc-parser (wasm) | 1.5 MB | 495.2 KB | 0.6x | 0.6x |
| tsv (ffi) | 3.3 MB | 1.4 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.5 MB | 3.3x | 3.1x |
| tsv format (ffi) | 3.1 MB | 1.3 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 703.2 KB | 0.5x | 0.5x |
| tsv (napi) | 3.5 MB | 1.5 MB | 1.0x | 1.0x |
| oxc-parser (napi) | 2.4 MB | 954.8 KB | 0.7x | 0.7x |
| oxfmt (napi) | 8.8 MB | 3.6 MB | 2.6x | 2.5x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (767f) | **62.0x** prettier, **62.0x** oxfmt |
| format typescript (2446f) | **24.3x** prettier, **1.67x** oxfmt |
| format css (50f) | **79.1x** prettier, **2.76x** oxfmt |
| parse svelte (767f) | **2.17x** svelte |
| parse typescript (2445f) | **1.58x** svelte, **0.64x** oxc-parser |
| parse css (50f) | **0.69x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (767f) | **39.4x** prettier, **6.64x** biome-wasm |
| format typescript (2446f) | **15.8x** prettier, **5.37x** biome-wasm, **4.42x** dprint-wasm |
| format css (50f) | **45.3x** prettier, **8.69x** biome-wasm |
| parse svelte (767f) | **1.77x** svelte |
| parse typescript (2445f) | **1.36x** svelte, **0.60x** oxc-parser-wasm |
| parse css (50f) | **0.54x** svelte |

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
