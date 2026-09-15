# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.4.2

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-09-15T02:58:45.083Z — tsv 0.3.0 (9b06410e)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@e8d37ffc1](https://github.com/fuzdev/corpora/tree/e8d37ffc18b8b8b743aaa93eca015871a7f6e98a) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_gitops/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2, typescript@6.0.3

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 1.15       | 5   | 866.10   | 872.29   | 883.48   | —        | —        | 858.72   | 894.63   | baseline                     |
| tsv-json                    | 5.88       | 25  | 169.66   | 173.00   | 182.16   | 182.87   | 183.23   | 166.51   | 183.25   | 5.13x                        |
| tsv_wasm-json               | 5.87       | 21  | 170.64   | 186.59   | 189.11   | 189.73   | 191.02   | 168.12   | 191.41   | 5.11x                        |
| tsv-json-no-locations       | 8.17       | 41  | 119.36   | 127.31   | 130.87   | 131.29   | 131.78   | 116.03   | 132.01   | 7.12x                        |
| tsv_wasm-json-no-locations  | 7.49       | 38  | 129.94   | 139.13   | 143.55   | 143.65   | 143.99   | 125.96   | 144.17   | 6.53x                        |
| tsv-internal                | 50.39      | 208 | 19.83    | 20.06    | 20.54    | 20.65    | 20.75    | 19.64    | 24.29    | 43.9x                        |
| tsv_wasm-internal           | 32.82      | 122 | 30.48    | 30.92    | 31.55    | 31.64    | 31.89    | 30.29    | 32.08    | 28.6x                        |
| rsvelte-parse               | 1.98       | 8   | 503.82   | 505.51   | 514.16   | —        | —        | 496.49   | 520.50   | 1.73x                        |
| rsvelte-parse-skip-expr-loc | 2.94       | 12  | 340.21   | 341.17   | 344.94   | 348.07   | 349.50   | 337.04   | 349.86   | 2.57x                        |

**Files (intersection):** 951

**Throughput:** svelte/compiler 2.8 MB/s, tsv-json 14.2 MB/s, tsv_wasm-json 14.2 MB/s, tsv-json-no-locations 19.7 MB/s, tsv_wasm-json-no-locations 18.1 MB/s, tsv-internal 121.5 MB/s, tsv_wasm-internal 79.2 MB/s, rsvelte-parse 4.8 MB/s, rsvelte-parse-skip-expr-loc 7.1 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 8.6x tsv-internal, tsv_wasm-json 5.6x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ---------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier   | 0.24       | 5  | 4.23    | 4.28    | 4.32    | —       | —       | 4.19    | 4.35    | baseline              |
| tsv        | 12.46      | 51 | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 52.9x                 |
| tsv_wasm   | 8.73       | 44 | 0.11    | 0.12    | 0.12    | 0.12    | 0.12    | 0.11    | 0.12    | 37.1x                 |
| oxfmt      | 0.22       | 5  | 4.57    | 4.57    | 4.62    | —       | —       | 4.41    | 4.65    | 0.94x                 |
| biome-wasm | 0.91       | 4  | 1.11    | 1.12    | 1.23    | —       | —       | 1.07    | 1.31    | 3.86x                 |

**Files (intersection):** 951

**Throughput:** prettier 0.6 MB/s, tsv 30.0 MB/s, tsv_wasm 21.0 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.2 MB/s

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- |
| acorn-typescript           | 0.22       | 5  | 4632.14  | 4663.53  | 4679.07  | —        | —        | 4615.18  | 4689.44  | baseline                      |
| tsv-json                   | 0.87       | 5  | 1144.48  | 1148.24  | 1148.37  | —        | —        | 1142.16  | 1148.46  | 4.06x                         |
| tsv_wasm-json              | 0.89       | 5  | 1117.72  | 1121.00  | 1121.37  | —        | —        | 1112.13  | 1121.61  | 4.16x                         |
| tsv-json-no-locations      | 1.52       | 8  | 653.60   | 662.25   | 669.90   | —        | —        | 642.69   | 685.22   | 7.07x                         |
| tsv_wasm-json-no-locations | 1.46       | 8  | 685.33   | 692.68   | 702.31   | —        | —        | 671.02   | 717.39   | 6.76x                         |
| tsv-internal               | 10.01      | 42 | 99.86    | 100.39   | 101.94   | 102.26   | 102.39   | 99.01    | 102.40   | 46.5x                         |
| tsv_wasm-internal          | 6.86       | 30 | 145.78   | 146.55   | 148.47   | 148.88   | 149.37   | 145.00   | 149.55   | 31.9x                         |
| oxc-parser                 | 1.14       | 6  | 874.19   | 878.36   | 881.51   | —        | —        | 864.42   | 883.89   | 5.31x                         |
| oxc-parser-wasm            | 0.90       | 4  | 1113.90  | 1120.88  | 1146.98  | —        | —        | 1108.83  | 1164.37  | 4.17x                         |
| yuku-parser                | 2.91       | 14 | 338.66   | 356.10   | 362.41   | 370.89   | 383.22   | 331.88   | 386.30   | 13.5x                         |
| yuku-parser-wasm           | 3.43       | 18 | 287.09   | 302.32   | 306.51   | 310.76   | 321.23   | 276.47   | 323.84   | 15.9x                         |
| swc                        | 0.74       | 5  | 1355.81  | 1360.66  | 1363.37  | —        | —        | 1340.94  | 1365.17  | 3.43x                         |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 4.0 MB/s, tsv-json 16.1 MB/s, tsv_wasm-json 16.5 MB/s, tsv-json-no-locations 28.1 MB/s, tsv_wasm-json-no-locations 26.8 MB/s, tsv-internal 184.6 MB/s, tsv_wasm-internal 126.4 MB/s, oxc-parser 21.1 MB/s, oxc-parser-wasm 16.6 MB/s, yuku-parser 53.7 MB/s, yuku-parser-wasm 63.2 MB/s, swc 13.6 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv_wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv_wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv_wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 11.5x tsv-internal, tsv_wasm-json 7.7x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.07       | 5 | 13.97   | 13.98   | 14.00   | —       | —       | 13.94   | 14.01   | baseline              |
| tsv         | 2.23       | 9 | 0.45    | 0.45    | 0.45    | —       | —       | 0.45    | 0.46    | 31.2x                 |
| tsv_wasm    | 1.58       | 6 | 0.63    | 0.64    | 0.64    | —       | —       | 0.63    | 0.64    | 22.1x                 |
| oxfmt       | 1.11       | 6 | 0.90    | 0.90    | 0.91    | —       | —       | 0.90    | 0.91    | 15.5x                 |
| biome-wasm  | 0.23       | 4 | 4.40    | 4.41    | 4.74    | —       | —       | 4.33    | 4.95    | 3.19x                 |
| dprint-wasm | 0.31       | 5 | 3.21    | 3.21    | 3.21    | —       | —       | 3.20    | 3.21    | 4.36x                 |

**Files (intersection):** 2643

**Throughput:** prettier 1.3 MB/s, tsv 41.2 MB/s, tsv_wasm 29.2 MB/s, oxfmt 20.5 MB/s, biome-wasm 4.2 MB/s, dprint-wasm 5.8 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv_wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2645/2645 (100%), dprint-wasm 2645/2645 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 50.53      | 232  | 19.74    | 19.97    | 20.22    | 20.42    | 22.56    | 18.94    | 22.99    | baseline                     |
| tsv-json          | 58.07      | 287  | 16.88    | 18.12    | 18.69    | 18.92    | 20.09    | 16.07    | 21.29    | 1.15x                        |
| tsv_wasm-json     | 58.94      | 258  | 16.69    | 18.13    | 18.70    | 19.02    | 19.42    | 16.30    | 21.83    | 1.17x                        |
| tsv-internal      | 242.51     | 1042 | 4.12     | 4.14     | 4.20     | 4.28     | 4.55     | 4.09     | 5.19     | 4.80x                        |
| tsv_wasm-internal | 154.35     | 617  | 6.48     | 6.51     | 6.60     | 6.65     | 6.69     | 6.45     | 6.99     | 3.05x                        |
| postcss           | 70.70      | 331  | 14.04    | 14.49    | 14.99    | 15.43    | 17.67    | 13.37    | 19.43    | 1.40x                        |

**Files (intersection):** 55

**Throughput:** svelte/compiler 20.5 MB/s, tsv-json 23.5 MB/s, tsv_wasm-json 23.9 MB/s, tsv-internal 98.2 MB/s, tsv_wasm-internal 62.5 MB/s, postcss 28.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.2x tsv-internal, tsv_wasm-json 2.6x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 2.08       | 9   | 482.53   | 491.42   | 508.91   | —        | —        | 460.93   | 530.23   | baseline              |
| tsv        | 127.46     | 571 | 7.84     | 7.91     | 8.03     | 8.08     | 8.20     | 7.73     | 8.49     | 61.4x                 |
| tsv_wasm   | 88.30      | 392 | 11.31    | 11.43    | 11.58    | 11.70    | 11.88    | 11.19    | 12.11    | 42.5x                 |
| oxfmt      | 49.34      | 245 | 20.32    | 20.66    | 21.03    | 21.23    | 22.40    | 18.76    | 23.34    | 23.8x                 |
| biome-wasm | 11.42      | 29  | 87.62    | 88.33    | 91.54    | 98.42    | 116.54   | 86.93    | 118.45   | 5.50x                 |
| malva-wasm | 16.57      | 83  | 60.23    | 60.77    | 61.30    | 61.54    | 62.08    | 59.58    | 62.10    | 7.98x                 |

**Files (intersection):** 55

**Throughput:** prettier 0.8 MB/s, tsv 51.6 MB/s, tsv_wasm 35.8 MB/s, oxfmt 20.0 MB/s, biome-wasm 4.6 MB/s, malva-wasm 6.7 MB/s

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
| format svelte (951f) | **52.9x** prettier, **56.6x** oxfmt |
| format typescript (2643f) | **31.2x** prettier, **2.01x** oxfmt |
| format css (55f) | **61.4x** prettier, **2.58x** oxfmt |
| parse svelte (951f) | **5.13x** svelte/compiler, **2.96x** rsvelte-parse |
| parse typescript (2642f) | **4.06x** acorn-typescript, **0.76x** oxc-parser, **0.30x** yuku-parser, **1.18x** swc |
| parse css (55f) | **1.15x** svelte/compiler, **0.82x** postcss |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **37.1x** prettier, **9.61x** biome-wasm |
| format typescript (2643f) | **22.1x** prettier, **6.93x** biome-wasm, **5.07x** dprint-wasm |
| format css (55f) | **42.5x** prettier, **7.73x** biome-wasm, **5.33x** malva-wasm |
| parse svelte (951f) | **5.11x** svelte/compiler |
| parse typescript (2642f) | **4.16x** acorn-typescript, **1.00x** oxc-parser-wasm, **0.26x** yuku-parser-wasm |
| parse css (55f) | **1.17x** svelte/compiler, **0.83x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds), so it carries the same payload disclosure oxc-parser does — the mechanism matches `tsv-json` (serialize, cross, materialize) while the tree it produces is neither tsv’s loc-bearing drop-in shape nor its span-only wire; measured on the perf corpus its JSON is 0.64× `tsv-json`’s bytes. rsvelte-parse returns a compact JSON string the caller parses — the identical mechanism `tsv-json` measures (same serialize + boundary + `JSON.parse` cost) and within ~1.5% of its payload measured across the corpus (0.13% smaller in aggregate at the current pin, per-component median exactly 1.00 — the axis a throughput ratio integrates), so it is the one third-party parse row matched to tsv on BOTH axes. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since no Rust CSS parser exposes an AST to JS. Not payload-matched either: it keeps selectors and values as strings where `parseCss` (and so tsv) builds full ASTs — 0.38× tsv’s node count and at most 0.56× its JSON bytes on the perf corpus. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in every parse-capable package) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

## Unstable Rows

8 timed row(s) were not stable: a cv past 10% (std_dev / mean — `cv` after outlier removal; `cv (raw)` before it, which counts only under 30 raw samples, where one deviant sweep is a real share of the row) or a drift past 5% (the median of the second half of the timings against the first's — a cost that moved WHILE the row was measured, which the cleaned cv cannot see: a second mode is deleted or blended, not reported). The drift's sign names the mechanism: negative means the row got FASTER while measured (still warming up — under-warmed), positive means it got slower (degrading — a leak, a heap tipping over, thermal). Every `Nx` involving one of these divides a mean that may be neither mode — read it as approximate, and re-run before drawing a conclusion from it; a longer window does not converge a drifting row, it moves the answer.

| Row | cv | cv (raw) | drift | samples (cleaned/raw) |
| --- | ---: | ---: | ---: | ---: |
| parse/css/tsv_wasm-json | 4.2% | 5.7% | -9.3% | 258/291 |
| parse/svelte/tsv_wasm-json | 1.2% | 4.9% | -9.1% | 21/29 |
| parse/css/tsv-json | 5.7% | 6.0% | -9.0% | 287/290 |
| parse/svelte/tsv_wasm-json-no-locations | 4.9% | 4.9% | -8.2% | 38/38 |
| format/svelte/biome-wasm | 1.9% | 8.2% | -6.4% | 4/5 |
| parse/svelte/tsv-json-no-locations | 4.3% | 4.3% | -7.3% | 41/41 |
| format/typescript/biome-wasm | 0.8% | 5.7% | -6.7% | 4/5 |
| parse/typescript/yuku-parser | 3.4% | 4.6% | -5.8% | 14/15 |

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
