# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.4.2

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-09-16T20:50:50.310Z — tsv 0.4.0 (0d7c0251)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@e8d37ffc1](https://github.com/fuzdev/corpora/tree/e8d37ffc18b8b8b743aaa93eca015871a7f6e98a) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_gitops/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 1.22       | 8   | 830.41   | 835.65   | 840.61   | —        | —        | 792.58   | 848.61   | baseline                     |
| tsv-json                    | 6.05       | 30  | 165.33   | 166.54   | 167.15   | 167.98   | 169.77   | 162.71   | 170.28   | 4.97x                        |
| tsv-wasm-json               | 5.84       | 29  | 171.30   | 171.96   | 174.43   | 174.92   | 176.41   | 168.72   | 176.94   | 4.80x                        |
| tsv-json-no-locations       | 8.50       | 43  | 117.52   | 118.39   | 119.01   | 119.34   | 120.93   | 115.76   | 121.85   | 6.99x                        |
| tsv-wasm-json-no-locations  | 7.90       | 39  | 126.68   | 127.32   | 127.91   | 128.59   | 130.46   | 124.76   | 130.85   | 6.49x                        |
| tsv-internal                | 52.16      | 239 | 19.16    | 19.24    | 19.39    | 19.58    | 19.82    | 19.02    | 20.01    | 42.9x                        |
| tsv-wasm-internal           | 33.80      | 144 | 29.59    | 29.64    | 29.84    | 30.00    | 30.29    | 29.48    | 30.35    | 27.8x                        |
| rsvelte-parse               | 2.05       | 10  | 488.19   | 490.26   | 491.30   | 498.52   | 504.30   | 483.39   | 505.75   | 1.69x                        |
| rsvelte-parse-skip-expr-loc | 3.04       | 14  | 327.62   | 337.88   | 339.46   | 340.70   | 340.87   | 324.27   | 340.91   | 2.50x                        |

**Files (intersection):** 951

**Throughput:** svelte/compiler 2.9 MB/s, tsv-json 14.6 MB/s, tsv-wasm-json 14.1 MB/s, tsv-json-no-locations 20.5 MB/s, tsv-wasm-json-no-locations 19.1 MB/s, tsv-internal 125.8 MB/s, tsv-wasm-internal 81.5 MB/s, rsvelte-parse 4.9 MB/s, rsvelte-parse-skip-expr-loc 7.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 8.6x tsv-internal, tsv-wasm-json 5.8x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ---------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier   | 0.22       | 6  | 4.46    | 4.55    | 4.70    | —       | —       | 4.39    | 4.76    | baseline              |
| tsv        | 12.53      | 62 | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 55.8x                 |
| tsv-wasm   | 8.78       | 40 | 0.11    | 0.11    | 0.11    | 0.12    | 0.12    | 0.11    | 0.12    | 39.1x                 |
| oxfmt      | 0.21       | 6  | 4.74    | 4.83    | 5.03    | —       | —       | 4.68    | 5.21    | 0.94x                 |
| biome-wasm | 0.94       | 8  | 1.06    | 1.08    | 1.08    | —       | —       | 1.03    | 1.08    | 4.20x                 |

**Files (intersection):** 951

**Throughput:** prettier 0.5 MB/s, tsv 30.2 MB/s, tsv-wasm 21.2 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.3 MB/s

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- |
| acorn-typescript           | 0.19       | 8  | 5358.85  | 5365.00  | 5379.79  | —        | —        | 5313.15  | 5382.42  | baseline                      |
| tsv-json                   | 0.88       | 8  | 1129.47  | 1131.55  | 1133.32  | —        | —        | 1127.41  | 1134.57  | 4.74x                         |
| tsv-wasm-json              | 0.90       | 8  | 1106.80  | 1110.34  | 1112.03  | —        | —        | 1098.68  | 1113.35  | 4.84x                         |
| tsv-json-no-locations      | 1.54       | 8  | 649.59   | 652.86   | 654.38   | —        | —        | 644.85   | 656.72   | 8.24x                         |
| tsv-wasm-json-no-locations | 1.47       | 8  | 679.77   | 681.63   | 682.56   | —        | —        | 669.26   | 682.76   | 7.89x                         |
| tsv-internal               | 10.18      | 51 | 98.16    | 98.37    | 98.64    | 98.72    | 98.86    | 97.71    | 98.86    | 54.5x                         |
| tsv-wasm-internal          | 7.07       | 36 | 141.33   | 141.67   | 141.79   | 142.12   | 142.15   | 140.85   | 142.16   | 37.9x                         |
| oxc-parser                 | 1.14       | 8  | 879.78   | 880.58   | 881.19   | —        | —        | 875.10   | 882.28   | 6.09x                         |
| oxc-parser-wasm            | 0.87       | 8  | 1144.61  | 1154.98  | 1172.87  | —        | —        | 1111.80  | 1206.21  | 4.68x                         |
| yuku-parser                | 2.87       | 14 | 347.52   | 357.50   | 364.38   | 375.33   | 393.00   | 338.23   | 397.42   | 15.4x                         |
| yuku-parser-wasm           | 3.56       | 17 | 278.97   | 289.12   | 296.86   | 303.41   | 317.49   | 272.92   | 321.01   | 19.1x                         |
| swc                        | 0.74       | 7  | 1347.39  | 1350.23  | 1355.84  | —        | —        | 1342.79  | 1367.93  | 3.98x                         |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 3.4 MB/s, tsv-json 16.3 MB/s, tsv-wasm-json 16.7 MB/s, tsv-json-no-locations 28.4 MB/s, tsv-wasm-json-no-locations 27.2 MB/s, tsv-internal 187.8 MB/s, tsv-wasm-internal 130.4 MB/s, oxc-parser 21.0 MB/s, oxc-parser-wasm 16.1 MB/s, yuku-parser 52.9 MB/s, yuku-parser-wasm 65.6 MB/s, swc 13.7 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv-wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv-wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv-wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 11.5x tsv-internal, tsv-wasm-json 7.8x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.07       | 8  | 15.06   | 15.37   | 15.42   | —       | —       | 14.75   | 15.52   | baseline              |
| tsv         | 2.24       | 12 | 0.45    | 0.45    | 0.45    | 0.45    | 0.45    | 0.44    | 0.45    | 33.8x                 |
| tsv-wasm    | 1.59       | 8  | 0.63    | 0.63    | 0.63    | —       | —       | 0.63    | 0.63    | 24.0x                 |
| oxfmt       | 1.10       | 6  | 0.92    | 0.93    | 0.95    | —       | —       | 0.90    | 0.96    | 16.5x                 |
| biome-wasm  | 0.23       | 7  | 4.39    | 4.41    | 4.43    | —       | —       | 4.38    | 4.43    | 3.43x                 |
| dprint-wasm | 0.31       | 8  | 3.22    | 3.22    | 3.22    | —       | —       | 3.22    | 3.23    | 4.69x                 |

**Files (intersection):** 2643

**Throughput:** prettier 1.2 MB/s, tsv 41.4 MB/s, tsv-wasm 29.3 MB/s, oxfmt 20.2 MB/s, biome-wasm 4.2 MB/s, dprint-wasm 5.7 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv-wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2645/2645 (100%), dprint-wasm 2645/2645 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 49.45      | 239  | 20.24    | 20.36    | 20.48    | 20.70    | 23.39    | 19.65    | 23.55    | baseline                     |
| tsv-json          | 63.72      | 291  | 15.60    | 16.07    | 16.51    | 16.93    | 17.57    | 15.27    | 19.06    | 1.29x                        |
| tsv-wasm-json     | 65.66      | 264  | 15.21    | 15.45    | 15.87    | 15.96    | 16.53    | 15.06    | 18.33    | 1.33x                        |
| tsv-internal      | 289.02     | 1344 | 3.46     | 3.47     | 3.48     | 3.51     | 3.58     | 3.44     | 7.49     | 5.84x                        |
| tsv-wasm-internal | 189.16     | 892  | 5.29     | 5.30     | 5.31     | 5.33     | 5.47     | 5.26     | 5.58     | 3.82x                        |
| postcss           | 62.37      | 264  | 16.02    | 16.29    | 17.03    | 17.21    | 20.14    | 15.67    | 21.36    | 1.26x                        |

**Files (intersection):** 55

**Throughput:** svelte/compiler 20.0 MB/s, tsv-json 25.8 MB/s, tsv-wasm-json 26.6 MB/s, tsv-internal 117.1 MB/s, tsv-wasm-internal 76.6 MB/s, postcss 25.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.5x tsv-internal, tsv-wasm-json 2.9x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.89       | 10  | 526.98   | 532.99   | 536.67   | 537.92   | 538.93   | 522.48   | 539.18   | baseline              |
| tsv        | 138.56     | 629 | 7.21     | 7.24     | 7.30     | 7.35     | 7.56     | 7.15     | 7.93     | 73.3x                 |
| tsv-wasm   | 99.15      | 427 | 10.09    | 10.11    | 10.19    | 10.24    | 10.48    | 10.03    | 10.61    | 52.5x                 |
| oxfmt      | 49.67      | 241 | 20.14    | 20.44    | 21.05    | 21.48    | 23.46    | 18.65    | 24.24    | 26.3x                 |
| biome-wasm | 11.69      | 47  | 85.24    | 86.38    | 87.17    | 87.62    | 90.19    | 84.55    | 92.22    | 6.19x                 |
| malva-wasm | 16.40      | 69  | 60.96    | 61.06    | 61.58    | 61.67    | 61.90    | 60.83    | 62.01    | 8.68x                 |

**Files (intersection):** 55

**Throughput:** prettier 0.8 MB/s, tsv 56.1 MB/s, tsv-wasm 40.2 MB/s, oxfmt 20.1 MB/s, biome-wasm 4.7 MB/s, malva-wasm 6.6 MB/s

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
| format svelte (951f) | **55.8x** prettier, **59.2x** oxfmt |
| format typescript (2643f) | **33.8x** prettier, **2.05x** oxfmt |
| format css (55f) | **73.3x** prettier, **2.79x** oxfmt |
| parse svelte (951f) | **4.97x** svelte/compiler, **2.95x** rsvelte-parse |
| parse typescript (2642f) | **4.74x** acorn-typescript, **0.78x** oxc-parser, **0.31x** yuku-parser, **1.19x** swc |
| parse css (55f) | **1.29x** svelte/compiler, **1.02x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **39.1x** prettier, **9.30x** biome-wasm |
| format typescript (2643f) | **24.0x** prettier, **6.99x** biome-wasm, **5.12x** dprint-wasm |
| format css (55f) | **52.5x** prettier, **8.48x** biome-wasm, **6.04x** malva-wasm |
| parse svelte (951f) | **4.80x** svelte/compiler |
| parse typescript (2642f) | **4.84x** acorn-typescript, **1.03x** oxc-parser-wasm, **0.25x** yuku-parser-wasm |
| parse css (55f) | **1.33x** svelte/compiler, **1.05x** postcss |

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
