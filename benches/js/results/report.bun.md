# tsv benchmark results

**Runtime:** bun

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-07-06T22:45:31.629Z — tsv 0.1.0 (a99ef299)

**Corpus:** 762 Svelte (1.8 MB), 2302 TypeScript (16.1 MB), 50 CSS (0.3 MB) — 3114 files, 18.2 MB total

**Sources:** ../zzz/src (325), ../fuz_app/src (664), ../fuz_blog/src (32), ../fuz_code/src (62), ../fuz_css/src (122), ../fuz_docs/src (64), ../fuz_gitops/src (98), ../fuz_mastodon/src (24), ../fuz_template/src (15), ../fuz_ui/src (215), ../fuz_util/src (144), ../mdz/src (58), ../gro/src (155), ../svelte-docinfo/src (98), ../tsv.fuz.dev/src (27), ../ryanatkn.com/src (51), ../webdevladder.net/src (38), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (273), ../svelte/packages/svelte/src (380), ../svelte.dev/apps/svelte.dev/src (138), ../svelte.dev/packages/repl/src (48), ../svelte.dev/packages/site-kit/src (65)

**Versions:** svelte@5.56.4, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.139.0, oxfmt@0.57.0

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| -------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler            | 1.39       | 7   | 708.77   | 724.79   | 737.67   | —        | —        | 699.07   | 753.00   | baseline                     |
| tsv-json                   | 6.09       | 30  | 164.37   | 165.94   | 169.85   | 171.00   | 173.34   | 158.35   | 174.03   | 4.37x                        |
| tsv-json-no-locations      | 8.52       | 42  | 117.42   | 119.32   | 120.35   | 123.04   | 126.00   | 113.91   | 127.63   | 6.11x                        |
| tsv_wasm-json              | 5.64       | 27  | 177.46   | 179.07   | 181.24   | 186.01   | 201.11   | 172.13   | 205.96   | 4.05x                        |
| tsv_wasm-json-no-locations | 7.67       | 33  | 130.77   | 132.84   | 158.40   | 162.13   | 166.28   | 126.56   | 168.58   | 5.50x                        |
| tsv-internal               | 49.45      | 243 | 20.06    | 20.58    | 20.98    | 21.08    | 21.74    | 19.66    | 22.32    | 35.5x                        |
| tsv_wasm-internal          | 15.57      | 48  | 64.02    | 65.94    | 93.36    | 94.05    | 99.29    | 60.95    | 103.41   | 11.2x                        |

**Files (intersection):** 762

**Throughput:** svelte/compiler 2.6 MB/s, tsv-json 11.2 MB/s, tsv-json-no-locations 15.7 MB/s, tsv_wasm-json 10.4 MB/s, tsv_wasm-json-no-locations 14.1 MB/s, tsv-internal 91.0 MB/s, tsv_wasm-internal 28.7 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 8.1x tsv-internal, tsv_wasm-json 2.8x tsv_wasm-internal

## format/svelte

| Task Name | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.27       | 7  | 3.74    | 3.78    | 3.87    | —       | —       | 3.67    | 3.95    | baseline              |
| tsv       | 12.56      | 61 | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 47.2x                 |
| tsv_wasm  | 9.22       | 37 | 0.11    | 0.11    | 0.14    | 0.14    | 0.15    | 0.11    | 0.15    | 34.6x                 |
| oxfmt     | 0.21       | 7  | 4.76    | 4.80    | 4.91    | —       | —       | 4.59    | 5.03    | 0.79x                 |

**Files (intersection):** 762

**Throughput:** prettier 0.5 MB/s, tsv 23.1 MB/s, tsv_wasm 17.0 MB/s, oxfmt 0.4 MB/s

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- |
| acorn-typescript           | 0.17       | 7  | 5974.74  | 6010.80  | 6043.44  | —        | —        | 5911.78  | 6066.81  | baseline                      |
| tsv-json                   | 0.76       | 5  | 1317.57  | 1319.00  | 1319.16  | —        | —        | 1312.51  | 1319.27  | 4.54x                         |
| tsv-json-no-locations      | 1.32       | 6  | 759.61   | 762.86   | 770.00   | —        | —        | 755.36   | 778.22   | 7.88x                         |
| tsv_wasm-json              | 0.70       | 3  | 1430.29  | 1431.01  | 1431.27  | —        | —        | 1430.29  | 1431.45  | 4.18x                         |
| tsv_wasm-json-no-locations | 1.20       | 7  | 832.89   | 833.40   | 836.88   | —        | —        | 824.40   | 841.59   | 7.19x                         |
| tsv-internal               | 8.24       | 41 | 121.00   | 122.11   | 122.86   | 123.02   | 123.88   | 120.05   | 124.11   | 49.3x                         |
| tsv_wasm-internal          | 5.64       | 29 | 176.70   | 177.99   | 179.64   | 180.24   | 180.72   | 175.17   | 180.83   | 33.7x                         |
| oxc-parser                 | 1.07       | 5  | 935.36   | 936.47   | 938.04   | —        | —        | 934.13   | 939.34   | 6.39x                         |

**Files (intersection):** 2302

**Throughput:** acorn-typescript 2.7 MB/s, tsv-json 12.2 MB/s, tsv-json-no-locations 21.1 MB/s, tsv_wasm-json 11.2 MB/s, tsv_wasm-json-no-locations 19.3 MB/s, tsv-internal 132.4 MB/s, tsv_wasm-internal 90.6 MB/s, oxc-parser 17.2 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 10.9x tsv-internal, tsv_wasm-json 8.1x tsv_wasm-internal

## format/typescript

| Task Name | sweeps/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ---------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.07       | 7 | 14.50   | 14.59   | 14.70   | —       | —       | 13.92   | 14.82   | baseline              |
| tsv       | 1.88       | 8 | 0.53    | 0.53    | 0.54    | —       | —       | 0.53    | 0.55    | 27.1x                 |
| tsv_wasm  | 1.39       | 6 | 0.72    | 0.72    | 0.73    | —       | —       | 0.71    | 0.75    | 20.0x                 |
| oxfmt     | 0.95       | 4 | 1.05    | 1.06    | 1.07    | —       | —       | 1.05    | 1.08    | 13.7x                 |

**Files (intersection):** 2302

**Throughput:** prettier 1.1 MB/s, tsv 30.2 MB/s, tsv_wasm 22.4 MB/s, oxfmt 15.3 MB/s

## parse/css

| Task Name         | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 77.94      | 306 | 12.85    | 13.05    | 14.00    | 36.51    | 43.60    | 12.43    | 44.50    | baseline                     |
| tsv-json          | 64.03      | 310 | 15.61    | 16.17    | 16.50    | 16.91    | 19.57    | 14.46    | 29.87    | 0.82x                        |
| tsv_wasm-json     | 57.59      | 279 | 17.24    | 17.67    | 18.48    | 18.69    | 21.57    | 16.49    | 22.47    | 0.74x                        |
| tsv-internal      | 200.73     | 863 | 4.97     | 5.02     | 5.08     | 5.11     | 5.16     | 4.94     | 6.20     | 2.58x                        |
| tsv_wasm-internal | 134.13     | 656 | 7.44     | 7.50     | 7.57     | 7.60     | 7.68     | 7.37     | 8.33     | 1.72x                        |

**Files (intersection):** 50

**Throughput:** svelte/compiler 23.3 MB/s, tsv-json 19.1 MB/s, tsv_wasm-json 17.2 MB/s, tsv-internal 60.0 MB/s, tsv_wasm-internal 40.1 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 3.1x tsv-internal, tsv_wasm-json 2.3x tsv_wasm-internal

## format/css

| Task Name | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| --------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier  | 2.14       | 11  | 462.25   | 488.20   | 491.98   | 492.64   | 493.16   | 447.64   | 493.29   | baseline              |
| tsv       | 106.12     | 521 | 9.40     | 9.53     | 9.61     | 9.69     | 10.18    | 9.14     | 13.42    | 49.6x                 |
| tsv_wasm  | 74.98      | 373 | 13.31    | 13.47    | 13.54    | 13.59    | 13.72    | 13.12    | 15.08    | 35.1x                 |
| oxfmt     | 10.32      | 52  | 96.72    | 100.07   | 103.90   | 106.94   | 109.87   | 86.05    | 110.20   | 4.83x                 |

**Files (intersection):** 50

**Throughput:** prettier 0.6 MB/s, tsv 31.7 MB/s, tsv_wasm 22.4 MB/s, oxfmt 3.1 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 762.6 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.0 MB | 381.0 KB | 0.4x | 0.5x |
| tsv_wasm | 2.4 MB | 841.8 KB | — | — |
| oxc-parser (wasm) | 1.6 MB | 501.4 KB | 0.7x | 0.6x |
| tsv (ffi) | 3.3 MB | 1.4 MB | 1.0x | 1.0x |
| tsv format (ffi) | 3.1 MB | 1.3 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 691.2 KB | 0.5x | 0.5x |
| tsv (napi) | 3.5 MB | 1.5 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.5 MB | 4.6 MB | 3.3x | 3.1x |
| oxc-parser (napi) | 2.4 MB | 977.4 KB | 0.7x | 0.7x |
| oxfmt (napi) | 9.1 MB | 3.6 MB | 2.6x | 2.4x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (762f) | **47.2x** prettier, **59.9x** oxfmt |
| format typescript (2302f) | **27.1x** prettier, **1.98x** oxfmt |
| format css (50f) | **49.6x** prettier, **10.3x** oxfmt |
| parse svelte (762f) | **4.37x** svelte |
| parse typescript (2302f) | **4.54x** svelte, **0.71x** oxc-parser |
| parse css (50f) | **0.82x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (762f) | **34.6x** prettier |
| format typescript (2302f) | **20.0x** prettier |
| format css (50f) | **35.1x** prettier |
| parse svelte (762f) | **4.05x** svelte |
| parse typescript (2302f) | **4.18x** svelte |
| parse css (50f) | **0.74x** svelte |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no oxc counterpart — oxc exposes no comparably cheap mode (its JS API always serializes; experimentalLazy is setup-dominated). Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in `@fuzdev/tsv_parse_wasm` / `@fuzdev/tsv_wasm`) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._
