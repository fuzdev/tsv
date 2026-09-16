# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-09-15T17:33:04.436Z — tsv 0.3.0 (29a107e9)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@e8d37ffc1](https://github.com/fuzdev/corpora/tree/e8d37ffc18b8b8b743aaa93eca015871a7f6e98a) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_gitops/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 1.62       | 7   | 617.74   | 619.45   | 632.93   | —        | —        | 612.35   | 641.08   | baseline                     |
| tsv-json                    | 3.68       | 19  | 271.51   | 272.27   | 273.52   | 273.60   | 273.65   | 269.69   | 273.67   | 2.27x                        |
| tsv-wasm-json               | 3.44       | 18  | 290.16   | 291.39   | 291.97   | 292.05   | 292.08   | 288.67   | 292.09   | 2.12x                        |
| tsv-json-no-locations       | 6.08       | 30  | 164.42   | 165.18   | 165.44   | 165.61   | 167.94   | 163.00   | 168.89   | 3.75x                        |
| tsv-wasm-json-no-locations  | 5.44       | 26  | 183.64   | 184.37   | 184.64   | 185.00   | 185.38   | 182.33   | 185.46   | 3.35x                        |
| tsv-internal                | 45.55      | 211 | 21.95    | 22.00    | 22.09    | 22.33    | 22.65    | 21.81    | 22.80    | 28.1x                        |
| tsv-wasm-internal           | 32.42      | 145 | 30.84    | 30.90    | 31.10    | 31.31    | 31.55    | 30.73    | 31.67    | 20.0x                        |
| rsvelte-parse               | 1.69       | 9   | 592.49   | 592.90   | 594.49   | —        | —        | 590.93   | 595.99   | 1.04x                        |
| rsvelte-parse-skip-expr-loc | 2.59       | 12  | 386.23   | 386.83   | 388.15   | 389.33   | 390.37   | 383.18   | 390.63   | 1.60x                        |

**Files (intersection):** 951

**Throughput:** svelte/compiler 3.9 MB/s, tsv-json 8.9 MB/s, tsv-wasm-json 8.3 MB/s, tsv-json-no-locations 14.7 MB/s, tsv-wasm-json-no-locations 13.1 MB/s, tsv-internal 109.9 MB/s, tsv-wasm-internal 78.2 MB/s, rsvelte-parse 4.1 MB/s, rsvelte-parse-skip-expr-loc 6.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 12.4x tsv-internal, tsv-wasm-json 9.4x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ---------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier   | 0.18       | 8  | 5.68    | 5.69    | 5.71    | —       | —       | 5.62    | 5.75    | baseline              |
| tsv        | 12.68      | 60 | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 71.9x                 |
| tsv-wasm   | 9.13       | 46 | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 51.8x                 |
| oxfmt      | 0.18       | 8  | 5.63    | 5.64    | 5.66    | —       | —       | 5.57    | 5.68    | 1.01x                 |
| biome-wasm | 0.86       | 8  | 1.16    | 1.17    | 1.17    | —       | —       | 1.15    | 1.18    | 4.88x                 |

**Files (intersection):** 951

**Throughput:** prettier 0.4 MB/s, tsv 30.6 MB/s, tsv-wasm 22.0 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.1 MB/s

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.29       | 8  | 3.50    | 3.51    | 3.52    | —       | —       | 3.49    | 3.52    | baseline                      |
| tsv-json                   | 0.46       | 7  | 2.15    | 2.15    | 2.16    | —       | —       | 2.15    | 2.16    | 1.63x                         |
| tsv-wasm-json              | 0.45       | 8  | 2.20    | 2.21    | 2.21    | —       | —       | 2.20    | 2.22    | 1.59x                         |
| tsv-json-no-locations      | 0.97       | 8  | 1.03    | 1.03    | 1.03    | —       | —       | 1.03    | 1.03    | 3.41x                         |
| tsv-wasm-json-no-locations | 0.92       | 8  | 1.09    | 1.09    | 1.09    | —       | —       | 1.09    | 1.09    | 3.21x                         |
| tsv-internal               | 8.42       | 36 | 0.12    | 0.12    | 0.12    | 0.12    | 0.12    | 0.12    | 0.12    | 29.5x                         |
| tsv-wasm-internal          | 6.55       | 33 | 0.15    | 0.15    | 0.15    | 0.15    | 0.15    | 0.15    | 0.15    | 23.0x                         |
| oxc-parser                 | 0.71       | 5  | 1.40    | 1.40    | 1.41    | —       | —       | 1.40    | 1.42    | 2.50x                         |
| oxc-parser-wasm            | 0.69       | 7  | 1.44    | 1.44    | 1.45    | —       | —       | 1.44    | 1.46    | 2.43x                         |
| yuku-parser                | 2.34       | 10 | 0.43    | 0.43    | 0.45    | 0.45    | 0.45    | 0.43    | 0.45    | 8.19x                         |
| yuku-parser-wasm           | 2.70       | 13 | 0.37    | 0.37    | 0.38    | 0.39    | 0.39    | 0.36    | 0.39    | 9.46x                         |
| swc                        | 0.54       | 8  | 1.84    | 1.85    | 1.85    | —       | —       | 1.84    | 1.85    | 1.90x                         |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 5.3 MB/s, tsv-json 8.6 MB/s, tsv-wasm-json 8.4 MB/s, tsv-json-no-locations 18.0 MB/s, tsv-wasm-json-no-locations 16.9 MB/s, tsv-internal 155.3 MB/s, tsv-wasm-internal 120.8 MB/s, oxc-parser 13.2 MB/s, oxc-parser-wasm 12.8 MB/s, yuku-parser 43.1 MB/s, yuku-parser-wasm 49.8 MB/s, swc 10.0 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv-wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv-wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv-wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 18.1x tsv-internal, tsv-wasm-json 14.4x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.07       | 8  | 14.53   | 14.58   | 14.62   | —       | —       | 14.45   | 14.68   | baseline              |
| tsv         | 2.22       | 12 | 0.45    | 0.45    | 0.45    | 0.45    | 0.45    | 0.45    | 0.45    | 32.2x                 |
| tsv-wasm    | 1.64       | 9  | 0.61    | 0.61    | 0.61    | —       | —       | 0.61    | 0.61    | 23.9x                 |
| oxfmt       | 1.07       | 8  | 0.93    | 0.95    | 0.95    | —       | —       | 0.92    | 0.95    | 15.6x                 |
| biome-wasm  | 0.20       | 8  | 4.88    | 4.90    | 4.91    | —       | —       | 4.86    | 4.92    | 2.98x                 |
| dprint-wasm | 0.30       | 7  | 3.30    | 3.31    | 3.31    | —       | —       | 3.30    | 3.32    | 4.40x                 |

**Files (intersection):** 2643

**Throughput:** prettier 1.3 MB/s, tsv 40.9 MB/s, tsv-wasm 30.3 MB/s, oxfmt 19.7 MB/s, biome-wasm 3.8 MB/s, dprint-wasm 5.6 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv-wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2645/2645 (100%), dprint-wasm 2645/2645 (100%)

## parse/css

| Task Name         | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 88.13      | 413 | 11.27    | 11.57    | 11.91    | 12.14    | 12.54    | 11.02    | 12.85    | baseline                     |
| tsv-json          | 44.44      | 185 | 22.56    | 22.66    | 24.53    | 25.21    | 25.57    | 21.94    | 26.13    | 0.50x                        |
| tsv-wasm-json     | 41.41      | 169 | 24.14    | 24.37    | 26.09    | 26.45    | 32.53    | 23.88    | 32.60    | 0.47x                        |
| tsv-internal      | 215.26     | 956 | 4.65     | 4.65     | 4.67     | 4.70     | 4.80     | 4.63     | 4.92     | 2.44x                        |
| tsv-wasm-internal | 146.36     | 688 | 6.83     | 6.85     | 6.87     | 6.90     | 7.05     | 6.78     | 7.14     | 1.66x                        |
| postcss           | 81.90      | 317 | 12.17    | 12.52    | 12.91    | 13.22    | 13.75    | 12.07    | 14.19    | 0.93x                        |

**Files (intersection):** 55

**Throughput:** svelte/compiler 35.7 MB/s, tsv-json 18.0 MB/s, tsv-wasm-json 16.8 MB/s, tsv-internal 87.2 MB/s, tsv-wasm-internal 59.3 MB/s, postcss 33.2 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.8x tsv-internal, tsv-wasm-json 3.5x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.51       | 6   | 663.65   | 670.41   | 688.77   | —        | —        | 658.88   | 691.86   | baseline              |
| tsv        | 122.70     | 548 | 8.15     | 8.17     | 8.22     | 8.27     | 8.47     | 8.10     | 10.76    | 81.3x                 |
| tsv-wasm   | 85.68      | 412 | 11.66    | 11.72    | 11.77    | 11.87    | 12.10    | 11.56    | 14.27    | 56.7x                 |
| oxfmt      | 48.73      | 243 | 20.52    | 20.91    | 21.41    | 21.63    | 22.29    | 19.01    | 23.44    | 32.3x                 |
| biome-wasm | 10.06      | 35  | 99.57    | 107.64   | 115.55   | 116.16   | 117.31   | 97.28    | 117.76   | 6.66x                 |
| malva-wasm | 19.21      | 84  | 52.04    | 52.14    | 52.58    | 52.84    | 53.69    | 51.93    | 55.77    | 12.7x                 |

**Files (intersection):** 55

**Throughput:** prettier 0.6 MB/s, tsv 49.7 MB/s, tsv-wasm 34.7 MB/s, oxfmt 19.7 MB/s, biome-wasm 4.1 MB/s, malva-wasm 7.8 MB/s

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

_`vs tsv` divides native rows by `tsv (napi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm rows by `tsv-wasm`. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **71.9x** prettier, **71.4x** oxfmt |
| format typescript (2643f) | **32.2x** prettier, **2.07x** oxfmt |
| format css (55f) | **81.3x** prettier, **2.52x** oxfmt |
| parse svelte (951f) | **2.27x** svelte/compiler, **2.18x** rsvelte-parse |
| parse typescript (2642f) | **1.63x** acorn-typescript, **0.65x** oxc-parser, **0.20x** yuku-parser, **0.86x** swc |
| parse css (55f) | **0.50x** svelte/compiler, **0.54x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **51.8x** prettier, **10.6x** biome-wasm |
| format typescript (2643f) | **23.9x** prettier, **8.03x** biome-wasm, **5.43x** dprint-wasm |
| format css (55f) | **56.7x** prettier, **8.52x** biome-wasm, **4.46x** malva-wasm |
| parse svelte (951f) | **2.12x** svelte/compiler |
| parse typescript (2642f) | **1.59x** acorn-typescript, **0.65x** oxc-parser-wasm, **0.17x** yuku-parser-wasm |
| parse css (55f) | **0.47x** svelte/compiler, **0.51x** postcss |

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
