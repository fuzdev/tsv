# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-09-16T20:36:06.121Z — tsv 0.4.0 (0d7c0251)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@e8d37ffc1](https://github.com/fuzdev/corpora/tree/e8d37ffc18b8b8b743aaa93eca015871a7f6e98a) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_gitops/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 1.59       | 8   | 626.33   | 630.27   | 640.55   | —        | —        | 613.58   | 653.21   | baseline                     |
| tsv-json                    | 3.76       | 19  | 266.07   | 266.72   | 267.27   | 267.84   | 267.88   | 264.74   | 267.89   | 2.36x                        |
| tsv-wasm-json               | 3.51       | 18  | 284.56   | 285.15   | 286.38   | 286.98   | 286.98   | 283.26   | 286.99   | 2.21x                        |
| tsv-json-no-locations       | 6.16       | 26  | 162.33   | 162.67   | 163.08   | 163.28   | 163.59   | 161.35   | 163.68   | 3.87x                        |
| tsv-wasm-json-no-locations  | 5.51       | 28  | 181.29   | 182.12   | 182.36   | 182.61   | 182.77   | 179.99   | 182.81   | 3.46x                        |
| tsv-internal                | 47.15      | 213 | 21.21    | 21.26    | 21.45    | 21.63    | 21.94    | 21.07    | 22.09    | 29.6x                        |
| tsv-wasm-internal           | 32.62      | 149 | 30.66    | 30.70    | 30.84    | 31.13    | 31.36    | 30.53    | 34.95    | 20.5x                        |
| rsvelte-parse               | 1.71       | 8   | 586.00   | 586.80   | 587.51   | —        | —        | 583.27   | 589.81   | 1.07x                        |
| rsvelte-parse-skip-expr-loc | 2.61       | 14  | 382.92   | 383.48   | 384.95   | 385.19   | 385.40   | 381.78   | 385.46   | 1.64x                        |

**Files (intersection):** 951

**Throughput:** svelte/compiler 3.8 MB/s, tsv-json 9.1 MB/s, tsv-wasm-json 8.5 MB/s, tsv-json-no-locations 14.8 MB/s, tsv-wasm-json-no-locations 13.3 MB/s, tsv-internal 113.7 MB/s, tsv-wasm-internal 78.7 MB/s, rsvelte-parse 4.1 MB/s, rsvelte-parse-skip-expr-loc 6.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 12.6x tsv-internal, tsv-wasm-json 9.3x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ---------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier   | 0.18       | 8  | 5.54    | 5.56    | 5.60    | —       | —       | 5.48    | 5.62    | baseline              |
| tsv        | 12.73      | 56 | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 70.6x                 |
| tsv-wasm   | 9.08       | 45 | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 50.3x                 |
| oxfmt      | 0.18       | 8  | 5.58    | 5.63    | 5.66    | —       | —       | 5.54    | 5.69    | 0.99x                 |
| biome-wasm | 0.85       | 8  | 1.17    | 1.17    | 1.18    | —       | —       | 1.16    | 1.18    | 4.74x                 |

**Files (intersection):** 951

**Throughput:** prettier 0.4 MB/s, tsv 30.7 MB/s, tsv-wasm 21.9 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.1 MB/s

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.29       | 8  | 3.49    | 3.50    | 3.51    | —       | —       | 3.48    | 3.52    | baseline                      |
| tsv-json                   | 0.47       | 8  | 2.11    | 2.11    | 2.11    | —       | —       | 2.10    | 2.11    | 1.66x                         |
| tsv-wasm-json              | 0.46       | 8  | 2.16    | 2.16    | 2.17    | —       | —       | 2.15    | 2.17    | 1.62x                         |
| tsv-json-no-locations      | 0.98       | 8  | 1.02    | 1.02    | 1.02    | —       | —       | 1.02    | 1.02    | 3.42x                         |
| tsv-wasm-json-no-locations | 0.91       | 8  | 1.09    | 1.09    | 1.09    | —       | —       | 1.09    | 1.10    | 3.20x                         |
| tsv-internal               | 8.43       | 40 | 0.12    | 0.12    | 0.12    | 0.12    | 0.12    | 0.12    | 0.12    | 29.5x                         |
| tsv-wasm-internal          | 6.55       | 33 | 0.15    | 0.15    | 0.15    | 0.15    | 0.15    | 0.15    | 0.15    | 22.9x                         |
| oxc-parser                 | 0.72       | 8  | 1.39    | 1.39    | 1.40    | —       | —       | 1.39    | 1.40    | 2.51x                         |
| oxc-parser-wasm            | 0.70       | 8  | 1.44    | 1.44    | 1.44    | —       | —       | 1.43    | 1.45    | 2.43x                         |
| yuku-parser                | 2.36       | 10 | 0.42    | 0.43    | 0.44    | 0.44    | 0.44    | 0.42    | 0.44    | 8.25x                         |
| yuku-parser-wasm           | 2.73       | 11 | 0.37    | 0.37    | 0.38    | 0.39    | 0.39    | 0.37    | 0.39    | 9.53x                         |
| swc                        | 0.55       | 8  | 1.82    | 1.82    | 1.82    | —       | —       | 1.81    | 1.82    | 1.92x                         |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 5.3 MB/s, tsv-json 8.8 MB/s, tsv-wasm-json 8.5 MB/s, tsv-json-no-locations 18.0 MB/s, tsv-wasm-json-no-locations 16.9 MB/s, tsv-internal 155.5 MB/s, tsv-wasm-internal 120.7 MB/s, oxc-parser 13.3 MB/s, oxc-parser-wasm 12.8 MB/s, yuku-parser 43.5 MB/s, yuku-parser-wasm 50.3 MB/s, swc 10.1 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv-wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv-wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv-wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 17.8x tsv-internal, tsv-wasm-json 14.1x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.07       | 8  | 14.61   | 14.67   | 14.69   | —       | —       | 14.53   | 14.71   | baseline              |
| tsv         | 2.22       | 11 | 0.45    | 0.45    | 0.45    | 0.45    | 0.46    | 0.45    | 0.46    | 32.4x                 |
| tsv-wasm    | 1.63       | 9  | 0.62    | 0.62    | 0.62    | —       | —       | 0.61    | 0.62    | 23.8x                 |
| oxfmt       | 1.08       | 7  | 0.92    | 0.93    | 0.93    | —       | —       | 0.92    | 0.93    | 15.8x                 |
| biome-wasm  | 0.20       | 5  | 4.91    | 4.91    | 4.91    | —       | —       | 4.91    | 4.91    | 2.98x                 |
| dprint-wasm | 0.30       | 8  | 3.30    | 3.30    | 3.30    | —       | —       | 3.29    | 3.30    | 4.43x                 |

**Files (intersection):** 2643

**Throughput:** prettier 1.3 MB/s, tsv 41.0 MB/s, tsv-wasm 30.0 MB/s, oxfmt 20.0 MB/s, biome-wasm 3.8 MB/s, dprint-wasm 5.6 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv-wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2645/2645 (100%), dprint-wasm 2645/2645 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 88.40      | 396  | 11.22    | 11.59    | 11.84    | 11.99    | 12.34    | 11.00    | 15.57    | baseline                     |
| tsv-json          | 45.26      | 177  | 22.11    | 22.19    | 24.15    | 24.97    | 25.29    | 21.62    | 25.42    | 0.51x                        |
| tsv-wasm-json     | 43.09      | 181  | 23.19    | 23.47    | 25.04    | 25.53    | 31.87    | 22.92    | 32.22    | 0.49x                        |
| tsv-internal      | 256.25     | 1171 | 3.90     | 3.91     | 3.93     | 3.96     | 4.05     | 3.87     | 4.46     | 2.90x                        |
| tsv-wasm-internal | 168.50     | 758  | 5.94     | 5.94     | 5.97     | 6.00     | 6.14     | 5.90     | 6.31     | 1.91x                        |
| postcss           | 81.39      | 365  | 12.20    | 12.53    | 12.88    | 13.11    | 13.29    | 12.05    | 13.92    | 0.92x                        |

**Files (intersection):** 55

**Throughput:** svelte/compiler 35.8 MB/s, tsv-json 18.3 MB/s, tsv-wasm-json 17.5 MB/s, tsv-internal 103.8 MB/s, tsv-wasm-internal 68.3 MB/s, postcss 33.0 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 5.7x tsv-internal, tsv-wasm-json 3.9x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.56       | 7   | 641.85   | 643.59   | 651.61   | —        | —        | 638.08   | 669.59   | baseline              |
| tsv        | 132.98     | 620 | 7.52     | 7.54     | 7.59     | 7.62     | 7.88     | 7.45     | 10.45    | 85.3x                 |
| tsv-wasm   | 93.68      | 429 | 10.67    | 10.71    | 10.78    | 10.86    | 11.02    | 10.60    | 13.48    | 60.1x                 |
| oxfmt      | 47.80      | 238 | 20.92    | 21.34    | 21.74    | 22.00    | 22.43    | 19.43    | 23.55    | 30.7x                 |
| biome-wasm | 9.50       | 45  | 103.70   | 105.55   | 116.28   | 118.89   | 119.19   | 97.99    | 119.36   | 6.09x                 |
| malva-wasm | 19.14      | 84  | 52.25    | 52.34    | 52.75    | 52.86    | 53.34    | 52.12    | 55.85    | 12.3x                 |

**Files (intersection):** 55

**Throughput:** prettier 0.6 MB/s, tsv 53.9 MB/s, tsv-wasm 37.9 MB/s, oxfmt 19.4 MB/s, biome-wasm 3.8 MB/s, malva-wasm 7.8 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv-format-wasm | 2.5 MB | 930.5 KB | 0.9x | 0.9x |
| tsv-parse-wasm | 1.0 MB | 385.7 KB | 0.4x | 0.4x |
| tsv-wasm | 2.8 MB | 1.0 MB | — | — |
| biome (wasm) | 44.6 MB | 11.4 MB | 15.9x | 11.1x |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.5x | 1.1x |
| oxc-parser (wasm) | 1.5 MB | 481.4 KB | 0.5x | 0.5x |
| yuku-parser (wasm) | 743.3 KB | 222.8 KB | 0.3x | 0.2x |
| malva (wasm) | 1.5 MB | 414.0 KB | 0.5x | 0.4x |
| tsv (ffi) | 3.5 MB | 1.6 MB | 0.9x | 0.9x |
| tsv format (ffi) | 3.2 MB | 1.5 MB | 0.8x | 0.8x |
| tsv parse (ffi) | 1.6 MB | 686.1 KB | 0.4x | 0.4x |
| tsv (napi) | 3.9 MB | 1.8 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.6 MB | 2.9x | 2.6x |
| oxc-parser (napi) | 2.1 MB | 882.6 KB | 0.5x | 0.5x |
| oxfmt (napi) | 9.1 MB | 3.7 MB | 2.3x | 2.1x |
| yuku-parser (napi) | 819.2 KB | 338.1 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 8.9 MB | 3.5 MB | 2.3x | 2.0x |
| rsvelte compiler (napi) | 17.6 MB | 7.4 MB | 4.5x | 4.2x |
| swc (napi) | 32.7 MB | 12.2 MB | 8.4x | 6.9x |

_`vs tsv` divides native rows by `tsv (napi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm rows by `tsv-wasm`. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **70.6x** prettier, **71.3x** oxfmt |
| format typescript (2643f) | **32.4x** prettier, **2.05x** oxfmt |
| format css (55f) | **85.3x** prettier, **2.78x** oxfmt |
| parse svelte (951f) | **2.36x** svelte/compiler, **2.20x** rsvelte-parse |
| parse typescript (2642f) | **1.66x** acorn-typescript, **0.66x** oxc-parser, **0.20x** yuku-parser, **0.86x** swc |
| parse css (55f) | **0.51x** svelte/compiler, **0.56x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **50.3x** prettier, **10.6x** biome-wasm |
| format typescript (2643f) | **23.8x** prettier, **7.98x** biome-wasm, **5.36x** dprint-wasm |
| format css (55f) | **60.1x** prettier, **9.86x** biome-wasm, **4.90x** malva-wasm |
| parse svelte (951f) | **2.21x** svelte/compiler |
| parse typescript (2642f) | **1.62x** acorn-typescript, **0.67x** oxc-parser-wasm, **0.17x** yuku-parser-wasm |
| parse css (55f) | **0.49x** svelte/compiler, **0.53x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv-wasm-json, so these parse rows are apples-to-apples. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds), so it carries the same payload disclosure oxc-parser does — the mechanism matches `tsv-json` (serialize, cross, materialize) while the tree it produces is neither tsv’s loc-bearing drop-in shape nor its span-only wire; measured on the perf corpus its JSON is 0.64× `tsv-json`’s bytes. rsvelte-parse returns a compact JSON string the caller parses — the identical mechanism `tsv-json` measures (same serialize + boundary + `JSON.parse` cost) and within ~1.5% of its payload measured across the corpus (0.13% smaller in aggregate at the current pin, per-component median exactly 1.00 — the axis a throughput ratio integrates), so it is the one third-party parse row matched to tsv on BOTH axes. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since no Rust CSS parser exposes an AST to JS. Not payload-matched either: it keeps selectors and values as strings where `parseCss` (and so tsv) builds full ASTs — 0.38× tsv’s node count and at most 0.56× its JSON bytes on the perf corpus. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

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
