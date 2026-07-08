# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-07-08T02:06:48.550Z — tsv 0.1.0 (99ac4c69)

**Corpus:** 762 Svelte (1.8 MB), 2395 TypeScript (16.4 MB), 50 CSS (0.3 MB) — 3207 files, 18.5 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (665), ../fuz_blog/src (33), ../fuz_code/src (63), ../fuz_css/src (124), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (16), ../fuz_ui/src (216), ../fuz_util/src (145), ../mdz/src (59), ../gro/src (156), ../svelte-docinfo/src (99), ../tsv.fuz.dev/src (28), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (297), ../svelte/packages/svelte/src (415), ../svelte.dev/apps/svelte.dev/src (144), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.4, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.139.0, oxfmt@0.57.0, @biomejs/wasm-bundler@2.5.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| -------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler            | 2.29       | 11  | 436.62   | 441.02   | 445.75   | 449.16   | 452.19   | 430.16   | 452.95   | baseline                     |
| tsv-json                   | 4.77       | 20  | 209.75   | 211.00   | 215.35   | 215.91   | 216.28   | 208.69   | 216.39   | 2.08x                        |
| tsv-json-no-locations      | 7.52       | 31  | 133.07   | 134.47   | 136.81   | 136.91   | 137.38   | 131.47   | 137.46   | 3.29x                        |
| tsv_wasm-json              | 4.29       | 18  | 233.02   | 234.53   | 237.90   | 238.73   | 239.07   | 231.88   | 239.15   | 1.88x                        |
| tsv_wasm-json-no-locations | 6.56       | 28  | 152.53   | 154.08   | 156.70   | 156.78   | 158.02   | 150.82   | 158.60   | 2.87x                        |
| tsv-internal               | 46.71      | 190 | 21.33    | 21.90    | 22.09    | 22.18    | 22.40    | 21.16    | 22.55    | 20.4x                        |
| tsv_wasm-internal          | 23.03      | 90  | 43.13    | 43.75    | 44.54    | 45.15    | 48.34    | 39.20    | 50.60    | 10.1x                        |

**Files (intersection):** 762

**Throughput:** svelte/compiler 4.2 MB/s, tsv-json 8.8 MB/s, tsv-json-no-locations 13.9 MB/s, tsv_wasm-json 7.9 MB/s, tsv_wasm-json-no-locations 12.1 MB/s, tsv-internal 86.1 MB/s, tsv_wasm-internal 42.5 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 9.8x tsv-internal, tsv_wasm-json 5.4x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.23       | 7  | 4382.01  | 4393.72  | 4415.93  | —        | —        | 4344.81  | 4441.34  | baseline              |
| tsv        | 12.94      | 49 | 77.16    | 78.95    | 80.00    | 80.22    | 82.81    | 76.77    | 84.55    | 56.7x                 |
| tsv_wasm   | 8.24       | 39 | 120.93   | 123.33   | 124.38   | 125.77   | 128.31   | 119.82   | 129.51   | 36.1x                 |
| oxfmt      | 0.24       | 5  | 4137.18  | 4158.19  | 4164.44  | —        | —        | 4101.44  | 4168.61  | 1.06x                 |
| biome-wasm | 1.14       | 6  | 880.24   | 888.29   | 894.79   | —        | —        | 855.59   | 899.46   | 4.98x                 |

**Files (intersection):** 762

**Throughput:** prettier 0.4 MB/s, tsv 23.9 MB/s, tsv_wasm 15.2 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.1 MB/s

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.34       | 4  | 2.97    | 2.97    | 2.97    | —       | —       | 2.96    | 2.97    | baseline                      |
| tsv-json                   | 0.51       | 4  | 1.97    | 1.97    | 1.97    | —       | —       | 1.96    | 1.97    | 1.51x                         |
| tsv-json-no-locations      | 1.05       | 6  | 0.96    | 0.96    | 0.96    | —       | —       | 0.95    | 0.96    | 3.11x                         |
| tsv_wasm-json              | 0.48       | 5  | 2.09    | 2.10    | 2.10    | —       | —       | 2.08    | 2.10    | 1.42x                         |
| tsv_wasm-json-no-locations | 0.95       | 5  | 1.05    | 1.05    | 1.05    | —       | —       | 1.05    | 1.05    | 2.83x                         |
| tsv-internal               | 7.27       | 37 | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 21.6x                         |
| tsv_wasm-internal          | 5.38       | 23 | 0.19    | 0.19    | 0.19    | 0.19    | 0.19    | 0.18    | 0.19    | 16.0x                         |
| oxc-parser                 | 0.80       | 5  | 1.24    | 1.25    | 1.25    | —       | —       | 1.23    | 1.26    | 2.39x                         |
| oxc-parser-wasm            | 0.78       | 5  | 1.28    | 1.28    | 1.28    | —       | —       | 1.28    | 1.28    | 2.32x                         |

**Files (intersection):** 2392

**Throughput:** acorn-typescript 5.5 MB/s, tsv-json 8.3 MB/s, tsv-json-no-locations 17.1 MB/s, tsv_wasm-json 7.8 MB/s, tsv_wasm-json-no-locations 15.6 MB/s, tsv-internal 118.9 MB/s, tsv_wasm-internal 87.9 MB/s, oxc-parser 13.2 MB/s, oxc-parser-wasm 12.8 MB/s

**Coverage:** acorn-typescript 2392/2395 (99%), tsv-json 2395/2395 (100%), tsv-json-no-locations 2395/2395 (100%), tsv_wasm-json 2395/2395 (100%), tsv_wasm-json-no-locations 2395/2395 (100%), tsv-internal 2395/2395 (100%), tsv_wasm-internal 2395/2395 (100%), oxc-parser 2393/2395 (99%), oxc-parser-wasm 2393/2395 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 14.3x tsv-internal, tsv_wasm-json 11.3x tsv_wasm-internal

## format/typescript

| Task Name  | sweeps/sec | n | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | - | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.08       | 6 | 13278.63 | 13287.81 | 13369.47 | —        | —        | 13241.68 | 13484.95 | baseline              |
| tsv        | 1.89       | 9 | 526.87   | 531.51   | 532.83   | —        | —        | 525.41   | 541.40   | 25.1x                 |
| tsv_wasm   | 1.26       | 6 | 792.33   | 792.70   | 792.96   | —        | —        | 792.09   | 793.02   | 16.7x                 |
| oxfmt      | 1.12       | 6 | 893.73   | 900.46   | 904.36   | —        | —        | 889.83   | 906.66   | 14.8x                 |
| biome-wasm | 0.23       | 4 | 4306.84  | 4456.40  | 8127.67  | —        | —        | 4249.15  | 10575.17 | 3.07x                 |

**Files (intersection):** 2393

**Throughput:** prettier 1.2 MB/s, tsv 31.0 MB/s, tsv_wasm 20.6 MB/s, oxfmt 18.3 MB/s, biome-wasm 3.8 MB/s

**Coverage:** prettier 2395/2395 (100%), tsv 2395/2395 (100%), tsv_wasm 2395/2395 (100%), oxfmt 2393/2395 (99%), biome-wasm 2395/2395 (100%)

## parse/css

| Task Name         | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 122.89     | 545 | 8.06     | 8.44     | 8.87     | 9.16     | 13.14    | 7.81     | 16.68    | baseline                     |
| tsv-json          | 52.40      | 238 | 19.02    | 19.47    | 19.66    | 21.99    | 22.80    | 18.29    | 26.74    | 0.43x                        |
| tsv_wasm-json     | 43.03      | 186 | 23.15    | 23.70    | 24.69    | 25.64    | 29.42    | 22.52    | 33.91    | 0.35x                        |
| tsv-internal      | 187.79     | 753 | 5.31     | 5.39     | 5.57     | 5.66     | 5.76     | 5.25     | 7.55     | 1.53x                        |
| tsv_wasm-internal | 95.68      | 479 | 10.54    | 10.58    | 10.97    | 11.04    | 11.09    | 9.39     | 11.20    | 0.78x                        |

**Files (intersection):** 50

**Throughput:** svelte/compiler 36.7 MB/s, tsv-json 15.6 MB/s, tsv_wasm-json 12.8 MB/s, tsv-internal 56.1 MB/s, tsv_wasm-internal 28.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 3.6x tsv-internal, tsv_wasm-json 2.2x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.88       | 10  | 531.81   | 540.53   | 546.95   | 549.48   | 551.52   | 519.33   | 552.02   | baseline              |
| tsv        | 105.05     | 467 | 9.49     | 9.63     | 9.76     | 9.79     | 9.94     | 9.42     | 13.61    | 56.0x                 |
| tsv_wasm   | 62.41      | 311 | 15.96    | 16.45    | 16.49    | 16.55    | 16.75    | 15.28    | 20.05    | 33.3x                 |
| oxfmt      | 11.33      | 57  | 88.13    | 90.94    | 92.25    | 93.05    | 94.55    | 81.06    | 94.98    | 6.04x                 |
| biome-wasm | 6.27       | 27  | 158.97   | 161.91   | 164.02   | 178.10   | 186.01   | 156.65   | 186.18   | 3.34x                 |

**Files (intersection):** 50

**Throughput:** prettier 0.6 MB/s, tsv 31.4 MB/s, tsv_wasm 18.6 MB/s, oxfmt 3.4 MB/s, biome-wasm 1.9 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 763.4 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.0 MB | 381.2 KB | 0.4x | 0.5x |
| tsv_wasm | 2.4 MB | 843.0 KB | — | — |
| biome (wasm) | 37.5 MB | 9.0 MB | 15.4x | 10.7x |
| oxc-parser (wasm) | 1.6 MB | 501.4 KB | 0.7x | 0.6x |
| tsv (ffi) | 3.3 MB | 1.4 MB | 1.0x | 1.0x |
| tsv format (ffi) | 3.1 MB | 1.3 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 691.8 KB | 0.5x | 0.5x |
| tsv (napi) | 3.5 MB | 1.5 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.5 MB | 4.6 MB | 3.3x | 3.1x |
| oxc-parser (napi) | 2.4 MB | 977.4 KB | 0.7x | 0.7x |
| oxfmt (napi) | 9.1 MB | 3.6 MB | 2.6x | 2.4x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (762f) | **56.7x** prettier, **53.5x** oxfmt |
| format typescript (2393f) | **25.1x** prettier, **1.70x** oxfmt |
| format css (50f) | **56.0x** prettier, **9.27x** oxfmt |
| parse svelte (762f) | **2.08x** svelte |
| parse typescript (2392f) | **1.51x** svelte, **0.63x** oxc-parser |
| parse css (50f) | **0.43x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (762f) | **36.1x** prettier, **7.25x** biome-wasm |
| format typescript (2393f) | **16.7x** prettier, **5.45x** biome-wasm |
| format css (50f) | **33.3x** prettier, **9.95x** biome-wasm |
| parse svelte (762f) | **1.88x** svelte |
| parse typescript (2392f) | **1.42x** svelte, **0.61x** oxc-parser-wasm |
| parse css (50f) | **0.35x** svelte |

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
