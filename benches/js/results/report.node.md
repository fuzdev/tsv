# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-09-15T02:48:31.241Z — tsv 0.3.0 (9b06410e)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@e8d37ffc1](https://github.com/fuzdev/corpora/tree/e8d37ffc18b8b8b743aaa93eca015871a7f6e98a) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_gitops/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2, typescript@6.0.3

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 1.57       | 7   | 634.13   | 650.37   | 655.69   | —        | —        | 627.82   | 661.34   | baseline                     |
| tsv-json                    | 3.68       | 17  | 271.52   | 273.05   | 276.50   | 278.93   | 279.58   | 269.84   | 279.74   | 2.35x                        |
| tsv_wasm-json               | 3.47       | 15  | 288.44   | 289.86   | 294.72   | 295.32   | 296.41   | 286.88   | 296.68   | 2.21x                        |
| tsv-json-no-locations       | 6.06       | 27  | 164.63   | 166.42   | 168.62   | 170.05   | 170.58   | 163.12   | 170.79   | 3.86x                        |
| tsv_wasm-json-no-locations  | 5.46       | 24  | 183.40   | 183.84   | 187.82   | 188.14   | 188.85   | 181.46   | 189.05   | 3.48x                        |
| tsv-internal                | 45.65      | 183 | 21.91    | 22.18    | 22.54    | 22.64    | 22.76    | 21.69    | 23.65    | 29.1x                        |
| tsv_wasm-internal           | 32.49      | 123 | 30.78    | 30.98    | 31.71    | 31.78    | 31.97    | 30.60    | 32.17    | 20.7x                        |
| rsvelte-parse               | 1.70       | 7   | 590.04   | 593.61   | 605.23   | —        | —        | 588.47   | 605.35   | 1.08x                        |
| rsvelte-parse-skip-expr-loc | 2.59       | 13  | 385.36   | 386.90   | 392.79   | 394.20   | 394.63   | 381.31   | 394.74   | 1.65x                        |

**Files (intersection):** 951

**Throughput:** svelte/compiler 3.8 MB/s, tsv-json 8.9 MB/s, tsv_wasm-json 8.4 MB/s, tsv-json-no-locations 14.6 MB/s, tsv_wasm-json-no-locations 13.2 MB/s, tsv-internal 110.1 MB/s, tsv_wasm-internal 78.4 MB/s, rsvelte-parse 4.1 MB/s, rsvelte-parse-skip-expr-loc 6.2 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 12.4x tsv-internal, tsv_wasm-json 9.4x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ---------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier   | 0.18       | 5  | 5.66    | 5.71    | 5.71    | —       | —       | 5.61    | 5.71    | baseline              |
| tsv        | 12.62      | 50 | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 71.5x                 |
| tsv_wasm   | 8.96       | 43 | 0.11    | 0.11    | 0.11    | 0.12    | 0.12    | 0.11    | 0.12    | 50.7x                 |
| oxfmt      | 0.18       | 4  | 5.66    | 5.67    | 5.69    | —       | —       | 5.63    | 5.71    | 1.00x                 |
| biome-wasm | 0.86       | 5  | 1.16    | 1.16    | 1.16    | —       | —       | 1.15    | 1.17    | 4.90x                 |

**Files (intersection):** 951

**Throughput:** prettier 0.4 MB/s, tsv 30.4 MB/s, tsv_wasm 21.6 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.1 MB/s

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.29       | 5  | 3.47    | 3.48    | 3.48    | —       | —       | 3.45    | 3.48    | baseline                      |
| tsv-json                   | 0.46       | 5  | 2.16    | 2.16    | 2.16    | —       | —       | 2.15    | 2.16    | 1.61x                         |
| tsv_wasm-json              | 0.46       | 5  | 2.18    | 2.19    | 2.20    | —       | —       | 2.18    | 2.20    | 1.59x                         |
| tsv-json-no-locations      | 0.98       | 5  | 1.02    | 1.03    | 1.03    | —       | —       | 1.02    | 1.03    | 3.38x                         |
| tsv_wasm-json-no-locations | 0.91       | 5  | 1.09    | 1.09    | 1.09    | —       | —       | 1.09    | 1.09    | 3.17x                         |
| tsv-internal               | 8.39       | 38 | 0.12    | 0.12    | 0.12    | 0.12    | 0.12    | 0.12    | 0.12    | 29.1x                         |
| tsv_wasm-internal          | 6.54       | 28 | 0.15    | 0.15    | 0.16    | 0.16    | 0.16    | 0.15    | 0.16    | 22.7x                         |
| oxc-parser                 | 0.72       | 5  | 1.39    | 1.39    | 1.40    | —       | —       | 1.39    | 1.40    | 2.49x                         |
| oxc-parser-wasm            | 0.69       | 5  | 1.44    | 1.44    | 1.45    | —       | —       | 1.44    | 1.45    | 2.40x                         |
| yuku-parser                | 2.37       | 9  | 0.42    | 0.43    | 0.44    | —       | —       | 0.42    | 0.45    | 8.20x                         |
| yuku-parser-wasm           | 2.75       | 11 | 0.36    | 0.37    | 0.38    | 0.39    | 0.39    | 0.36    | 0.40    | 9.52x                         |
| swc                        | 0.55       | 5  | 1.81    | 1.81    | 1.82    | —       | —       | 1.81    | 1.82    | 1.91x                         |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 5.3 MB/s, tsv-json 8.6 MB/s, tsv_wasm-json 8.4 MB/s, tsv-json-no-locations 18.0 MB/s, tsv_wasm-json-no-locations 16.9 MB/s, tsv-internal 154.8 MB/s, tsv_wasm-internal 120.6 MB/s, oxc-parser 13.2 MB/s, oxc-parser-wasm 12.8 MB/s, yuku-parser 43.6 MB/s, yuku-parser-wasm 50.7 MB/s, swc 10.2 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv_wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv_wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv_wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 18.1x tsv-internal, tsv_wasm-json 14.3x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.07       | 5 | 14.75   | 14.76   | 14.80   | —       | —       | 14.63   | 14.82   | baseline              |
| tsv         | 2.21       | 9 | 0.45    | 0.45    | 0.46    | —       | —       | 0.45    | 0.46    | 32.6x                 |
| tsv_wasm    | 1.62       | 7 | 0.62    | 0.62    | 0.63    | —       | —       | 0.62    | 0.64    | 23.8x                 |
| oxfmt       | 1.08       | 6 | 0.93    | 0.93    | 0.93    | —       | —       | 0.92    | 0.93    | 15.9x                 |
| biome-wasm  | 0.21       | 5 | 4.88    | 4.89    | 4.90    | —       | —       | 4.84    | 4.90    | 3.02x                 |
| dprint-wasm | 0.30       | 5 | 3.30    | 3.30    | 3.30    | —       | —       | 3.30    | 3.30    | 4.46x                 |

**Files (intersection):** 2643

**Throughput:** prettier 1.3 MB/s, tsv 40.8 MB/s, tsv_wasm 29.9 MB/s, oxfmt 19.9 MB/s, biome-wasm 3.8 MB/s, dprint-wasm 5.6 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv_wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2645/2645 (100%), dprint-wasm 2645/2645 (100%)

## parse/css

| Task Name         | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 87.43      | 435 | 11.38    | 11.63    | 11.97    | 12.17    | 12.42    | 10.99    | 13.00    | baseline                     |
| tsv-json          | 43.77      | 192 | 22.83    | 23.22    | 25.04    | 25.63    | 25.92    | 22.14    | 26.34    | 0.50x                        |
| tsv_wasm-json     | 40.81      | 170 | 24.45    | 24.96    | 26.24    | 26.64    | 32.43    | 24.06    | 32.82    | 0.47x                        |
| tsv-internal      | 216.39     | 862 | 4.62     | 4.65     | 4.71     | 4.79     | 4.91     | 4.60     | 5.09     | 2.47x                        |
| tsv_wasm-internal | 147.99     | 601 | 6.76     | 6.78     | 6.88     | 6.94     | 6.98     | 6.70     | 7.35     | 1.69x                        |
| postcss           | 81.01      | 393 | 12.29    | 12.55    | 12.88    | 13.23    | 13.73    | 11.99    | 14.17    | 0.93x                        |

**Files (intersection):** 55

**Throughput:** svelte/compiler 35.4 MB/s, tsv-json 17.7 MB/s, tsv_wasm-json 16.5 MB/s, tsv-internal 87.7 MB/s, tsv_wasm-internal 59.9 MB/s, postcss 32.8 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.9x tsv-internal, tsv_wasm-json 3.6x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.51       | 7   | 662.18   | 670.87   | 676.35   | —        | —        | 657.87   | 684.99   | baseline              |
| tsv        | 122.13     | 507 | 8.18     | 8.24     | 8.38     | 8.43     | 8.57     | 8.13     | 11.20    | 81.1x                 |
| tsv_wasm   | 86.54      | 401 | 11.54    | 11.62    | 11.81    | 11.91    | 12.62    | 11.43    | 14.47    | 57.4x                 |
| oxfmt      | 48.14      | 240 | 20.78    | 21.18    | 21.63    | 21.90    | 22.31    | 19.06    | 23.10    | 31.9x                 |
| biome-wasm | 8.67       | 39  | 115.07   | 116.82   | 117.64   | 118.76   | 119.23   | 112.61   | 119.32   | 5.76x                 |
| malva-wasm | 19.14      | 75  | 52.25    | 52.73    | 53.30    | 53.42    | 53.51    | 52.02    | 53.77    | 12.7x                 |

**Files (intersection):** 55

**Throughput:** prettier 0.6 MB/s, tsv 49.5 MB/s, tsv_wasm 35.1 MB/s, oxfmt 19.5 MB/s, biome-wasm 3.5 MB/s, malva-wasm 7.8 MB/s

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
| format svelte (951f) | **71.5x** prettier, **71.3x** oxfmt |
| format typescript (2643f) | **32.6x** prettier, **2.05x** oxfmt |
| format css (55f) | **81.1x** prettier, **2.54x** oxfmt |
| parse svelte (951f) | **2.35x** svelte/compiler, **2.17x** rsvelte-parse |
| parse typescript (2642f) | **1.61x** acorn-typescript, **0.65x** oxc-parser, **0.20x** yuku-parser, **0.84x** swc |
| parse css (55f) | **0.50x** svelte/compiler, **0.54x** postcss |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **50.7x** prettier, **10.4x** biome-wasm |
| format typescript (2643f) | **23.8x** prettier, **7.89x** biome-wasm, **5.34x** dprint-wasm |
| format css (55f) | **57.4x** prettier, **9.98x** biome-wasm, **4.52x** malva-wasm |
| parse svelte (951f) | **2.21x** svelte/compiler |
| parse typescript (2642f) | **1.59x** acorn-typescript, **0.66x** oxc-parser-wasm, **0.17x** yuku-parser-wasm |
| parse css (55f) | **0.47x** svelte/compiler, **0.50x** postcss |

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
