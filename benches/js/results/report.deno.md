# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.9.6

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-09-16T20:20:39.043Z — tsv 0.4.0 (0d7c0251)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@e8d37ffc1](https://github.com/fuzdev/corpora/tree/e8d37ffc18b8b8b743aaa93eca015871a7f6e98a) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_gitops/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 1.65       | 9   | 603.77   | 607.90   | 611.35   | —        | —        | 590.91   | 623.51   | baseline                     |
| tsv-json                    | 4.14       | 20  | 241.32   | 241.76   | 242.22   | 242.22   | 247.71   | 240.70   | 249.09   | 2.51x                        |
| tsv-wasm-json               | 3.55       | 18  | 281.31   | 281.75   | 281.90   | 282.01   | 282.03   | 280.43   | 282.04   | 2.15x                        |
| tsv-json-no-locations       | 6.75       | 34  | 148.08   | 148.57   | 148.70   | 148.74   | 149.13   | 147.63   | 149.32   | 4.08x                        |
| tsv-wasm-json-no-locations  | 5.43       | 27  | 184.04   | 184.74   | 185.00   | 185.23   | 187.23   | 183.38   | 187.95   | 3.28x                        |
| tsv-internal                | 48.50      | 216 | 20.62    | 20.65    | 20.76    | 20.89    | 21.15    | 20.53    | 21.20    | 29.3x                        |
| tsv-wasm-internal           | 29.26      | 132 | 34.18    | 34.24    | 34.38    | 34.55    | 34.81    | 34.03    | 34.89    | 17.7x                        |
| rsvelte-parse               | 1.80       | 9   | 556.47   | 556.73   | 557.83   | —        | —        | 555.25   | 558.14   | 1.09x                        |
| rsvelte-parse-skip-expr-loc | 2.74       | 14  | 365.49   | 365.89   | 366.89   | 367.17   | 367.40   | 363.10   | 367.46   | 1.66x                        |

**Files (intersection):** 951

**Throughput:** svelte/compiler 4.0 MB/s, tsv-json 10.0 MB/s, tsv-wasm-json 8.6 MB/s, tsv-json-no-locations 16.3 MB/s, tsv-wasm-json-no-locations 13.1 MB/s, tsv-internal 117.0 MB/s, tsv-wasm-internal 70.6 MB/s, rsvelte-parse 4.3 MB/s, rsvelte-parse-skip-expr-loc 6.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 11.7x tsv-internal, tsv-wasm-json 8.2x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.19       | 8  | 5345.06  | 5437.31  | 5470.59  | —        | —        | 5232.06  | 5482.63  | baseline              |
| tsv        | 12.86      | 57 | 77.71    | 78.03    | 78.47    | 78.58    | 79.01    | 77.50    | 79.22    | 68.9x                 |
| tsv-wasm   | 7.97       | 37 | 125.30   | 125.98   | 126.47   | 127.54   | 129.70   | 124.91   | 130.08   | 42.7x                 |
| oxfmt      | 0.19       | 8  | 5355.05  | 5367.84  | 5397.74  | —        | —        | 5270.60  | 5405.53  | 1.00x                 |
| biome-wasm | 1.09       | 8  | 915.94   | 921.33   | 926.80   | —        | —        | 910.14   | 938.84   | 5.83x                 |

**Files (intersection):** 951

**Throughput:** prettier 0.5 MB/s, tsv 31.0 MB/s, tsv-wasm 19.2 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.6 MB/s

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.31       | 8  | 3.21    | 3.23    | 3.24    | —       | —       | 3.19    | 3.24    | baseline                      |
| tsv-json                   | 0.53       | 7  | 1.88    | 1.89    | 1.90    | —       | —       | 1.88    | 1.90    | 1.71x                         |
| tsv-wasm-json              | 0.47       | 8  | 2.11    | 2.12    | 2.12    | —       | —       | 2.10    | 2.12    | 1.52x                         |
| tsv-json-no-locations      | 1.11       | 7  | 0.90    | 0.90    | 0.90    | —       | —       | 0.90    | 0.91    | 3.56x                         |
| tsv-wasm-json-no-locations | 0.92       | 7  | 1.08    | 1.08    | 1.09    | —       | —       | 1.08    | 1.09    | 2.97x                         |
| tsv-internal               | 9.12       | 44 | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 29.3x                         |
| tsv-wasm-internal          | 5.65       | 29 | 0.18    | 0.18    | 0.18    | 0.18    | 0.18    | 0.18    | 0.18    | 18.2x                         |
| oxc-parser                 | 0.80       | 8  | 1.26    | 1.26    | 1.27    | —       | —       | 1.22    | 1.29    | 2.57x                         |
| oxc-parser-wasm            | 0.72       | 8  | 1.38    | 1.38    | 1.39    | —       | —       | 1.38    | 1.39    | 2.33x                         |
| yuku-parser                | 2.09       | 7  | 0.48    | 0.48    | 0.51    | —       | —       | 0.47    | 0.51    | 6.73x                         |
| yuku-parser-wasm           | 2.30       | 11 | 0.44    | 0.44    | 0.44    | 0.44    | 0.44    | 0.42    | 0.44    | 7.39x                         |
| swc                        | 0.58       | 8  | 1.73    | 1.73    | 1.74    | —       | —       | 1.73    | 1.74    | 1.86x                         |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 5.7 MB/s, tsv-json 9.8 MB/s, tsv-wasm-json 8.7 MB/s, tsv-json-no-locations 20.4 MB/s, tsv-wasm-json-no-locations 17.0 MB/s, tsv-internal 168.2 MB/s, tsv-wasm-internal 104.3 MB/s, oxc-parser 14.7 MB/s, oxc-parser-wasm 13.4 MB/s, yuku-parser 38.6 MB/s, yuku-parser-wasm 42.4 MB/s, swc 10.6 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv-wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv-wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv-wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 17.2x tsv-internal, tsv-wasm-json 11.9x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.08       | 6  | 13.06   | 13.08   | 13.09   | —       | —       | 13.05   | 13.10   | baseline              |
| tsv         | 2.30       | 11 | 0.44    | 0.44    | 0.44    | 0.44    | 0.44    | 0.43    | 0.44    | 30.0x                 |
| tsv-wasm    | 1.40       | 8  | 0.71    | 0.72    | 0.72    | —       | —       | 0.71    | 0.72    | 18.3x                 |
| oxfmt       | 1.08       | 8  | 0.93    | 0.93    | 0.93    | —       | —       | 0.92    | 0.93    | 14.1x                 |
| biome-wasm  | 0.22       | 7  | 4.55    | 4.55    | 4.56    | —       | —       | 4.54    | 4.57    | 2.88x                 |
| dprint-wasm | 0.27       | 8  | 3.71    | 3.71    | 3.71    | —       | —       | 3.71    | 3.71    | 3.52x                 |

**Files (intersection):** 2643

**Throughput:** prettier 1.4 MB/s, tsv 42.4 MB/s, tsv-wasm 25.8 MB/s, oxfmt 19.9 MB/s, biome-wasm 4.1 MB/s, dprint-wasm 5.0 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv-wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2645/2645 (100%), dprint-wasm 2645/2645 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 90.08      | 451  | 11.05    | 11.30    | 11.43    | 11.56    | 11.78    | 10.70    | 11.82    | baseline                     |
| tsv-json          | 50.14      | 218  | 19.87    | 20.26    | 20.63    | 21.59    | 22.85    | 19.54    | 33.03    | 0.56x                        |
| tsv-wasm-json     | 41.12      | 182  | 24.28    | 24.57    | 25.03    | 25.87    | 26.46    | 24.04    | 27.51    | 0.46x                        |
| tsv-internal      | 265.39     | 1217 | 3.77     | 3.77     | 3.79     | 3.80     | 3.88     | 3.75     | 4.00     | 2.95x                        |
| tsv-wasm-internal | 149.31     | 628  | 6.69     | 6.73     | 6.81     | 6.84     | 6.93     | 6.66     | 7.05     | 1.66x                        |
| postcss           | 81.05      | 404  | 12.31    | 12.52    | 12.78    | 12.91    | 13.14    | 11.88    | 13.69    | 0.90x                        |

**Files (intersection):** 55

**Throughput:** svelte/compiler 36.5 MB/s, tsv-json 20.3 MB/s, tsv-wasm-json 16.7 MB/s, tsv-internal 107.5 MB/s, tsv-wasm-internal 60.5 MB/s, postcss 32.8 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 5.3x tsv-internal, tsv-wasm-json 3.6x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.60       | 7   | 626.73   | 629.61   | 640.54   | —        | —        | 620.71   | 664.48   | baseline              |
| tsv        | 143.58     | 605 | 6.97     | 6.99     | 7.07     | 7.13     | 7.28     | 6.93     | 7.80     | 89.8x                 |
| tsv-wasm   | 80.09      | 371 | 12.48    | 12.51    | 12.55    | 12.67    | 12.89    | 12.43    | 12.97    | 50.1x                 |
| oxfmt      | 48.32      | 238 | 20.68    | 21.02    | 21.33    | 21.52    | 22.65    | 19.32    | 23.78    | 30.2x                 |
| biome-wasm | 9.77       | 45  | 102.19   | 103.14   | 104.17   | 104.45   | 114.98   | 101.33   | 118.04   | 6.11x                 |
| malva-wasm | 17.62      | 76  | 56.77    | 56.84    | 57.30    | 57.44    | 57.88    | 56.65    | 58.03    | 11.0x                 |

**Files (intersection):** 55

**Throughput:** prettier 0.6 MB/s, tsv 58.2 MB/s, tsv-wasm 32.4 MB/s, oxfmt 19.6 MB/s, biome-wasm 4.0 MB/s, malva-wasm 7.1 MB/s

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
| tsv (ffi) | 3.5 MB | 1.6 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.6 MB | 3.2x | 2.8x |
| tsv format (ffi) | 3.2 MB | 1.5 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 686.1 KB | 0.4x | 0.4x |
| tsv (napi) | 3.9 MB | 1.8 MB | 1.1x | 1.1x |
| oxc-parser (napi) | 2.1 MB | 882.6 KB | 0.6x | 0.5x |
| oxfmt (napi) | 9.1 MB | 3.7 MB | 2.6x | 2.3x |
| yuku-parser (napi) | 819.2 KB | 338.1 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 8.9 MB | 3.5 MB | 2.5x | 2.2x |
| rsvelte compiler (napi) | 17.6 MB | 7.4 MB | 5.0x | 4.6x |
| swc (napi) | 32.7 MB | 12.2 MB | 9.3x | 7.5x |

_`vs tsv` divides native rows by `tsv (ffi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm rows by `tsv-wasm`. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **68.9x** prettier, **68.8x** oxfmt |
| format typescript (2643f) | **30.0x** prettier, **2.13x** oxfmt |
| format css (55f) | **89.8x** prettier, **2.97x** oxfmt |
| parse svelte (951f) | **2.51x** svelte/compiler, **2.31x** rsvelte-parse |
| parse typescript (2642f) | **1.71x** acorn-typescript, **0.66x** oxc-parser, **0.25x** yuku-parser, **0.92x** swc |
| parse css (55f) | **0.56x** svelte/compiler, **0.62x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **42.7x** prettier, **7.32x** biome-wasm |
| format typescript (2643f) | **18.3x** prettier, **6.36x** biome-wasm, **5.19x** dprint-wasm |
| format css (55f) | **50.1x** prettier, **8.20x** biome-wasm, **4.55x** malva-wasm |
| parse svelte (951f) | **2.15x** svelte/compiler |
| parse typescript (2642f) | **1.52x** acorn-typescript, **0.65x** oxc-parser-wasm, **0.21x** yuku-parser-wasm |
| parse css (55f) | **0.46x** svelte/compiler, **0.51x** postcss |

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
