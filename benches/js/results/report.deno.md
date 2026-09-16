# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.9.6

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-09-15T17:17:31.122Z — tsv 0.3.0 (29a107e9)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@e8d37ffc1](https://github.com/fuzdev/corpora/tree/e8d37ffc18b8b8b743aaa93eca015871a7f6e98a) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_gitops/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 1.67       | 9   | 594.52   | 603.72   | 624.28   | —        | —        | 584.12   | 626.74   | baseline                     |
| tsv-json                    | 4.12       | 20  | 242.49   | 242.94   | 243.65   | 243.96   | 249.64   | 241.76   | 251.06   | 2.47x                        |
| tsv-wasm-json               | 3.53       | 18  | 283.55   | 283.90   | 284.35   | 284.70   | 284.93   | 282.60   | 284.98   | 2.12x                        |
| tsv-json-no-locations       | 6.71       | 34  | 148.92   | 149.67   | 149.85   | 149.94   | 150.02   | 148.47   | 150.03   | 4.02x                        |
| tsv-wasm-json-no-locations  | 5.41       | 27  | 184.79   | 185.24   | 185.48   | 185.51   | 188.28   | 184.05   | 189.30   | 3.25x                        |
| tsv-internal                | 47.41      | 210 | 21.09    | 21.14    | 21.28    | 21.41    | 21.69    | 20.96    | 21.91    | 28.4x                        |
| tsv-wasm-internal           | 28.70      | 129 | 34.84    | 34.88    | 35.00    | 35.30    | 35.49    | 34.74    | 35.53    | 17.2x                        |
| rsvelte-parse               | 1.79       | 8   | 557.50   | 558.51   | 558.82   | —        | —        | 556.82   | 560.04   | 1.08x                        |
| rsvelte-parse-skip-expr-loc | 2.74       | 14  | 365.16   | 365.77   | 366.98   | 367.59   | 367.86   | 362.44   | 367.93   | 1.64x                        |

**Files (intersection):** 951

**Throughput:** svelte/compiler 4.0 MB/s, tsv-json 9.9 MB/s, tsv-wasm-json 8.5 MB/s, tsv-json-no-locations 16.2 MB/s, tsv-wasm-json-no-locations 13.1 MB/s, tsv-internal 114.3 MB/s, tsv-wasm-internal 69.2 MB/s, rsvelte-parse 4.3 MB/s, rsvelte-parse-skip-expr-loc 6.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 11.5x tsv-internal, tsv-wasm-json 8.1x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.19       | 5  | 5280.23  | 5401.84  | 5407.97  | —        | —        | 5273.59  | 5416.40  | baseline              |
| tsv        | 12.65      | 63 | 78.91    | 79.24    | 79.64    | 79.75    | 80.05    | 78.52    | 80.47    | 66.8x                 |
| tsv-wasm   | 7.92       | 36 | 126.19   | 126.84   | 127.56   | 128.95   | 130.96   | 125.83   | 132.21   | 41.8x                 |
| oxfmt      | 0.19       | 8  | 5373.66  | 5396.82  | 5400.98  | —        | —        | 5315.80  | 5409.37  | 0.98x                 |
| biome-wasm | 1.08       | 7  | 924.46   | 935.84   | 956.97   | —        | —        | 914.63   | 966.41   | 5.70x                 |

**Files (intersection):** 951

**Throughput:** prettier 0.5 MB/s, tsv 30.5 MB/s, tsv-wasm 19.1 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.6 MB/s

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.31       | 8  | 3.28    | 3.29    | 3.30    | —       | —       | 3.25    | 3.30    | baseline                      |
| tsv-json                   | 0.53       | 7  | 1.89    | 1.89    | 1.89    | —       | —       | 1.88    | 1.91    | 1.74x                         |
| tsv-wasm-json              | 0.47       | 7  | 2.11    | 2.11    | 2.12    | —       | —       | 2.11    | 2.13    | 1.55x                         |
| tsv-json-no-locations      | 1.11       | 7  | 0.90    | 0.90    | 0.91    | —       | —       | 0.90    | 0.91    | 3.62x                         |
| tsv-wasm-json-no-locations | 0.92       | 8  | 1.08    | 1.08    | 1.08    | —       | —       | 1.08    | 1.08    | 3.03x                         |
| tsv-internal               | 9.12       | 41 | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 29.9x                         |
| tsv-wasm-internal          | 5.73       | 29 | 0.17    | 0.17    | 0.18    | 0.18    | 0.18    | 0.17    | 0.18    | 18.8x                         |
| oxc-parser                 | 0.80       | 8  | 1.26    | 1.26    | 1.26    | —       | —       | 1.23    | 1.27    | 2.62x                         |
| oxc-parser-wasm            | 0.73       | 7  | 1.38    | 1.38    | 1.39    | —       | —       | 1.37    | 1.39    | 2.38x                         |
| yuku-parser                | 2.10       | 7  | 0.48    | 0.48    | 0.52    | —       | —       | 0.47    | 0.58    | 6.87x                         |
| yuku-parser-wasm           | 2.35       | 11 | 0.43    | 0.44    | 0.44    | 0.47    | 0.51    | 0.41    | 0.52    | 7.69x                         |
| swc                        | 0.57       | 7  | 1.74    | 1.74    | 1.75    | —       | —       | 1.73    | 1.76    | 1.88x                         |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 5.6 MB/s, tsv-json 9.8 MB/s, tsv-wasm-json 8.7 MB/s, tsv-json-no-locations 20.4 MB/s, tsv-wasm-json-no-locations 17.1 MB/s, tsv-internal 168.2 MB/s, tsv-wasm-internal 105.6 MB/s, oxc-parser 14.7 MB/s, oxc-parser-wasm 13.4 MB/s, yuku-parser 38.7 MB/s, yuku-parser-wasm 43.3 MB/s, swc 10.6 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv-wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv-wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv-wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 17.2x tsv-internal, tsv-wasm-json 12.1x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.08       | 8  | 13.01   | 13.05   | 13.09   | —       | —       | 12.89   | 13.11   | baseline              |
| tsv         | 2.29       | 12 | 0.44    | 0.44    | 0.44    | 0.44    | 0.44    | 0.43    | 0.44    | 29.8x                 |
| tsv-wasm    | 1.40       | 7  | 0.71    | 0.71    | 0.71    | —       | —       | 0.71    | 0.72    | 18.3x                 |
| oxfmt       | 1.10       | 7  | 0.91    | 0.92    | 0.92    | —       | —       | 0.91    | 0.92    | 14.3x                 |
| biome-wasm  | 0.22       | 6  | 4.61    | 4.62    | 4.62    | —       | —       | 4.61    | 4.62    | 2.82x                 |
| dprint-wasm | 0.27       | 8  | 3.72    | 3.72    | 3.73    | —       | —       | 3.72    | 3.73    | 3.50x                 |

**Files (intersection):** 2643

**Throughput:** prettier 1.4 MB/s, tsv 42.3 MB/s, tsv-wasm 25.9 MB/s, oxfmt 20.2 MB/s, biome-wasm 4.0 MB/s, dprint-wasm 5.0 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv-wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2645/2645 (100%), dprint-wasm 2645/2645 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 91.88      | 460  | 10.85    | 11.08    | 11.21    | 11.27    | 11.44    | 10.54    | 11.64    | baseline                     |
| tsv-json          | 49.20      | 232  | 20.34    | 20.48    | 21.01    | 21.95    | 22.81    | 19.87    | 32.36    | 0.54x                        |
| tsv-wasm-json     | 39.75      | 176  | 25.10    | 25.39    | 26.00    | 27.23    | 27.61    | 24.85    | 38.00    | 0.43x                        |
| tsv-internal      | 229.10     | 1060 | 4.36     | 4.38     | 4.40     | 4.42     | 4.53     | 4.34     | 8.37     | 2.49x                        |
| tsv-wasm-internal | 128.84     | 580  | 7.76     | 7.78     | 7.81     | 7.84     | 8.01     | 7.74     | 8.29     | 1.40x                        |
| postcss           | 83.12      | 410  | 12.02    | 12.20    | 12.51    | 12.75    | 13.06    | 11.56    | 13.97    | 0.90x                        |

**Files (intersection):** 55

**Throughput:** svelte/compiler 37.2 MB/s, tsv-json 19.9 MB/s, tsv-wasm-json 16.1 MB/s, tsv-internal 92.8 MB/s, tsv-wasm-internal 52.2 MB/s, postcss 33.7 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.7x tsv-internal, tsv-wasm-json 3.2x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.64       | 8   | 610.31   | 610.88   | 618.08   | —        | —        | 605.63   | 639.59   | baseline              |
| tsv        | 131.68     | 605 | 7.59     | 7.62     | 7.70     | 7.76     | 7.99     | 7.51     | 8.40     | 80.2x                 |
| tsv-wasm   | 74.24      | 341 | 13.47    | 13.49    | 13.53    | 13.65    | 13.82    | 13.42    | 13.88    | 45.2x                 |
| oxfmt      | 47.85      | 240 | 20.87    | 21.14    | 21.47    | 21.69    | 22.14    | 19.70    | 22.34    | 29.1x                 |
| biome-wasm | 9.83       | 45  | 101.44   | 102.75   | 103.26   | 103.93   | 120.06   | 100.46   | 121.66   | 5.98x                 |
| malva-wasm | 17.53      | 73  | 57.05    | 57.14    | 57.55    | 57.66    | 58.07    | 56.88    | 58.30    | 10.7x                 |

**Files (intersection):** 55

**Throughput:** prettier 0.7 MB/s, tsv 53.3 MB/s, tsv-wasm 30.1 MB/s, oxfmt 19.4 MB/s, biome-wasm 4.0 MB/s, malva-wasm 7.1 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv-format-wasm | 2.5 MB | 928.4 KB | 0.9x | 0.9x |
| tsv-parse-wasm | 995.6 KB | 384.6 KB | 0.4x | 0.4x |
| tsv-wasm | 2.8 MB | 1.0 MB | — | — |
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

_`vs tsv` divides native rows by `tsv (ffi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm rows by `tsv-wasm`. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **66.8x** prettier, **67.9x** oxfmt |
| format typescript (2643f) | **29.8x** prettier, **2.09x** oxfmt |
| format css (55f) | **80.2x** prettier, **2.75x** oxfmt |
| parse svelte (951f) | **2.47x** svelte/compiler, **2.30x** rsvelte-parse |
| parse typescript (2642f) | **1.74x** acorn-typescript, **0.66x** oxc-parser, **0.25x** yuku-parser, **0.92x** swc |
| parse css (55f) | **0.54x** svelte/compiler, **0.59x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **41.8x** prettier, **7.33x** biome-wasm |
| format typescript (2643f) | **18.3x** prettier, **6.48x** biome-wasm, **5.23x** dprint-wasm |
| format css (55f) | **45.2x** prettier, **7.55x** biome-wasm, **4.24x** malva-wasm |
| parse svelte (951f) | **2.12x** svelte/compiler |
| parse typescript (2642f) | **1.55x** acorn-typescript, **0.65x** oxc-parser-wasm, **0.20x** yuku-parser-wasm |
| parse css (55f) | **0.43x** svelte/compiler, **0.48x** postcss |

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
