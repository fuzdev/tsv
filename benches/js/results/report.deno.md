# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.9.6

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-09-15T02:37:12.152Z — tsv 0.3.0 (9b06410e)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@e8d37ffc1](https://github.com/fuzdev/corpora/tree/e8d37ffc18b8b8b743aaa93eca015871a7f6e98a) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_gitops/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2, typescript@6.0.3

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 1.62       | 9   | 615.76   | 628.21   | 635.44   | —        | —        | 595.94   | 652.68   | baseline                     |
| tsv-json                    | 4.14       | 18  | 241.76   | 242.40   | 247.76   | 248.25   | 248.27   | 240.39   | 248.28   | 2.55x                        |
| tsv_wasm-json               | 3.52       | 15  | 283.71   | 285.92   | 290.12   | 290.65   | 290.67   | 282.36   | 290.67   | 2.17x                        |
| tsv-json-no-locations       | 6.69       | 30  | 149.11   | 151.08   | 152.85   | 153.52   | 154.13   | 147.92   | 154.27   | 4.13x                        |
| tsv_wasm-json-no-locations  | 5.38       | 23  | 185.97   | 186.82   | 190.01   | 190.30   | 190.39   | 184.75   | 190.42   | 3.32x                        |
| tsv-internal                | 47.29      | 190 | 21.14    | 21.36    | 21.60    | 21.68    | 22.14    | 20.96    | 25.08    | 29.2x                        |
| tsv_wasm-internal           | 28.54      | 111 | 35.04    | 35.34    | 35.74    | 35.86    | 35.96    | 34.87    | 36.28    | 17.6x                        |
| rsvelte-parse               | 1.80       | 7   | 557.19   | 560.26   | 568.69   | —        | —        | 554.70   | 575.65   | 1.11x                        |
| rsvelte-parse-skip-expr-loc | 2.73       | 13  | 366.51   | 369.52   | 372.64   | 375.67   | 378.60   | 362.86   | 379.33   | 1.68x                        |

**Files (intersection):** 951

**Throughput:** svelte/compiler 3.9 MB/s, tsv-json 10.0 MB/s, tsv_wasm-json 8.5 MB/s, tsv-json-no-locations 16.1 MB/s, tsv_wasm-json-no-locations 13.0 MB/s, tsv-internal 114.1 MB/s, tsv_wasm-internal 68.8 MB/s, rsvelte-parse 4.3 MB/s, rsvelte-parse-skip-expr-loc 6.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 11.4x tsv-internal, tsv_wasm-json 8.1x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.19       | 5  | 5298.60  | 5305.13  | 5353.91  | —        | —        | 5232.78  | 5386.44  | baseline              |
| tsv        | 12.62      | 53 | 79.10    | 80.18    | 81.50    | 81.63    | 81.86    | 78.54    | 81.91    | 66.9x                 |
| tsv_wasm   | 7.83       | 32 | 127.62   | 129.42   | 131.08   | 131.63   | 132.14   | 126.84   | 132.43   | 41.5x                 |
| oxfmt      | 0.18       | 5  | 5399.27  | 5437.58  | 5461.54  | —        | —        | 5377.77  | 5477.51  | 0.98x                 |
| biome-wasm | 1.09       | 5  | 912.23   | 921.58   | 932.20   | —        | —        | 909.05   | 940.00   | 5.80x                 |

**Files (intersection):** 951

**Throughput:** prettier 0.5 MB/s, tsv 30.4 MB/s, tsv_wasm 18.9 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.6 MB/s

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.31       | 5  | 3.24    | 3.26    | 3.26    | —       | —       | 3.20    | 3.27    | baseline                      |
| tsv-json                   | 0.53       | 4  | 1.87    | 1.87    | 1.88    | —       | —       | 1.87    | 1.89    | 1.73x                         |
| tsv_wasm-json              | 0.47       | 5  | 2.11    | 2.11    | 2.12    | —       | —       | 2.10    | 2.12    | 1.53x                         |
| tsv-json-no-locations      | 1.11       | 5  | 0.90    | 0.91    | 0.91    | —       | —       | 0.90    | 0.91    | 3.58x                         |
| tsv_wasm-json-no-locations | 0.93       | 5  | 1.08    | 1.08    | 1.08    | —       | —       | 1.08    | 1.08    | 2.99x                         |
| tsv-internal               | 9.14       | 45 | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 29.6x                         |
| tsv_wasm-internal          | 5.70       | 25 | 0.18    | 0.18    | 0.18    | 0.18    | 0.18    | 0.17    | 0.18    | 18.4x                         |
| oxc-parser                 | 0.80       | 5  | 1.26    | 1.26    | 1.27    | —       | —       | 1.24    | 1.27    | 2.58x                         |
| oxc-parser-wasm            | 0.72       | 5  | 1.39    | 1.39    | 1.40    | —       | —       | 1.38    | 1.40    | 2.33x                         |
| yuku-parser                | 2.13       | 11 | 0.47    | 0.48    | 0.49    | 0.49    | 0.49    | 0.45    | 0.49    | 6.88x                         |
| yuku-parser-wasm           | 2.34       | 12 | 0.43    | 0.43    | 0.44    | 0.45    | 0.45    | 0.41    | 0.45    | 7.56x                         |
| swc                        | 0.58       | 5  | 1.71    | 1.71    | 1.72    | —       | —       | 1.70    | 1.72    | 1.89x                         |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 5.7 MB/s, tsv-json 9.8 MB/s, tsv_wasm-json 8.7 MB/s, tsv-json-no-locations 20.4 MB/s, tsv_wasm-json-no-locations 17.1 MB/s, tsv-internal 168.5 MB/s, tsv_wasm-internal 105.1 MB/s, oxc-parser 14.7 MB/s, oxc-parser-wasm 13.3 MB/s, yuku-parser 39.2 MB/s, yuku-parser-wasm 43.1 MB/s, swc 10.8 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv_wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv_wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv_wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 17.1x tsv-internal, tsv_wasm-json 12.0x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.08       | 5 | 12.96   | 12.97   | 12.98   | —       | —       | 12.86   | 12.99   | baseline              |
| tsv         | 2.29       | 9 | 0.44    | 0.44    | 0.45    | —       | —       | 0.44    | 0.46    | 29.6x                 |
| tsv_wasm    | 1.40       | 6 | 0.71    | 0.71    | 0.72    | —       | —       | 0.71    | 0.73    | 18.1x                 |
| oxfmt       | 1.07       | 6 | 0.93    | 0.93    | 0.94    | —       | —       | 0.92    | 0.94    | 13.9x                 |
| biome-wasm  | 0.22       | 5 | 4.52    | 4.53    | 4.54    | —       | —       | 4.52    | 4.55    | 2.86x                 |
| dprint-wasm | 0.27       | 5 | 3.71    | 3.72    | 3.72    | —       | —       | 3.71    | 3.72    | 3.49x                 |

**Files (intersection):** 2643

**Throughput:** prettier 1.4 MB/s, tsv 42.2 MB/s, tsv_wasm 25.9 MB/s, oxfmt 19.8 MB/s, biome-wasm 4.1 MB/s, dprint-wasm 5.0 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv_wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2645/2645 (100%), dprint-wasm 2645/2645 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 90.31      | 451  | 11.02    | 11.28    | 11.47    | 11.62    | 11.85    | 10.60    | 12.22    | baseline                     |
| tsv-json          | 48.50      | 216  | 20.53    | 20.95    | 21.25    | 21.90    | 22.70    | 20.31    | 23.07    | 0.54x                        |
| tsv_wasm-json     | 38.99      | 180  | 25.55    | 26.00    | 26.40    | 26.78    | 27.55    | 25.28    | 27.80    | 0.43x                        |
| tsv-internal      | 228.02     | 1081 | 4.38     | 4.40     | 4.45     | 4.50     | 4.58     | 4.33     | 5.97     | 2.52x                        |
| tsv_wasm-internal | 128.66     | 594  | 7.77     | 7.81     | 7.89     | 7.95     | 8.01     | 7.71     | 8.12     | 1.42x                        |
| postcss           | 83.84      | 416  | 11.91    | 12.17    | 12.35    | 12.53    | 12.75    | 11.46    | 13.58    | 0.93x                        |

**Files (intersection):** 55

**Throughput:** svelte/compiler 36.6 MB/s, tsv-json 19.6 MB/s, tsv_wasm-json 15.8 MB/s, tsv-internal 92.4 MB/s, tsv_wasm-internal 52.1 MB/s, postcss 34.0 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.7x tsv-internal, tsv_wasm-json 3.3x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.63       | 9   | 610.57   | 621.09   | 635.88   | —        | —        | 599.10   | 642.01   | baseline              |
| tsv        | 130.39     | 614 | 7.66     | 7.71     | 7.82     | 7.91     | 7.99     | 7.56     | 8.42     | 80.2x                 |
| tsv_wasm   | 73.59      | 297 | 13.57    | 13.72    | 13.85    | 13.90    | 14.07    | 13.51    | 15.00    | 45.2x                 |
| oxfmt      | 49.05      | 242 | 20.41    | 20.78    | 21.16    | 21.57    | 22.41    | 18.95    | 25.54    | 30.2x                 |
| biome-wasm | 9.62       | 36  | 103.75   | 106.09   | 112.92   | 116.24   | 117.34   | 101.53   | 117.81   | 5.92x                 |
| malva-wasm | 17.47      | 79  | 57.11    | 57.71    | 58.27    | 58.42    | 58.78    | 56.81    | 59.20    | 10.7x                 |

**Files (intersection):** 55

**Throughput:** prettier 0.7 MB/s, tsv 52.8 MB/s, tsv_wasm 29.8 MB/s, oxfmt 19.9 MB/s, biome-wasm 3.9 MB/s, malva-wasm 7.1 MB/s

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
| tsv (ffi) | 3.5 MB | 1.6 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.6 MB | 3.2x | 2.8x |
| tsv format (ffi) | 3.2 MB | 1.5 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 684.0 KB | 0.4x | 0.4x |
| tsv (napi) | 3.9 MB | 1.8 MB | 1.1x | 1.1x |
| oxc-parser (napi) | 2.1 MB | 882.6 KB | 0.6x | 0.5x |
| oxfmt (napi) | 9.1 MB | 3.7 MB | 2.6x | 2.3x |
| yuku-parser (napi) | 819.2 KB | 338.1 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 8.9 MB | 3.5 MB | 2.5x | 2.2x |
| rsvelte compiler (napi) | 17.6 MB | 7.4 MB | 5.0x | 4.6x |
| swc (napi) | 32.7 MB | 12.2 MB | 9.3x | 7.6x |

_`vs tsv` divides native rows by `tsv (ffi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm rows by `tsv_wasm`. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **66.9x** prettier, **68.4x** oxfmt |
| format typescript (2643f) | **29.6x** prettier, **2.13x** oxfmt |
| format css (55f) | **80.2x** prettier, **2.66x** oxfmt |
| parse svelte (951f) | **2.55x** svelte/compiler, **2.30x** rsvelte-parse |
| parse typescript (2642f) | **1.73x** acorn-typescript, **0.67x** oxc-parser, **0.25x** yuku-parser, **0.91x** swc |
| parse css (55f) | **0.54x** svelte/compiler, **0.58x** postcss |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **41.5x** prettier, **7.15x** biome-wasm |
| format typescript (2643f) | **18.1x** prettier, **6.35x** biome-wasm, **5.21x** dprint-wasm |
| format css (55f) | **45.2x** prettier, **7.65x** biome-wasm, **4.21x** malva-wasm |
| parse svelte (951f) | **2.17x** svelte/compiler |
| parse typescript (2642f) | **1.53x** acorn-typescript, **0.66x** oxc-parser-wasm, **0.20x** yuku-parser-wasm |
| parse css (55f) | **0.43x** svelte/compiler, **0.46x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds), so it carries the same payload disclosure oxc-parser does — the mechanism matches `tsv-json` (serialize, cross, materialize) while the tree it produces is neither tsv’s loc-bearing drop-in shape nor its span-only wire; measured on the perf corpus its JSON is 0.64× `tsv-json`’s bytes. rsvelte-parse returns a compact JSON string the caller parses — the identical mechanism `tsv-json` measures (same serialize + boundary + `JSON.parse` cost) and within ~1.5% of its payload measured across the corpus (0.13% smaller in aggregate at the current pin, per-component median exactly 1.00 — the axis a throughput ratio integrates), so it is the one third-party parse row matched to tsv on BOTH axes. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since no Rust CSS parser exposes an AST to JS. Not payload-matched either: it keeps selectors and values as strings where `parseCss` (and so tsv) builds full ASTs — 0.38× tsv’s node count and at most 0.56× its JSON bytes on the perf corpus. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in every parse-capable package) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

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
