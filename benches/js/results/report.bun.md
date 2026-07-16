# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.3.14

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-07-16T00:23:32.242Z — tsv 0.1.0 (eca5466f)

**Corpus:** 763 Svelte (1.9 MB), 2437 TypeScript (16.8 MB), 49 CSS (0.3 MB) — 3249 files, 19.0 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (665), ../fuz_blog/src (37), ../fuz_code/src (66), ../fuz_css/src (146), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (16), ../fuz_ui/src (216), ../fuz_util/src (145), ../mdz/src (71), ../gro/src (156), ../svelte-docinfo/src (99), ../tsv.fuz.dev/src (28), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (297), ../svelte/packages/svelte/src (415), ../svelte.dev/apps/svelte.dev/src (145), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.4, acorn@8.16.0, acorn-typescript@1.0.11, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.140.0, oxfmt@0.59.0

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| -------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler            | 1.36       | 7   | 735.86   | 737.67   | 754.47   | —        | —        | 717.59   | 778.92   | baseline                     |
| tsv-json                   | 6.02       | 31  | 166.02   | 167.74   | 169.50   | 172.18   | 172.63   | 159.50   | 172.71   | 4.44x                        |
| tsv-json-no-locations      | 8.46       | 42  | 117.96   | 120.18   | 122.07   | 123.24   | 127.70   | 113.98   | 129.64   | 6.23x                        |
| tsv_wasm-json              | 5.57       | 26  | 179.13   | 182.66   | 184.86   | 197.57   | 206.56   | 174.37   | 207.38   | 4.10x                        |
| tsv_wasm-json-no-locations | 7.59       | 33  | 132.14   | 134.35   | 147.29   | 162.84   | 168.19   | 127.46   | 170.22   | 5.59x                        |
| tsv-internal               | 50.64      | 250 | 19.59    | 20.11    | 20.42    | 20.57    | 20.74    | 19.20    | 46.54    | 37.3x                        |
| tsv_wasm-internal          | 16.14      | 58  | 61.31    | 63.38    | 66.14    | 92.81    | 94.73    | 56.99    | 95.87    | 11.9x                        |

**Files (intersection):** 763

**Throughput:** svelte/compiler 2.5 MB/s, tsv-json 11.3 MB/s, tsv-json-no-locations 15.9 MB/s, tsv_wasm-json 10.5 MB/s, tsv_wasm-json-no-locations 14.3 MB/s, tsv-internal 95.1 MB/s, tsv_wasm-internal 30.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 8.4x tsv-internal, tsv_wasm-json 2.9x tsv_wasm-internal

## format/svelte

| Task Name | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.27       | 7  | 3.70    | 3.80    | 3.87    | —       | —       | 3.62    | 3.88    | baseline              |
| tsv       | 13.97      | 69 | 0.07    | 0.07    | 0.07    | 0.07    | 0.08    | 0.07    | 0.08    | 52.1x                 |
| tsv_wasm  | 7.24       | 37 | 0.14    | 0.14    | 0.14    | 0.14    | 0.15    | 0.13    | 0.15    | 27.0x                 |
| oxfmt     | 0.20       | 7  | 5.04    | 5.11    | 5.14    | —       | —       | 4.85    | 5.18    | 0.74x                 |

**Files (intersection):** 763

**Throughput:** prettier 0.5 MB/s, tsv 26.2 MB/s, tsv_wasm 13.6 MB/s, oxfmt 0.4 MB/s

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- |
| acorn-typescript           | 0.15       | 5  | 6685.62  | 6736.68  | 6870.29  | —        | —        | 6676.08  | 7018.54  | baseline                      |
| tsv-json                   | 0.74       | 4  | 1350.94  | 1351.56  | 1353.78  | —        | —        | 1350.18  | 1355.27  | 4.94x                         |
| tsv-json-no-locations      | 1.29       | 5  | 774.00   | 774.72   | 777.87   | —        | —        | 773.24   | 782.50   | 8.64x                         |
| tsv_wasm-json              | 0.68       | 4  | 1463.71  | 1465.31  | 1466.30  | —        | —        | 1448.33  | 1466.97  | 4.58x                         |
| tsv_wasm-json-no-locations | 1.17       | 5  | 853.25   | 854.27   | 859.89   | —        | —        | 846.56   | 865.37   | 7.85x                         |
| tsv-internal               | 8.33       | 36 | 119.93   | 120.49   | 123.10   | 123.31   | 123.55   | 119.00   | 123.68   | 55.7x                         |
| tsv_wasm-internal          | 5.76       | 26 | 173.36   | 174.46   | 176.68   | 177.21   | 178.02   | 172.23   | 178.30   | 38.5x                         |
| oxc-parser                 | 1.05       | 6  | 955.83   | 958.45   | 961.49   | —        | —        | 947.58   | 963.82   | 6.99x                         |

**Files (intersection):** 2434

**Throughput:** acorn-typescript 2.5 MB/s, tsv-json 12.4 MB/s, tsv-json-no-locations 21.7 MB/s, tsv_wasm-json 11.5 MB/s, tsv_wasm-json-no-locations 19.7 MB/s, tsv-internal 140.0 MB/s, tsv_wasm-internal 96.7 MB/s, oxc-parser 17.6 MB/s

**Coverage:** acorn-typescript 2434/2437 (99%), tsv-json 2437/2437 (100%), tsv-json-no-locations 2437/2437 (100%), tsv_wasm-json 2437/2437 (100%), tsv_wasm-json-no-locations 2437/2437 (100%), tsv-internal 2437/2437 (100%), tsv_wasm-internal 2437/2437 (100%), oxc-parser 2435/2437 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 11.3x tsv-internal, tsv_wasm-json 8.4x tsv_wasm-internal

## format/typescript

| Task Name | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.07       | 7  | 15.09   | 15.27   | 15.34   | —       | —       | 14.52   | 15.46   | baseline              |
| tsv       | 1.93       | 10 | 0.52    | 0.52    | 0.53    | 0.53    | 0.53    | 0.51    | 0.53    | 29.0x                 |
| tsv_wasm  | 1.44       | 6  | 0.69    | 0.70    | 0.71    | —       | —       | 0.69    | 0.72    | 21.7x                 |
| oxfmt     | 0.99       | 5  | 1.01    | 1.02    | 1.02    | —       | —       | 1.01    | 1.02    | 14.8x                 |

**Files (intersection):** 2435

**Throughput:** prettier 1.1 MB/s, tsv 32.4 MB/s, tsv_wasm 24.3 MB/s, oxfmt 16.6 MB/s

**Coverage:** prettier 2437/2437 (100%), tsv 2437/2437 (100%), tsv_wasm 2437/2437 (100%), oxfmt 2435/2437 (99%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 68.90      | 293  | 14.48    | 14.81    | 16.98    | 20.62    | 25.50    | 14.09    | 31.35    | baseline                     |
| tsv-json          | 73.53      | 356  | 13.54    | 14.14    | 14.47    | 14.63    | 18.14    | 12.68    | 20.49    | 1.07x                        |
| tsv_wasm-json     | 68.55      | 332  | 14.44    | 14.89    | 15.65    | 15.92    | 18.05    | 13.83    | 20.06    | 0.99x                        |
| tsv-internal      | 327.63     | 1556 | 3.05     | 3.07     | 3.12     | 3.14     | 3.18     | 3.00     | 3.35     | 4.76x                        |
| tsv_wasm-internal | 218.86     | 896  | 4.56     | 4.62     | 4.70     | 4.73     | 4.83     | 4.53     | 6.49     | 3.18x                        |

**Files (intersection):** 49

**Throughput:** svelte/compiler 22.6 MB/s, tsv-json 24.1 MB/s, tsv_wasm-json 22.5 MB/s, tsv-internal 107.3 MB/s, tsv_wasm-internal 71.7 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.5x tsv-internal, tsv_wasm-json 3.2x tsv_wasm-internal

## format/css

| Task Name | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| --------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier  | 2.16       | 10  | 467.87   | 475.54   | 479.05   | 501.82   | 520.03   | 440.00   | 524.58   | baseline              |
| tsv       | 147.45     | 712 | 6.75     | 6.86     | 6.96     | 7.03     | 7.15     | 6.53     | 10.75    | 68.3x                 |
| tsv_wasm  | 101.73     | 502 | 9.79     | 9.93     | 10.04    | 10.09    | 10.26    | 9.67     | 10.42    | 47.1x                 |
| oxfmt     | 45.67      | 229 | 21.81    | 23.26    | 24.78    | 25.93    | 27.66    | 18.37    | 30.29    | 21.2x                 |

**Files (intersection):** 49

**Throughput:** prettier 0.7 MB/s, tsv 48.3 MB/s, tsv_wasm 33.3 MB/s, oxfmt 15.0 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.3 MB | 794.6 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.1 MB | 389.6 KB | 0.4x | 0.4x |
| tsv_wasm | 2.5 MB | 875.3 KB | — | — |
| oxc-parser (wasm) | 1.5 MB | 495.2 KB | 0.6x | 0.6x |
| tsv (ffi) | 3.4 MB | 1.5 MB | 1.0x | 1.0x |
| tsv format (ffi) | 3.1 MB | 1.4 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 705.0 KB | 0.5x | 0.5x |
| tsv (napi) | 3.5 MB | 1.5 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.3 MB | 4.5 MB | 3.2x | 3.0x |
| oxc-parser (napi) | 2.4 MB | 954.8 KB | 0.7x | 0.6x |
| oxfmt (napi) | 8.9 MB | 3.6 MB | 2.5x | 2.4x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (763f) | **52.1x** prettier, **70.3x** oxfmt |
| format typescript (2435f) | **29.0x** prettier, **1.96x** oxfmt |
| format css (49f) | **68.3x** prettier, **3.23x** oxfmt |
| parse svelte (763f) | **4.44x** svelte |
| parse typescript (2434f) | **4.94x** svelte, **0.71x** oxc-parser |
| parse css (49f) | **1.07x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (763f) | **27.0x** prettier |
| format typescript (2435f) | **21.7x** prettier |
| format css (49f) | **47.1x** prettier |
| parse svelte (763f) | **4.10x** svelte |
| parse typescript (2434f) | **4.58x** svelte |
| parse css (49f) | **0.99x** svelte |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no oxc counterpart — oxc exposes no comparably cheap mode (its JS API always serializes; experimentalLazy is setup-dominated). Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in `@fuzdev/tsv_parse_wasm` / `@fuzdev/tsv_wasm`) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

## Skipped Files

7 unique file+error combinations — Svelte 0, TypeScript 7, CSS 0.

**Per-benchmark skip counts:**
- parse/typescript: acorn-typescript: 3
- parse/typescript: oxc-parser: 2
- format/typescript: oxfmt: 2

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
