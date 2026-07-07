# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.8.3

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-07-07T20:02:16.112Z — tsv 0.1.0 (3ee86763)

**Corpus:** 762 Svelte (1.8 MB), 2315 TypeScript (16.1 MB), 50 CSS (0.3 MB) — 3127 files, 18.3 MB total

**Sources:** ../zzz/src (325), ../fuz_app/src (664), ../fuz_blog/src (32), ../fuz_code/src (62), ../fuz_css/src (135), ../fuz_docs/src (64), ../fuz_gitops/src (98), ../fuz_mastodon/src (24), ../fuz_template/src (15), ../fuz_ui/src (215), ../fuz_util/src (144), ../mdz/src (58), ../gro/src (155), ../svelte-docinfo/src (98), ../tsv.fuz.dev/src (27), ../ryanatkn.com/src (51), ../webdevladder.net/src (38), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (273), ../svelte/packages/svelte/src (380), ../svelte.dev/apps/svelte.dev/src (138), ../svelte.dev/packages/repl/src (48), ../svelte.dev/packages/site-kit/src (65)

**Versions:** svelte@5.56.4, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.139.0, oxfmt@0.57.0, @biomejs/wasm-bundler@2.5.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| -------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler            | 2.39       | 9   | 416.75   | 425.71   | 439.22   | —        | —        | 414.76   | 444.99   | baseline                     |
| tsv-json                   | 5.17       | 26  | 193.08   | 195.18   | 196.98   | 197.22   | 197.82   | 191.62   | 198.01   | 2.16x                        |
| tsv-json-no-locations      | 8.08       | 34  | 123.48   | 125.60   | 127.14   | 127.41   | 128.84   | 122.81   | 129.33   | 3.37x                        |
| tsv_wasm-json              | 4.26       | 20  | 234.51   | 235.92   | 240.28   | 241.96   | 242.25   | 232.98   | 242.31   | 1.78x                        |
| tsv_wasm-json-no-locations | 6.40       | 31  | 155.79   | 156.78   | 159.63   | 160.54   | 160.91   | 154.10   | 161.06   | 2.67x                        |
| tsv-internal               | 46.60      | 196 | 21.33    | 21.99    | 22.21    | 22.30    | 22.68    | 21.19    | 22.87    | 19.5x                        |
| tsv_wasm-internal          | 30.36      | 107 | 32.91    | 33.85    | 34.02    | 34.17    | 34.40    | 32.71    | 35.10    | 12.7x                        |

**Files (intersection):** 762

**Throughput:** svelte/compiler 4.4 MB/s, tsv-json 9.5 MB/s, tsv-json-no-locations 14.9 MB/s, tsv_wasm-json 7.9 MB/s, tsv_wasm-json-no-locations 11.8 MB/s, tsv-internal 86.0 MB/s, tsv_wasm-internal 56.1 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 9.0x tsv-internal, tsv_wasm-json 7.1x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.24       | 7  | 4175.72  | 4184.59  | 4189.06  | —        | —        | 4110.88  | 4193.64  | baseline              |
| tsv        | 12.76      | 62 | 77.89    | 79.37    | 80.34    | 80.44    | 81.50    | 77.30    | 82.48    | 53.1x                 |
| tsv_wasm   | 8.27       | 40 | 120.32   | 122.35   | 123.98   | 124.58   | 125.24   | 119.28   | 125.52   | 34.4x                 |
| oxfmt      | 0.25       | 4  | 4015.01  | 4020.59  | 4057.71  | —        | —        | 3985.75  | 4082.45  | 1.04x                 |
| biome-wasm | 1.47       | 6  | 681.35   | 686.90   | 700.91   | —        | —        | 678.14   | 708.59   | 6.12x                 |

**Files (intersection):** 762

**Throughput:** prettier 0.4 MB/s, tsv 23.6 MB/s, tsv_wasm 15.3 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.7 MB/s

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.37       | 4  | 2.68    | 2.68    | 2.68    | —       | —       | 2.68    | 2.68    | baseline                      |
| tsv-json                   | 0.57       | 5  | 1.74    | 1.75    | 1.76    | —       | —       | 1.74    | 1.76    | 1.53x                         |
| tsv-json-no-locations      | 1.18       | 5  | 0.85    | 0.85    | 0.85    | —       | —       | 0.84    | 0.86    | 3.17x                         |
| tsv_wasm-json              | 0.49       | 5  | 2.04    | 2.04    | 2.05    | —       | —       | 2.04    | 2.05    | 1.31x                         |
| tsv_wasm-json-no-locations | 0.97       | 4  | 1.03    | 1.03    | 1.04    | —       | —       | 1.03    | 1.04    | 2.60x                         |
| tsv-internal               | 7.44       | 31 | 0.13    | 0.14    | 0.14    | 0.14    | 0.14    | 0.13    | 0.14    | 19.9x                         |
| tsv_wasm-internal          | 4.92       | 20 | 0.20    | 0.20    | 0.21    | 0.21    | 0.21    | 0.20    | 0.21    | 13.2x                         |
| oxc-parser                 | 0.90       | 5  | 1.11    | 1.12    | 1.12    | —       | —       | 1.09    | 1.12    | 2.41x                         |
| oxc-parser-wasm            | 0.80       | 5  | 1.24    | 1.25    | 1.26    | —       | —       | 1.22    | 1.26    | 2.15x                         |

**Files (intersection):** 2315

**Throughput:** acorn-typescript 6.0 MB/s, tsv-json 9.2 MB/s, tsv-json-no-locations 19.1 MB/s, tsv_wasm-json 7.9 MB/s, tsv_wasm-json-no-locations 15.6 MB/s, tsv-internal 119.8 MB/s, tsv_wasm-internal 79.3 MB/s, oxc-parser 14.5 MB/s, oxc-parser-wasm 13.0 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 13.0x tsv-internal, tsv_wasm-json 10.1x tsv_wasm-internal

## format/typescript

| Task Name  | sweeps/sec | n | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | - | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.09       | 5 | 11475.24 | 11534.06 | 11569.22 | —        | —        | 11458.45 | 11572.65 | baseline              |
| tsv        | 1.92       | 9 | 519.74   | 521.79   | 524.63   | —        | —        | 518.28   | 536.66   | 22.1x                 |
| tsv_wasm   | 1.25       | 6 | 802.10   | 803.75   | 808.44   | —        | —        | 799.89   | 814.66   | 14.3x                 |
| oxfmt      | 1.14       | 6 | 868.83   | 883.06   | 888.10   | —        | —        | 865.66   | 888.48   | 13.1x                 |
| biome-wasm | 0.26       | 5 | 3914.35  | 3930.47  | 3930.94  | —        | —        | 3910.40  | 3931.25  | 2.93x                 |

**Files (intersection):** 2315

**Throughput:** prettier 1.4 MB/s, tsv 31.0 MB/s, tsv_wasm 20.1 MB/s, oxfmt 18.4 MB/s, biome-wasm 4.1 MB/s

## parse/css

| Task Name         | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 103.17     | 514 | 9.65     | 9.90     | 10.15    | 10.33    | 10.65    | 9.24     | 11.31    | baseline                     |
| tsv-json          | 58.40      | 261 | 17.06    | 17.39    | 18.16    | 18.84    | 19.44    | 16.79    | 28.43    | 0.57x                        |
| tsv_wasm-json     | 45.13      | 204 | 22.03    | 22.58    | 23.20    | 23.63    | 24.15    | 21.73    | 24.59    | 0.44x                        |
| tsv-internal      | 180.61     | 796 | 5.53     | 5.57     | 5.63     | 5.66     | 5.71     | 5.47     | 6.42     | 1.75x                        |
| tsv_wasm-internal | 114.48     | 486 | 8.72     | 8.81     | 8.87     | 8.90     | 9.00     | 8.69     | 9.14     | 1.11x                        |

**Files (intersection):** 50

**Throughput:** svelte/compiler 32.1 MB/s, tsv-json 18.2 MB/s, tsv_wasm-json 14.1 MB/s, tsv-internal 56.3 MB/s, tsv_wasm-internal 35.7 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 3.1x tsv-internal, tsv_wasm-json 2.5x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 2.00       | 9   | 497.04   | 505.81   | 508.78   | —        | —        | 493.29   | 515.92   | baseline              |
| tsv        | 103.13     | 470 | 9.69     | 9.79     | 10.03    | 10.14    | 10.30    | 9.56     | 10.72    | 51.5x                 |
| tsv_wasm   | 63.99      | 317 | 15.57    | 15.77    | 15.84    | 15.86    | 16.00    | 15.45    | 16.41    | 31.9x                 |
| oxfmt      | 11.43      | 58  | 87.63    | 89.43    | 90.74    | 91.29    | 93.45    | 81.81    | 93.68    | 5.71x                 |
| biome-wasm | 11.02      | 46  | 90.60    | 92.54    | 93.16    | 93.31    | 93.65    | 89.81    | 93.79    | 5.50x                 |

**Files (intersection):** 50

**Throughput:** prettier 0.6 MB/s, tsv 32.1 MB/s, tsv_wasm 19.9 MB/s, oxfmt 3.6 MB/s, biome-wasm 3.4 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 762.0 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.0 MB | 381.0 KB | 0.4x | 0.5x |
| tsv_wasm | 2.4 MB | 841.9 KB | — | — |
| biome (wasm) | 37.5 MB | 9.0 MB | 15.4x | 10.7x |
| oxc-parser (wasm) | 1.6 MB | 501.4 KB | 0.7x | 0.6x |
| tsv (ffi) | 3.3 MB | 1.4 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.5 MB | 4.6 MB | 3.4x | 3.2x |
| tsv format (ffi) | 3.1 MB | 1.3 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 691.2 KB | 0.5x | 0.5x |
| tsv (napi) | 3.5 MB | 1.5 MB | 1.0x | 1.0x |
| oxc-parser (napi) | 2.4 MB | 977.4 KB | 0.7x | 0.7x |
| oxfmt (napi) | 9.1 MB | 3.6 MB | 2.7x | 2.5x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (762f) | **53.1x** prettier, **51.1x** oxfmt |
| format typescript (2315f) | **22.1x** prettier, **1.68x** oxfmt |
| format css (50f) | **51.5x** prettier, **9.02x** oxfmt |
| parse svelte (762f) | **2.16x** svelte |
| parse typescript (2315f) | **1.53x** svelte, **0.64x** oxc-parser |
| parse css (50f) | **0.57x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (762f) | **34.4x** prettier, **5.63x** biome-wasm |
| format typescript (2315f) | **14.3x** prettier, **4.89x** biome-wasm |
| format css (50f) | **31.9x** prettier, **5.81x** biome-wasm |
| parse svelte (762f) | **1.78x** svelte |
| parse typescript (2315f) | **1.31x** svelte, **0.61x** oxc-parser-wasm |
| parse css (50f) | **0.44x** svelte |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no oxc counterpart — oxc exposes no comparably cheap mode (its JS API always serializes; experimentalLazy is setup-dominated). Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in `@fuzdev/tsv_parse_wasm` / `@fuzdev/tsv_wasm`) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._
