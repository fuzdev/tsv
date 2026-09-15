# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-09-15T01:20:12.180Z — tsv 0.3.0 (b2f9c39e)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@e8d37ffc1](https://github.com/fuzdev/corpora/tree/e8d37ffc18b8b8b743aaa93eca015871a7f6e98a) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_gitops/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2, typescript@6.0.3

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 1.57       | 8   | 635.46   | 645.10   | 652.97   | —        | —        | 617.18   | 662.83   | baseline                     |
| tsv-json                    | 3.70       | 17  | 269.73   | 271.73   | 273.74   | 275.50   | 277.12   | 268.10   | 277.52   | 2.36x                        |
| tsv_wasm-json               | 3.50       | 15  | 285.85   | 288.78   | 292.35   | 292.66   | 293.17   | 284.10   | 293.30   | 2.22x                        |
| tsv-json-no-locations       | 6.10       | 25  | 163.97   | 164.90   | 167.74   | 167.78   | 167.86   | 161.94   | 167.89   | 3.88x                        |
| tsv_wasm-json-no-locations  | 5.49       | 23  | 182.15   | 183.09   | 186.47   | 186.64   | 186.91   | 180.20   | 187.00   | 3.50x                        |
| tsv-internal                | 45.65      | 220 | 21.77    | 22.23    | 22.39    | 22.44    | 22.55    | 21.56    | 22.73    | 29.0x                        |
| tsv_wasm-internal           | 32.19      | 145 | 30.93    | 31.51    | 31.70    | 31.81    | 31.96    | 30.74    | 35.21    | 20.5x                        |
| rsvelte-parse               | 1.71       | 7   | 584.99   | 587.33   | 598.14   | —        | —        | 582.13   | 599.36   | 1.09x                        |
| rsvelte-parse-skip-expr-loc | 2.60       | 13  | 382.84   | 386.67   | 391.76   | 393.21   | 393.48   | 380.33   | 393.55   | 1.65x                        |

**Files (intersection):** 951

**Throughput:** svelte/compiler 3.8 MB/s, tsv-json 8.9 MB/s, tsv_wasm-json 8.4 MB/s, tsv-json-no-locations 14.7 MB/s, tsv_wasm-json-no-locations 13.3 MB/s, tsv-internal 110.1 MB/s, tsv_wasm-internal 77.6 MB/s, rsvelte-parse 4.1 MB/s, rsvelte-parse-skip-expr-loc 6.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 12.3x tsv-internal, tsv_wasm-json 9.2x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ---------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier   | 0.18       | 7  | 5.64    | 5.66    | 5.68    | —       | —       | 5.59    | 5.68    | baseline              |
| tsv        | 12.65      | 46 | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 71.3x                 |
| tsv_wasm   | 8.96       | 45 | 0.11    | 0.11    | 0.11    | 0.11    | 0.12    | 0.11    | 0.12    | 50.5x                 |
| oxfmt      | 0.18       | 7  | 5.65    | 5.68    | 5.69    | —       | —       | 5.61    | 5.70    | 1.00x                 |
| biome-wasm | 0.85       | 4  | 1.18    | 1.18    | 1.18    | —       | —       | 1.16    | 1.18    | 4.79x                 |

**Files (intersection):** 951

**Throughput:** prettier 0.4 MB/s, tsv 30.5 MB/s, tsv_wasm 21.6 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.0 MB/s

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.29       | 5  | 3.49    | 3.50    | 3.50    | —       | —       | 3.48    | 3.51    | baseline                      |
| tsv-json                   | 0.47       | 5  | 2.14    | 2.14    | 2.15    | —       | —       | 2.14    | 2.15    | 1.63x                         |
| tsv_wasm-json              | 0.46       | 4  | 2.17    | 2.18    | 2.18    | —       | —       | 2.16    | 2.19    | 1.61x                         |
| tsv-json-no-locations      | 0.97       | 5  | 1.03    | 1.03    | 1.03    | —       | —       | 1.03    | 1.03    | 3.39x                         |
| tsv_wasm-json-no-locations | 0.91       | 5  | 1.09    | 1.09    | 1.09    | —       | —       | 1.09    | 1.09    | 3.19x                         |
| tsv-internal               | 8.41       | 43 | 0.12    | 0.12    | 0.12    | 0.12    | 0.12    | 0.12    | 0.12    | 29.4x                         |
| tsv_wasm-internal          | 6.51       | 32 | 0.15    | 0.15    | 0.16    | 0.16    | 0.16    | 0.15    | 0.16    | 22.7x                         |
| oxc-parser                 | 0.72       | 5  | 1.39    | 1.39    | 1.40    | —       | —       | 1.39    | 1.40    | 2.50x                         |
| oxc-parser-wasm            | 0.69       | 5  | 1.44    | 1.45    | 1.45    | —       | —       | 1.44    | 1.45    | 2.42x                         |
| yuku-parser                | 2.34       | 10 | 0.43    | 0.43    | 0.45    | 0.45    | 0.45    | 0.42    | 0.45    | 8.16x                         |
| yuku-parser-wasm           | 2.72       | 12 | 0.37    | 0.38    | 0.39    | 0.40    | 0.42    | 0.36    | 0.42    | 9.51x                         |
| swc                        | 0.55       | 5  | 1.80    | 1.81    | 1.81    | —       | —       | 1.80    | 1.81    | 1.94x                         |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 5.3 MB/s, tsv-json 8.6 MB/s, tsv_wasm-json 8.5 MB/s, tsv-json-no-locations 17.9 MB/s, tsv_wasm-json-no-locations 16.9 MB/s, tsv-internal 155.1 MB/s, tsv_wasm-internal 120.1 MB/s, oxc-parser 13.2 MB/s, oxc-parser-wasm 12.8 MB/s, yuku-parser 43.1 MB/s, yuku-parser-wasm 50.2 MB/s, swc 10.2 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv_wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv_wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv_wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 18.0x tsv-internal, tsv_wasm-json 14.1x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.07       | 7  | 15.25   | 15.27   | 15.29   | —       | —       | 15.17   | 15.31   | baseline              |
| tsv         | 2.22       | 10 | 0.45    | 0.45    | 0.46    | 0.46    | 0.46    | 0.45    | 0.46    | 33.8x                 |
| tsv_wasm    | 1.61       | 8  | 0.62    | 0.62    | 0.63    | —       | —       | 0.62    | 0.63    | 24.6x                 |
| oxfmt       | 1.07       | 6  | 0.93    | 0.94    | 0.95    | —       | —       | 0.92    | 0.95    | 16.3x                 |
| biome-wasm  | 0.08       | 5  | 13.43   | 13.49   | 13.58   | —       | —       | 11.52   | 13.61   | 1.16x                 |
| dprint-wasm | 0.30       | 5  | 3.30    | 3.30    | 3.30    | —       | —       | 3.29    | 3.31    | 4.63x                 |

**Files (intersection):** 2643

**Throughput:** prettier 1.2 MB/s, tsv 40.9 MB/s, tsv_wasm 29.8 MB/s, oxfmt 19.8 MB/s, biome-wasm 1.4 MB/s, dprint-wasm 5.6 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv_wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2645/2645 (100%), dprint-wasm 2645/2645 (100%)

## parse/css

| Task Name         | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 85.77      | 409 | 11.61    | 11.96    | 12.39    | 12.85    | 16.25    | 11.10    | 23.54    | baseline                     |
| tsv-json          | 43.83      | 194 | 22.76    | 23.24    | 24.83    | 25.30    | 25.87    | 21.98    | 26.00    | 0.51x                        |
| tsv_wasm-json     | 40.67      | 181 | 24.45    | 25.04    | 26.30    | 27.18    | 32.81    | 23.96    | 33.27    | 0.47x                        |
| tsv-internal      | 215.81     | 874 | 4.63     | 4.68     | 4.75     | 4.78     | 4.83     | 4.60     | 5.05     | 2.52x                        |
| tsv_wasm-internal | 146.69     | 604 | 6.81     | 6.89     | 6.97     | 7.00     | 7.08     | 6.74     | 8.76     | 1.71x                        |
| postcss           | 79.53      | 377 | 12.51    | 12.81    | 13.41    | 13.83    | 14.40    | 12.17    | 14.53    | 0.93x                        |

**Files (intersection):** 55

**Throughput:** svelte/compiler 34.7 MB/s, tsv-json 17.8 MB/s, tsv_wasm-json 16.5 MB/s, tsv-internal 87.4 MB/s, tsv_wasm-internal 59.4 MB/s, postcss 32.2 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.9x tsv-internal, tsv_wasm-json 3.6x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.50       | 8   | 663.54   | 669.69   | 682.21   | —        | —        | 646.62   | 691.61   | baseline              |
| tsv        | 121.55     | 495 | 8.22     | 8.34     | 8.58     | 8.71     | 9.06     | 8.12     | 10.78    | 80.8x                 |
| tsv_wasm   | 85.29      | 400 | 11.67    | 11.88    | 12.02    | 12.07    | 12.37    | 11.54    | 14.28    | 56.7x                 |
| oxfmt      | 48.49      | 242 | 20.52    | 21.06    | 21.62    | 22.04    | 22.56    | 19.20    | 23.24    | 32.2x                 |
| biome-wasm | 6.36       | 24  | 157.06   | 173.64   | 177.41   | 189.11   | 192.67   | 140.74   | 194.04   | 4.23x                 |
| malva-wasm | 18.98      | 90  | 52.45    | 53.24    | 53.88    | 54.40    | 54.68    | 52.01    | 55.02    | 12.6x                 |

**Files (intersection):** 55

**Throughput:** prettier 0.6 MB/s, tsv 49.2 MB/s, tsv_wasm 34.5 MB/s, oxfmt 19.6 MB/s, biome-wasm 2.6 MB/s, malva-wasm 7.7 MB/s

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

_`vs tsv` divides native rows by `tsv (napi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm rows by `tsv_wasm`. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **71.3x** prettier, **71.6x** oxfmt |
| format typescript (2643f) | **33.8x** prettier, **2.07x** oxfmt |
| format css (55f) | **80.8x** prettier, **2.51x** oxfmt |
| parse svelte (951f) | **2.36x** svelte/compiler, **2.17x** rsvelte-parse |
| parse typescript (2642f) | **1.63x** acorn-typescript, **0.65x** oxc-parser, **0.20x** yuku-parser, **0.84x** swc |
| parse css (55f) | **0.51x** svelte/compiler, **0.55x** postcss |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **50.5x** prettier, **10.5x** biome-wasm |
| format typescript (2643f) | **24.6x** prettier, **21.2x** biome-wasm, **5.32x** dprint-wasm |
| format css (55f) | **56.7x** prettier, **13.4x** biome-wasm, **4.49x** malva-wasm |
| parse svelte (951f) | **2.22x** svelte/compiler |
| parse typescript (2642f) | **1.61x** acorn-typescript, **0.67x** oxc-parser-wasm, **0.17x** yuku-parser-wasm |
| parse css (55f) | **0.47x** svelte/compiler, **0.51x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds), so it carries the same payload disclosure oxc-parser does — the mechanism matches `tsv-json` (serialize, cross, materialize) while the tree it produces is neither tsv’s loc-bearing drop-in shape nor its span-only wire; measured on the perf corpus its JSON is 0.64× `tsv-json`’s bytes. rsvelte-parse returns a compact JSON string the caller parses — the identical mechanism `tsv-json` measures (same serialize + boundary + `JSON.parse` cost) and within ~1.5% of its payload measured across the corpus (0.13% smaller in aggregate at the current pin, per-component median exactly 1.00 — the axis a throughput ratio integrates), so it is the one third-party parse row matched to tsv on BOTH axes. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since no Rust CSS parser exposes an AST to JS. Not payload-matched either: it keeps selectors and values as strings where `parseCss` (and so tsv) builds full ASTs — 0.38× tsv’s node count and at most 0.56× its JSON bytes on the perf corpus. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in every parse-capable package) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

## Unstable Rows

1 timed row(s) were not stable: a cv past 10% (std_dev / mean — `cv` after outlier removal; `cv (raw)` before it, which counts only under 30 raw samples, where one deviant sweep is a real share of the row) or a drift past 5% (the median of the second half of the timings against the first's — a cost that moved WHILE the row was measured, which the cleaned cv cannot see: a second mode is deleted or blended, not reported). Every `Nx` involving one of these divides a mean that may be neither mode — read it as approximate, and re-run before drawing a conclusion from it; a longer window does not converge a drifting row, it moves the answer.

| Row | cv | cv (raw) | drift | samples (cleaned/raw) |
| --- | ---: | ---: | ---: | ---: |
| format/typescript/biome-wasm | 6.8% | 38.3% | +179.7% | 5/7 |

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
