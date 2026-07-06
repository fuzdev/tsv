# tsv benchmark results

**Runtime:** node

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-07-06T22:36:55.632Z — tsv 0.1.0 (a99ef299)

**Corpus:** 762 Svelte (1.8 MB), 2302 TypeScript (16.1 MB), 50 CSS (0.3 MB) — 3114 files, 18.2 MB total

**Sources:** ../zzz/src (325), ../fuz_app/src (664), ../fuz_blog/src (32), ../fuz_code/src (62), ../fuz_css/src (122), ../fuz_docs/src (64), ../fuz_gitops/src (98), ../fuz_mastodon/src (24), ../fuz_template/src (15), ../fuz_ui/src (215), ../fuz_util/src (144), ../mdz/src (58), ../gro/src (155), ../svelte-docinfo/src (98), ../tsv.fuz.dev/src (27), ../ryanatkn.com/src (51), ../webdevladder.net/src (38), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (273), ../svelte/packages/svelte/src (380), ../svelte.dev/apps/svelte.dev/src (138), ../svelte.dev/packages/repl/src (48), ../svelte.dev/packages/site-kit/src (65)

**Versions:** svelte@5.56.4, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.139.0, oxfmt@0.57.0, @biomejs/wasm-bundler@2.5.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| -------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler            | 2.22       | 11  | 449.55   | 457.42   | 459.24   | 462.47   | 465.60   | 444.97   | 466.38   | baseline                     |
| tsv-json                   | 4.72       | 20  | 212.13   | 212.77   | 216.52   | 216.91   | 217.34   | 210.58   | 217.45   | 2.13x                        |
| tsv-json-no-locations      | 7.39       | 36  | 134.87   | 136.08   | 138.37   | 138.73   | 139.51   | 132.87   | 139.83   | 3.33x                        |
| tsv_wasm-json              | 4.24       | 18  | 236.03   | 237.12   | 240.49   | 242.54   | 242.85   | 234.37   | 242.90   | 1.91x                        |
| tsv_wasm-json-no-locations | 6.49       | 33  | 153.50   | 155.43   | 157.24   | 158.04   | 159.05   | 151.33   | 159.41   | 2.93x                        |
| tsv-internal               | 46.17      | 227 | 21.49    | 22.08    | 22.39    | 22.51    | 22.78    | 21.14    | 24.60    | 20.8x                        |
| tsv_wasm-internal          | 22.84      | 90  | 43.25    | 44.24    | 45.27    | 47.37    | 49.88    | 38.53    | 55.02    | 10.3x                        |

**Files (intersection):** 762

**Throughput:** svelte/compiler 4.1 MB/s, tsv-json 8.7 MB/s, tsv-json-no-locations 13.6 MB/s, tsv_wasm-json 7.8 MB/s, tsv_wasm-json-no-locations 11.9 MB/s, tsv-internal 85.0 MB/s, tsv_wasm-internal 42.0 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 9.8x tsv-internal, tsv_wasm-json 5.4x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.23       | 7  | 4427.03  | 4451.27  | 4461.50  | —        | —        | 4411.51  | 4475.01  | baseline              |
| tsv        | 12.88      | 63 | 77.18    | 78.70    | 79.40    | 79.86    | 80.30    | 76.53    | 80.44    | 57.1x                 |
| tsv_wasm   | 8.44       | 43 | 117.70   | 119.63   | 120.93   | 122.63   | 123.68   | 116.21   | 123.94   | 37.5x                 |
| oxfmt      | 0.24       | 5  | 4170.05  | 4204.78  | 4212.29  | —        | —        | 4153.39  | 4217.29  | 1.06x                 |
| biome-wasm | 1.13       | 6  | 887.63   | 891.37   | 893.08   | —        | —        | 872.01   | 894.33   | 5.02x                 |

**Files (intersection):** 762

**Throughput:** prettier 0.4 MB/s, tsv 23.7 MB/s, tsv_wasm 15.5 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.1 MB/s

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.33       | 5  | 2.99    | 2.99    | 3.00    | —       | —       | 2.98    | 3.00    | baseline                      |
| tsv-json                   | 0.51       | 4  | 1.94    | 1.94    | 1.95    | —       | —       | 1.94    | 1.95    | 1.54x                         |
| tsv-json-no-locations      | 1.04       | 6  | 0.96    | 0.97    | 0.97    | —       | —       | 0.96    | 0.97    | 3.10x                         |
| tsv_wasm-json              | 0.48       | 5  | 2.07    | 2.08    | 2.08    | —       | —       | 2.06    | 2.08    | 1.44x                         |
| tsv_wasm-json-no-locations | 0.95       | 5  | 1.05    | 1.05    | 1.05    | —       | —       | 1.05    | 1.05    | 2.84x                         |
| tsv-internal               | 7.22       | 36 | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 21.6x                         |
| tsv_wasm-internal          | 5.47       | 24 | 0.18    | 0.18    | 0.19    | 0.19    | 0.19    | 0.18    | 0.19    | 16.4x                         |
| oxc-parser                 | 0.79       | 4  | 1.26    | 1.26    | 1.27    | —       | —       | 1.26    | 1.27    | 2.37x                         |
| oxc-parser-wasm            | 0.77       | 5  | 1.30    | 1.30    | 1.30    | —       | —       | 1.29    | 1.30    | 2.30x                         |

**Files (intersection):** 2302

**Throughput:** acorn-typescript 5.4 MB/s, tsv-json 8.3 MB/s, tsv-json-no-locations 16.7 MB/s, tsv_wasm-json 7.8 MB/s, tsv_wasm-json-no-locations 15.3 MB/s, tsv-internal 115.9 MB/s, tsv_wasm-internal 87.9 MB/s, oxc-parser 12.7 MB/s, oxc-parser-wasm 12.4 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 14.0x tsv-internal, tsv_wasm-json 11.3x tsv_wasm-internal

## format/typescript

| Task Name  | sweeps/sec | n | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | - | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.08       | 7 | 13176.89 | 13222.89 | 13254.46 | —        | —        | 13135.27 | 13258.37 | baseline              |
| tsv        | 1.91       | 8 | 522.44   | 523.66   | 528.58   | —        | —        | 521.25   | 537.16   | 25.3x                 |
| tsv_wasm   | 1.38       | 6 | 725.83   | 727.15   | 734.10   | —        | —        | 723.83   | 744.27   | 18.2x                 |
| oxfmt      | 1.11       | 4 | 904.23   | 905.37   | 916.45   | —        | —        | 903.20   | 927.44   | 14.6x                 |
| biome-wasm | 0.23       | 4 | 4302.99  | 4343.03  | 7549.46  | —        | —        | 4235.71  | 9687.09  | 3.07x                 |

**Files (intersection):** 2302

**Throughput:** prettier 1.2 MB/s, tsv 30.7 MB/s, tsv_wasm 22.1 MB/s, oxfmt 17.8 MB/s, biome-wasm 3.7 MB/s

## parse/css

| Task Name         | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 122.15     | 571 | 8.11     | 8.43     | 8.78     | 9.08     | 13.41    | 7.75     | 16.54    | baseline                     |
| tsv-json          | 51.49      | 233 | 19.42    | 19.75    | 20.03    | 22.58    | 23.11    | 18.53    | 26.87    | 0.42x                        |
| tsv_wasm-json     | 47.28      | 206 | 21.13    | 21.44    | 22.80    | 23.23    | 25.87    | 20.72    | 30.22    | 0.39x                        |
| tsv-internal      | 187.21     | 724 | 5.34     | 5.40     | 5.47     | 5.51     | 5.63     | 5.29     | 5.90     | 1.53x                        |
| tsv_wasm-internal | 127.01     | 626 | 7.85     | 7.94     | 8.03     | 8.07     | 8.19     | 7.74     | 8.80     | 1.04x                        |

**Files (intersection):** 50

**Throughput:** svelte/compiler 36.5 MB/s, tsv-json 15.4 MB/s, tsv_wasm-json 14.1 MB/s, tsv-internal 56.0 MB/s, tsv_wasm-internal 38.0 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 3.6x tsv-internal, tsv_wasm-json 2.7x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.88       | 10  | 530.09   | 535.39   | 554.99   | 555.36   | 555.65   | 514.03   | 555.73   | baseline              |
| tsv        | 103.67     | 466 | 9.61     | 9.80     | 10.03    | 10.11    | 10.33    | 9.48     | 13.96    | 55.1x                 |
| tsv_wasm   | 70.98      | 351 | 14.04    | 14.24    | 14.31    | 14.38    | 14.66    | 13.85    | 18.43    | 37.7x                 |
| oxfmt      | 10.95      | 55  | 91.76    | 94.96    | 96.87    | 97.72    | 98.62    | 82.81    | 99.26    | 5.82x                 |
| biome-wasm | 6.28       | 27  | 158.84   | 161.26   | 170.19   | 173.86   | 181.56   | 157.32   | 184.97   | 3.34x                 |

**Files (intersection):** 50

**Throughput:** prettier 0.6 MB/s, tsv 31.0 MB/s, tsv_wasm 21.2 MB/s, oxfmt 3.3 MB/s, biome-wasm 1.9 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 762.6 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.0 MB | 381.0 KB | 0.4x | 0.5x |
| tsv_wasm | 2.4 MB | 841.8 KB | — | — |
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
| format svelte (762f) | **57.1x** prettier, **53.9x** oxfmt |
| format typescript (2302f) | **25.3x** prettier, **1.73x** oxfmt |
| format css (50f) | **55.1x** prettier, **9.46x** oxfmt |
| parse svelte (762f) | **2.13x** svelte |
| parse typescript (2302f) | **1.54x** svelte, **0.65x** oxc-parser |
| parse css (50f) | **0.42x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (762f) | **37.5x** prettier, **7.47x** biome-wasm |
| format typescript (2302f) | **18.2x** prettier, **5.91x** biome-wasm |
| format css (50f) | **37.7x** prettier, **11.3x** biome-wasm |
| parse svelte (762f) | **1.91x** svelte |
| parse typescript (2302f) | **1.44x** svelte, **0.63x** oxc-parser-wasm |
| parse css (50f) | **0.39x** svelte |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no oxc counterpart — oxc exposes no comparably cheap mode (its JS API always serializes; experimentalLazy is setup-dominated). Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in `@fuzdev/tsv_parse_wasm` / `@fuzdev/tsv_wasm`) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._
