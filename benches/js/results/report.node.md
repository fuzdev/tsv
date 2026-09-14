# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-09-14T22:22:31.255Z — tsv 0.3.0 (7eb42464)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@e8d37ffc1](https://github.com/fuzdev/corpora/tree/e8d37ffc18b8b8b743aaa93eca015871a7f6e98a) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_gitops/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 1.56       | 6   | 640.52   | 655.66   | 661.45   | —        | —        | 637.56   | 670.41   | baseline                     |
| tsv-json                    | 3.66       | 18  | 272.89   | 274.01   | 274.86   | 276.79   | 279.48   | 270.08   | 280.16   | 2.35x                        |
| tsv_wasm-json               | 3.46       | 18  | 287.85   | 290.68   | 293.67   | 295.98   | 296.81   | 284.84   | 297.01   | 2.22x                        |
| tsv-json-no-locations       | 6.11       | 24  | 163.87   | 164.65   | 167.55   | 167.68   | 168.04   | 162.35   | 168.14   | 3.92x                        |
| tsv_wasm-json-no-locations  | 5.47       | 23  | 182.98   | 184.40   | 187.93   | 188.23   | 189.87   | 181.23   | 190.45   | 3.51x                        |
| tsv-internal                | 46.08      | 179 | 21.64    | 22.14    | 22.34    | 22.41    | 22.91    | 21.50    | 26.06    | 29.6x                        |
| tsv_wasm-internal           | 32.08      | 115 | 31.15    | 31.81    | 32.02    | 32.10    | 32.29    | 31.01    | 32.43    | 20.6x                        |
| rsvelte-parse               | 1.70       | 7   | 587.96   | 590.71   | 606.84   | —        | —        | 585.51   | 608.94   | 1.09x                        |
| rsvelte-parse-skip-expr-loc | 2.60       | 12  | 384.02   | 387.71   | 392.71   | 394.41   | 395.01   | 381.86   | 395.17   | 1.67x                        |

**Files (intersection):** 951

**Throughput:** svelte/compiler 3.8 MB/s, tsv-json 8.8 MB/s, tsv_wasm-json 8.4 MB/s, tsv-json-no-locations 14.7 MB/s, tsv_wasm-json-no-locations 13.2 MB/s, tsv-internal 111.1 MB/s, tsv_wasm-internal 77.4 MB/s, rsvelte-parse 4.1 MB/s, rsvelte-parse-skip-expr-loc 6.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 12.6x tsv-internal, tsv_wasm-json 9.3x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ---------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier   | 0.18       | 7  | 5.63    | 5.69    | 5.77    | —       | —       | 5.55    | 5.83    | baseline              |
| tsv        | 12.60      | 58 | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 71.3x                 |
| tsv_wasm   | 8.95       | 42 | 0.11    | 0.11    | 0.11    | 0.12    | 0.12    | 0.11    | 0.12    | 50.7x                 |
| oxfmt      | 0.18       | 6  | 5.60    | 5.62    | 5.68    | —       | —       | 5.58    | 5.77    | 1.01x                 |
| biome-wasm | 0.86       | 5  | 1.16    | 1.17    | 1.18    | —       | —       | 1.14    | 1.18    | 4.86x                 |

**Files (intersection):** 951

**Throughput:** prettier 0.4 MB/s, tsv 30.4 MB/s, tsv_wasm 21.6 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.1 MB/s

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.29       | 5  | 3.50    | 3.50    | 3.51    | —       | —       | 3.47    | 3.51    | baseline                      |
| tsv-json                   | 0.47       | 5  | 2.13    | 2.13    | 2.13    | —       | —       | 2.13    | 2.13    | 1.64x                         |
| tsv_wasm-json              | 0.46       | 5  | 2.17    | 2.18    | 2.19    | —       | —       | 2.17    | 2.19    | 1.60x                         |
| tsv-json-no-locations      | 0.98       | 5  | 1.02    | 1.02    | 1.02    | —       | —       | 1.02    | 1.02    | 3.43x                         |
| tsv_wasm-json-no-locations | 0.92       | 5  | 1.09    | 1.09    | 1.09    | —       | —       | 1.09    | 1.09    | 3.21x                         |
| tsv-internal               | 8.45       | 34 | 0.12    | 0.12    | 0.12    | 0.12    | 0.12    | 0.12    | 0.12    | 29.5x                         |
| tsv_wasm-internal          | 6.48       | 26 | 0.15    | 0.16    | 0.16    | 0.16    | 0.16    | 0.15    | 0.16    | 22.6x                         |
| oxc-parser                 | 0.72       | 5  | 1.39    | 1.39    | 1.40    | —       | —       | 1.37    | 1.40    | 2.52x                         |
| oxc-parser-wasm            | 0.70       | 5  | 1.43    | 1.43    | 1.43    | —       | —       | 1.43    | 1.43    | 2.45x                         |
| yuku-parser                | 2.36       | 10 | 0.42    | 0.43    | 0.44    | 0.44    | 0.45    | 0.42    | 0.45    | 8.25x                         |
| yuku-parser-wasm           | 2.73       | 11 | 0.37    | 0.37    | 0.38    | 0.39    | 0.40    | 0.36    | 0.40    | 9.54x                         |
| swc                        | 0.55       | 5  | 1.81    | 1.81    | 1.81    | —       | —       | 1.81    | 1.81    | 1.93x                         |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 5.3 MB/s, tsv-json 8.7 MB/s, tsv_wasm-json 8.5 MB/s, tsv-json-no-locations 18.1 MB/s, tsv_wasm-json-no-locations 16.9 MB/s, tsv-internal 155.8 MB/s, tsv_wasm-internal 119.5 MB/s, oxc-parser 13.3 MB/s, oxc-parser-wasm 12.9 MB/s, yuku-parser 43.5 MB/s, yuku-parser-wasm 50.3 MB/s, swc 10.2 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv_wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv_wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv_wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 18.0x tsv-internal, tsv_wasm-json 14.1x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.07       | 7 | 14.90   | 14.95   | 15.00   | —       | —       | 14.79   | 15.08   | baseline              |
| tsv         | 2.23       | 9 | 0.45    | 0.45    | 0.45    | —       | —       | 0.45    | 0.46    | 33.3x                 |
| tsv_wasm    | 1.62       | 7 | 0.62    | 0.62    | 0.63    | —       | —       | 0.62    | 0.63    | 24.2x                 |
| oxfmt       | 1.08       | 6 | 0.92    | 0.92    | 0.93    | —       | —       | 0.92    | 0.93    | 16.2x                 |
| biome-wasm  | 0.16       | 5 | 4.92    | 12.46   | 13.40   | —       | —       | 4.85    | 13.46   | 2.39x                 |
| dprint-wasm | 0.31       | 5 | 3.26    | 3.27    | 3.27    | —       | —       | 3.26    | 3.27    | 4.57x                 |

**Files (intersection):** 2643

**Throughput:** prettier 1.2 MB/s, tsv 41.2 MB/s, tsv_wasm 29.9 MB/s, oxfmt 20.0 MB/s, biome-wasm 3.0 MB/s, dprint-wasm 5.7 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv_wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2645/2645 (100%), dprint-wasm 2645/2645 (100%)

## parse/css

| Task Name         | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 86.46      | 411 | 11.52    | 11.83    | 12.19    | 12.57    | 18.16    | 11.11    | 23.17    | baseline                     |
| tsv-json          | 43.96      | 195 | 22.70    | 23.27    | 24.73    | 25.17    | 26.03    | 21.89    | 26.30    | 0.51x                        |
| tsv_wasm-json     | 40.61      | 186 | 24.46    | 25.16    | 26.19    | 27.08    | 31.99    | 24.01    | 33.42    | 0.47x                        |
| tsv-internal      | 214.75     | 888 | 4.65     | 4.70     | 4.77     | 4.85     | 5.09     | 4.60     | 6.13     | 2.48x                        |
| tsv_wasm-internal | 146.12     | 712 | 6.83     | 6.89     | 6.97     | 7.00     | 7.05     | 6.70     | 7.53     | 1.69x                        |
| postcss           | 80.47      | 382 | 12.35    | 12.66    | 13.25    | 13.63    | 14.10    | 12.04    | 14.49    | 0.93x                        |

**Files (intersection):** 55

**Throughput:** svelte/compiler 35.0 MB/s, tsv-json 17.8 MB/s, tsv_wasm-json 16.4 MB/s, tsv-internal 87.0 MB/s, tsv_wasm-internal 59.2 MB/s, postcss 32.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.9x tsv-internal, tsv_wasm-json 3.6x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.49       | 8   | 664.43   | 677.28   | 695.53   | —        | —        | 646.69   | 706.07   | baseline              |
| tsv        | 121.25     | 559 | 8.22     | 8.33     | 8.45     | 8.49     | 8.61     | 8.13     | 10.74    | 81.2x                 |
| tsv_wasm   | 86.05      | 416 | 11.56    | 11.75    | 11.86    | 11.90    | 12.02    | 11.44    | 14.24    | 57.7x                 |
| oxfmt      | 46.13      | 231 | 21.65    | 22.00    | 22.45    | 22.67    | 23.39    | 20.39    | 23.67    | 30.9x                 |
| biome-wasm | 6.32       | 25  | 158.51   | 174.08   | 191.45   | 193.46   | 196.48   | 140.06   | 197.68   | 4.23x                 |
| malva-wasm | 19.06      | 86  | 52.24    | 53.16    | 53.85    | 54.29    | 55.09    | 51.85    | 57.13    | 12.8x                 |

**Files (intersection):** 55

**Throughput:** prettier 0.6 MB/s, tsv 49.1 MB/s, tsv_wasm 34.9 MB/s, oxfmt 18.7 MB/s, biome-wasm 2.6 MB/s, malva-wasm 7.7 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.5 MB | 928.4 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 995.6 KB | 384.6 KB | 0.4x | 0.4x |
| tsv_wasm | 2.8 MB | 1.0 MB | — | — |
| biome (wasm) | 44.6 MB | 11.4 MB | 15.9x | 11.1x |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.5x | 1.1x |
| oxc-parser (wasm) | 1.5 MB | 481.4 KB | 0.5x | 0.5x |
| yuku-parser (wasm) | 743.3 KB | 222.8 KB | 0.3x | 0.2x |
| malva (wasm) | 1.5 MB | 414.0 KB | 0.5x | 0.4x |
| tsv (ffi) | 3.5 MB | 1.6 MB | 0.9x | 0.9x |
| tsv format (ffi) | 3.2 MB | 1.5 MB | 0.8x | 0.8x |
| tsv parse (ffi) | 1.6 MB | 684.0 KB | 0.4x | 0.4x |
| tsv (napi) | 3.9 MB | 1.8 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.6 MB | 2.9x | 2.6x |
| oxc-parser (napi) | 2.1 MB | 882.6 KB | 0.5x | 0.5x |
| oxfmt (napi) | 9.1 MB | 3.7 MB | 2.3x | 2.1x |
| yuku-parser (napi) | 819.2 KB | 338.1 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 8.9 MB | 3.5 MB | 2.3x | 2.0x |
| rsvelte compiler (napi) | 17.6 MB | 7.4 MB | 4.5x | 4.2x |
| swc (napi) | 32.7 MB | 12.2 MB | 8.4x | 6.9x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **71.3x** prettier, **70.6x** oxfmt |
| format typescript (2643f) | **33.3x** prettier, **2.06x** oxfmt |
| format css (55f) | **81.2x** prettier, **2.63x** oxfmt |
| parse svelte (951f) | **2.35x** svelte/compiler, **2.15x** rsvelte-parse |
| parse typescript (2642f) | **1.64x** acorn-typescript, **0.65x** oxc-parser, **0.20x** yuku-parser, **0.85x** swc |
| parse css (55f) | **0.51x** svelte/compiler, **0.55x** postcss |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **50.7x** prettier, **10.4x** biome-wasm |
| format typescript (2643f) | **24.2x** prettier, **10.1x** biome-wasm, **5.29x** dprint-wasm |
| format css (55f) | **57.7x** prettier, **13.6x** biome-wasm, **4.51x** malva-wasm |
| parse svelte (951f) | **2.22x** svelte/compiler |
| parse typescript (2642f) | **1.60x** acorn-typescript, **0.66x** oxc-parser-wasm, **0.17x** yuku-parser-wasm |
| parse css (55f) | **0.47x** svelte/compiler, **0.50x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds), so it carries the same payload disclosure oxc-parser does — the mechanism matches `tsv-json` (serialize, cross, materialize) while the tree it produces is neither tsv’s loc-bearing drop-in shape nor its span-only wire. rsvelte-parse returns a compact JSON string the caller parses — the identical mechanism `tsv-json` measures (same serialize + boundary + `JSON.parse` cost) and within ~1.5% of its payload measured across the corpus (the axis a throughput ratio integrates; per component the spread is wider), so it is the one third-party parse row matched to tsv on BOTH axes. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since no Rust CSS parser exposes an AST to JS. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in every parse-capable package) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

## Unstable Rows

1 timed row(s) varied more than 10% across iterations (cv = std_dev / mean, post-outlier-removal). Every `Nx` involving one of these divides an unstable mean — read it as approximate, and prefer re-running before drawing a conclusion from it.

| Row | cv | samples |
| --- | ---: | ---: |
| format/typescript/biome-wasm | 47.8% | 5 |

## Skipped Files

3 files skipped, 12 unique file+error combinations — Svelte 0, TypeScript 3, CSS 0 files.

**Per-benchmark skip counts:**
- parse/typescript: acorn-typescript: 3
- parse/typescript: swc: 3
- parse/typescript: oxc-parser: 2
- parse/typescript: oxc-parser-wasm: 2
- parse/typescript: yuku-parser: 2
- parse/typescript: yuku-parser-wasm: 2
- format/typescript: oxfmt: 2

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
