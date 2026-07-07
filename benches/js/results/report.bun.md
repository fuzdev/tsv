# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.3.14

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-07-07T20:20:23.755Z — tsv 0.1.0 (3ee86763)

**Corpus:** 762 Svelte (1.8 MB), 2315 TypeScript (16.1 MB), 50 CSS (0.3 MB) — 3127 files, 18.3 MB total

**Sources:** ../zzz/src (325), ../fuz_app/src (664), ../fuz_blog/src (32), ../fuz_code/src (62), ../fuz_css/src (135), ../fuz_docs/src (64), ../fuz_gitops/src (98), ../fuz_mastodon/src (24), ../fuz_template/src (15), ../fuz_ui/src (215), ../fuz_util/src (144), ../mdz/src (58), ../gro/src (155), ../svelte-docinfo/src (98), ../tsv.fuz.dev/src (27), ../ryanatkn.com/src (51), ../webdevladder.net/src (38), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (273), ../svelte/packages/svelte/src (380), ../svelte.dev/apps/svelte.dev/src (138), ../svelte.dev/packages/repl/src (48), ../svelte.dev/packages/site-kit/src (65)

**Versions:** svelte@5.56.4, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.139.0, oxfmt@0.57.0

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| -------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler            | 1.43       | 8   | 699.06   | 710.21   | 722.50   | —        | —        | 681.54   | 732.55   | baseline                     |
| tsv-json                   | 6.14       | 31  | 162.91   | 165.10   | 166.62   | 167.47   | 168.03   | 157.26   | 168.20   | 4.31x                        |
| tsv-json-no-locations      | 8.58       | 42  | 116.52   | 118.23   | 120.85   | 122.01   | 127.89   | 112.13   | 131.66   | 6.02x                        |
| tsv_wasm-json              | 5.67       | 28  | 176.54   | 178.07   | 180.05   | 180.23   | 197.73   | 172.38   | 204.49   | 3.97x                        |
| tsv_wasm-json-no-locations | 7.71       | 34  | 129.56   | 132.76   | 144.79   | 158.69   | 161.62   | 124.99   | 162.65   | 5.41x                        |
| tsv-internal               | 49.39      | 235 | 20.06    | 20.75    | 20.92    | 21.03    | 21.39    | 19.81    | 21.55    | 34.6x                        |
| tsv_wasm-internal          | 15.67      | 60  | 63.56    | 65.10    | 89.00    | 94.89    | 100.76   | 57.79    | 102.63   | 11.0x                        |

**Files (intersection):** 762

**Throughput:** svelte/compiler 2.6 MB/s, tsv-json 11.3 MB/s, tsv-json-no-locations 15.8 MB/s, tsv_wasm-json 10.5 MB/s, tsv_wasm-json-no-locations 14.2 MB/s, tsv-internal 91.2 MB/s, tsv_wasm-internal 28.9 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 8.0x tsv-internal, tsv_wasm-json 2.8x tsv_wasm-internal

## format/svelte

| Task Name | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.28       | 7  | 3.59    | 3.67    | 3.71    | —       | —       | 3.53    | 3.76    | baseline              |
| tsv       | 12.77      | 52 | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 46.3x                 |
| tsv_wasm  | 9.31       | 36 | 0.11    | 0.11    | 0.14    | 0.14    | 0.17    | 0.10    | 0.19    | 33.7x                 |
| oxfmt     | 0.21       | 7  | 4.79    | 4.80    | 4.88    | —       | —       | 4.60    | 4.99    | 0.76x                 |

**Files (intersection):** 762

**Throughput:** prettier 0.5 MB/s, tsv 23.6 MB/s, tsv_wasm 17.2 MB/s, oxfmt 0.4 MB/s

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- |
| acorn-typescript           | 0.16       | 7  | 6075.18  | 6088.80  | 6105.69  | —        | —        | 5974.61  | 6119.37  | baseline                      |
| tsv-json                   | 0.77       | 5  | 1303.03  | 1305.68  | 1307.12  | —        | —        | 1289.41  | 1308.09  | 4.66x                         |
| tsv-json-no-locations      | 1.34       | 6  | 746.61   | 747.64   | 751.78   | —        | —        | 745.03   | 757.95   | 8.12x                         |
| tsv_wasm-json              | 0.71       | 5  | 1410.74  | 1411.21  | 1412.38  | —        | —        | 1398.79  | 1413.16  | 4.31x                         |
| tsv_wasm-json-no-locations | 1.21       | 7  | 827.64   | 829.69   | 831.75   | —        | —        | 820.88   | 833.91   | 7.33x                         |
| tsv-internal               | 8.32       | 32 | 120.08   | 121.59   | 123.16   | 123.29   | 123.43   | 119.70   | 123.52   | 50.5x                         |
| tsv_wasm-internal          | 5.62       | 23 | 177.85   | 179.28   | 182.35   | 182.45   | 182.45   | 176.95   | 182.45   | 34.1x                         |
| oxc-parser                 | 1.09       | 6  | 916.11   | 920.33   | 923.58   | —        | —        | 907.81   | 926.54   | 6.62x                         |

**Files (intersection):** 2315

**Throughput:** acorn-typescript 2.7 MB/s, tsv-json 12.4 MB/s, tsv-json-no-locations 21.6 MB/s, tsv_wasm-json 11.4 MB/s, tsv_wasm-json-no-locations 19.5 MB/s, tsv-internal 134.1 MB/s, tsv_wasm-internal 90.6 MB/s, oxc-parser 17.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 10.8x tsv-internal, tsv_wasm-json 7.9x tsv_wasm-internal

## format/typescript

| Task Name | sweeps/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ---------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.07       | 7 | 13.83   | 14.36   | 14.48   | —       | —       | 13.34   | 14.56   | baseline              |
| tsv       | 1.90       | 7 | 0.53    | 0.53    | 0.53    | —       | —       | 0.52    | 0.54    | 26.5x                 |
| tsv_wasm  | 1.40       | 6 | 0.72    | 0.72    | 0.72    | —       | —       | 0.71    | 0.73    | 19.5x                 |
| oxfmt     | 0.95       | 5 | 1.05    | 1.06    | 1.06    | —       | —       | 1.03    | 1.07    | 13.3x                 |

**Files (intersection):** 2315

**Throughput:** prettier 1.2 MB/s, tsv 30.7 MB/s, tsv_wasm 22.5 MB/s, oxfmt 15.4 MB/s

## parse/css

| Task Name         | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 73.69      | 304 | 13.55    | 13.80    | 15.12    | 25.39    | 32.57    | 13.22    | 33.49    | baseline                     |
| tsv-json          | 63.61      | 308 | 15.78    | 16.16    | 16.59    | 16.89    | 19.86    | 14.78    | 26.80    | 0.86x                        |
| tsv_wasm-json     | 56.72      | 272 | 17.50    | 17.99    | 18.69    | 18.92    | 21.10    | 16.89    | 22.62    | 0.77x                        |
| tsv-internal      | 189.03     | 786 | 5.28     | 5.33     | 5.38     | 5.40     | 5.44     | 5.26     | 9.67     | 2.57x                        |
| tsv_wasm-internal | 129.33     | 641 | 7.71     | 7.77     | 7.86     | 7.89     | 7.93     | 7.65     | 8.17     | 1.76x                        |

**Files (intersection):** 50

**Throughput:** svelte/compiler 22.9 MB/s, tsv-json 19.8 MB/s, tsv_wasm-json 17.7 MB/s, tsv-internal 58.9 MB/s, tsv_wasm-internal 40.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 3.0x tsv-internal, tsv_wasm-json 2.3x tsv_wasm-internal

## format/css

| Task Name | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| --------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier  | 2.21       | 10  | 449.53   | 461.80   | 467.34   | 480.85   | 491.66   | 437.00   | 494.36   | baseline              |
| tsv       | 103.65     | 508 | 9.62     | 9.73     | 9.84     | 9.91     | 10.21    | 9.35     | 14.46    | 46.8x                 |
| tsv_wasm  | 73.07      | 362 | 13.65    | 13.81    | 13.87    | 13.93    | 14.06    | 13.47    | 17.32    | 33.0x                 |
| oxfmt     | 10.31      | 52  | 97.35    | 99.54    | 102.19   | 105.42   | 106.78   | 86.44    | 107.62   | 4.66x                 |

**Files (intersection):** 50

**Throughput:** prettier 0.7 MB/s, tsv 32.3 MB/s, tsv_wasm 22.8 MB/s, oxfmt 3.2 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 762.0 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.0 MB | 381.0 KB | 0.4x | 0.5x |
| tsv_wasm | 2.4 MB | 841.9 KB | — | — |
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
| format svelte (762f) | **46.3x** prettier, **60.7x** oxfmt |
| format typescript (2315f) | **26.5x** prettier, **2.00x** oxfmt |
| format css (50f) | **46.8x** prettier, **10.1x** oxfmt |
| parse svelte (762f) | **4.31x** svelte |
| parse typescript (2315f) | **4.66x** svelte, **0.70x** oxc-parser |
| parse css (50f) | **0.86x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (762f) | **33.7x** prettier |
| format typescript (2315f) | **19.5x** prettier |
| format css (50f) | **33.0x** prettier |
| parse svelte (762f) | **3.97x** svelte |
| parse typescript (2315f) | **4.31x** svelte |
| parse css (50f) | **0.77x** svelte |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no oxc counterpart — oxc exposes no comparably cheap mode (its JS API always serializes; experimentalLazy is setup-dominated). Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in `@fuzdev/tsv_parse_wasm` / `@fuzdev/tsv_wasm`) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._
