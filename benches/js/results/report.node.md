# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-07-07T20:11:53.386Z — tsv 0.1.0 (3ee86763)

**Corpus:** 762 Svelte (1.8 MB), 2315 TypeScript (16.1 MB), 50 CSS (0.3 MB) — 3127 files, 18.3 MB total

**Sources:** ../zzz/src (325), ../fuz_app/src (664), ../fuz_blog/src (32), ../fuz_code/src (62), ../fuz_css/src (135), ../fuz_docs/src (64), ../fuz_gitops/src (98), ../fuz_mastodon/src (24), ../fuz_template/src (15), ../fuz_ui/src (215), ../fuz_util/src (144), ../mdz/src (58), ../gro/src (155), ../svelte-docinfo/src (98), ../tsv.fuz.dev/src (27), ../ryanatkn.com/src (51), ../webdevladder.net/src (38), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (273), ../svelte/packages/svelte/src (380), ../svelte.dev/apps/svelte.dev/src (138), ../svelte.dev/packages/repl/src (48), ../svelte.dev/packages/site-kit/src (65)

**Versions:** svelte@5.56.4, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.139.0, oxfmt@0.57.0, @biomejs/wasm-bundler@2.5.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| -------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler            | 2.28       | 12  | 436.42   | 442.83   | 447.72   | 448.41   | 448.95   | 433.11   | 449.09   | baseline                     |
| tsv-json                   | 4.67       | 24  | 212.02   | 219.22   | 220.12   | 222.44   | 222.89   | 208.08   | 222.90   | 2.05x                        |
| tsv-json-no-locations      | 7.15       | 36  | 139.70   | 141.41   | 142.36   | 143.62   | 144.82   | 134.31   | 144.91   | 3.14x                        |
| tsv_wasm-json              | 4.14       | 21  | 242.83   | 244.51   | 245.11   | 245.16   | 245.29   | 234.45   | 245.32   | 1.82x                        |
| tsv_wasm-json-no-locations | 6.43       | 33  | 154.83   | 157.06   | 160.03   | 161.03   | 162.96   | 151.10   | 163.58   | 2.82x                        |
| tsv-internal               | 45.53      | 227 | 22.06    | 22.35    | 22.49    | 22.62    | 22.77    | 21.04    | 24.98    | 20.0x                        |
| tsv_wasm-internal          | 23.25      | 95  | 42.54    | 43.48    | 44.13    | 44.89    | 46.31    | 39.15    | 47.42    | 10.2x                        |

**Files (intersection):** 762

**Throughput:** svelte/compiler 4.2 MB/s, tsv-json 8.6 MB/s, tsv-json-no-locations 13.2 MB/s, tsv_wasm-json 7.6 MB/s, tsv_wasm-json-no-locations 11.9 MB/s, tsv-internal 84.1 MB/s, tsv_wasm-internal 42.9 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 9.7x tsv-internal, tsv_wasm-json 5.6x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.23       | 7  | 4251.76  | 4269.34  | 4279.76  | —        | —        | 4238.37  | 4284.34  | baseline              |
| tsv        | 13.08      | 46 | 76.40    | 78.66    | 79.27    | 79.57    | 81.14    | 75.92    | 81.46    | 55.7x                 |
| tsv_wasm   | 8.52       | 41 | 116.66   | 119.92   | 121.05   | 122.44   | 126.17   | 114.79   | 126.33   | 36.3x                 |
| oxfmt      | 0.24       | 5  | 4147.74  | 4149.77  | 4158.96  | —        | —        | 4109.22  | 4165.09  | 1.03x                 |
| biome-wasm | 1.13       | 6  | 888.30   | 892.31   | 897.98   | —        | —        | 871.07   | 902.50   | 4.79x                 |

**Files (intersection):** 762

**Throughput:** prettier 0.4 MB/s, tsv 24.1 MB/s, tsv_wasm 15.7 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.1 MB/s

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.34       | 5  | 2.95    | 2.96    | 2.96    | —       | —       | 2.94    | 2.96    | baseline                      |
| tsv-json                   | 0.52       | 5  | 1.93    | 1.93    | 1.94    | —       | —       | 1.93    | 1.94    | 1.53x                         |
| tsv-json-no-locations      | 1.05       | 5  | 0.95    | 0.95    | 0.95    | —       | —       | 0.95    | 0.95    | 3.11x                         |
| tsv_wasm-json              | 0.49       | 5  | 2.06    | 2.06    | 2.06    | —       | —       | 2.05    | 2.07    | 1.43x                         |
| tsv_wasm-json-no-locations | 0.96       | 5  | 1.04    | 1.04    | 1.04    | —       | —       | 1.03    | 1.04    | 2.85x                         |
| tsv-internal               | 7.29       | 29 | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 21.5x                         |
| tsv_wasm-internal          | 5.55       | 23 | 0.18    | 0.18    | 0.18    | 0.18    | 0.18    | 0.18    | 0.18    | 16.4x                         |
| oxc-parser                 | 0.81       | 5  | 1.24    | 1.24    | 1.25    | —       | —       | 1.23    | 1.25    | 2.38x                         |
| oxc-parser-wasm            | 0.78       | 4  | 1.28    | 1.28    | 1.28    | —       | —       | 1.28    | 1.28    | 2.31x                         |

**Files (intersection):** 2315

**Throughput:** acorn-typescript 5.5 MB/s, tsv-json 8.3 MB/s, tsv-json-no-locations 17.0 MB/s, tsv_wasm-json 7.8 MB/s, tsv_wasm-json-no-locations 15.5 MB/s, tsv-internal 117.4 MB/s, tsv_wasm-internal 89.3 MB/s, oxc-parser 13.0 MB/s, oxc-parser-wasm 12.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 14.1x tsv-internal, tsv_wasm-json 11.4x tsv_wasm-internal

## format/typescript

| Task Name  | sweeps/sec | n | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | - | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.08       | 7 | 12599.89 | 12609.09 | 12609.86 | —        | —        | 12575.38 | 12610.94 | baseline              |
| tsv        | 1.91       | 9 | 522.69   | 524.45   | 528.44   | —        | —        | 520.59   | 537.62   | 24.1x                 |
| tsv_wasm   | 1.38       | 6 | 727.43   | 727.53   | 735.25   | —        | —        | 725.27   | 746.83   | 17.3x                 |
| oxfmt      | 1.13       | 6 | 881.10   | 883.09   | 888.35   | —        | —        | 871.65   | 893.26   | 14.3x                 |
| biome-wasm | 0.24       | 4 | 4283.66  | 4295.22  | 7866.47  | —        | —        | 4201.73  | 10247.30 | 2.96x                 |

**Files (intersection):** 2315

**Throughput:** prettier 1.3 MB/s, tsv 30.8 MB/s, tsv_wasm 22.2 MB/s, oxfmt 18.3 MB/s, biome-wasm 3.8 MB/s

## parse/css

| Task Name         | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 115.63     | 539 | 8.55     | 8.93     | 9.18     | 9.53     | 13.95    | 8.31     | 17.97    | baseline                     |
| tsv-json          | 51.78      | 235 | 19.25    | 19.72    | 20.01    | 21.58    | 22.87    | 18.53    | 25.43    | 0.45x                        |
| tsv_wasm-json     | 47.29      | 203 | 21.13    | 21.52    | 22.77    | 23.39    | 25.68    | 20.74    | 26.94    | 0.41x                        |
| tsv-internal      | 175.66     | 668 | 5.69     | 5.75     | 5.83     | 5.86     | 5.91     | 5.66     | 6.64     | 1.52x                        |
| tsv_wasm-internal | 121.40     | 597 | 8.20     | 8.30     | 8.35     | 8.37     | 8.45     | 8.14     | 8.72     | 1.05x                        |

**Files (intersection):** 50

**Throughput:** svelte/compiler 36.0 MB/s, tsv-json 16.1 MB/s, tsv_wasm-json 14.7 MB/s, tsv-internal 54.7 MB/s, tsv_wasm-internal 37.8 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 3.4x tsv-internal, tsv_wasm-json 2.6x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.91       | 10  | 519.47   | 531.05   | 541.14   | 541.38   | 541.58   | 511.75   | 541.63   | baseline              |
| tsv        | 100.34     | 487 | 9.92     | 10.06    | 10.15    | 10.19    | 10.25    | 9.83     | 13.87    | 52.5x                 |
| tsv_wasm   | 70.25      | 337 | 14.17    | 14.39    | 14.46    | 14.50    | 14.54    | 14.06    | 17.98    | 36.8x                 |
| oxfmt      | 11.32      | 57  | 88.29    | 90.86    | 92.17    | 92.80    | 93.37    | 82.21    | 93.45    | 5.92x                 |
| biome-wasm | 5.98       | 30  | 167.62   | 172.57   | 176.01   | 187.20   | 191.36   | 135.57   | 192.77   | 3.13x                 |

**Files (intersection):** 50

**Throughput:** prettier 0.6 MB/s, tsv 31.2 MB/s, tsv_wasm 21.9 MB/s, oxfmt 3.5 MB/s, biome-wasm 1.9 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 762.0 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.0 MB | 381.0 KB | 0.4x | 0.5x |
| tsv_wasm | 2.4 MB | 841.9 KB | — | — |
| biome (wasm) | 37.5 MB | 9.0 MB | 15.4x | 10.7x |
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
| format svelte (762f) | **55.7x** prettier, **54.1x** oxfmt |
| format typescript (2315f) | **24.1x** prettier, **1.69x** oxfmt |
| format css (50f) | **52.5x** prettier, **8.86x** oxfmt |
| parse svelte (762f) | **2.05x** svelte |
| parse typescript (2315f) | **1.53x** svelte, **0.64x** oxc-parser |
| parse css (50f) | **0.45x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (762f) | **36.3x** prettier, **7.57x** biome-wasm |
| format typescript (2315f) | **17.3x** prettier, **5.85x** biome-wasm |
| format css (50f) | **36.8x** prettier, **11.8x** biome-wasm |
| parse svelte (762f) | **1.82x** svelte |
| parse typescript (2315f) | **1.43x** svelte, **0.62x** oxc-parser-wasm |
| parse css (50f) | **0.41x** svelte |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no oxc counterpart — oxc exposes no comparably cheap mode (its JS API always serializes; experimentalLazy is setup-dominated). Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in `@fuzdev/tsv_parse_wasm` / `@fuzdev/tsv_wasm`) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._
