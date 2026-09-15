# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.9.6

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-09-15T01:07:21.584Z — tsv 0.3.0 (b2f9c39e)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@e8d37ffc1](https://github.com/fuzdev/corpora/tree/e8d37ffc18b8b8b743aaa93eca015871a7f6e98a) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_gitops/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2, typescript@6.0.3

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 1.62       | 9   | 610.85   | 629.10   | 635.36   | —        | —        | 598.99   | 643.58   | baseline                     |
| tsv-json                    | 4.13       | 18  | 241.59   | 242.89   | 246.70   | 247.72   | 248.15   | 240.31   | 248.26   | 2.55x                        |
| tsv_wasm-json               | 3.52       | 14  | 284.43   | 286.21   | 291.12   | 291.47   | 292.99   | 282.67   | 293.37   | 2.17x                        |
| tsv-json-no-locations       | 6.62       | 34  | 150.22   | 151.92   | 155.00   | 155.47   | 155.90   | 148.48   | 156.02   | 4.08x                        |
| tsv_wasm-json-no-locations  | 5.34       | 25  | 186.54   | 189.25   | 191.55   | 191.84   | 192.22   | 185.07   | 192.32   | 3.29x                        |
| tsv-internal                | 47.03      | 170 | 21.24    | 21.72    | 22.07    | 22.78    | 22.99    | 21.10    | 23.17    | 29.0x                        |
| tsv_wasm-internal           | 28.43      | 120 | 35.02    | 35.84    | 36.03    | 36.13    | 36.27    | 34.85    | 36.51    | 17.5x                        |
| rsvelte-parse               | 1.80       | 7   | 556.50   | 561.70   | 570.05   | —        | —        | 554.05   | 576.96   | 1.11x                        |
| rsvelte-parse-skip-expr-loc | 2.73       | 13  | 366.46   | 367.23   | 371.15   | 374.71   | 379.98   | 362.66   | 381.30   | 1.68x                        |

**Files (intersection):** 951

**Throughput:** svelte/compiler 3.9 MB/s, tsv-json 10.0 MB/s, tsv_wasm-json 8.5 MB/s, tsv-json-no-locations 16.0 MB/s, tsv_wasm-json-no-locations 12.9 MB/s, tsv-internal 113.4 MB/s, tsv_wasm-internal 68.6 MB/s, rsvelte-parse 4.3 MB/s, rsvelte-parse-skip-expr-loc 6.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 11.4x tsv-internal, tsv_wasm-json 8.1x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.18       | 7  | 5435.04  | 5475.71  | 5508.95  | —        | —        | 5385.02  | 5525.32  | baseline              |
| tsv        | 12.49      | 57 | 79.62    | 81.65    | 83.19    | 84.16    | 84.62    | 78.82    | 84.66    | 68.0x                 |
| tsv_wasm   | 7.73       | 39 | 128.70   | 130.35   | 132.77   | 133.47   | 134.05   | 127.40   | 134.10   | 42.1x                 |
| oxfmt      | 0.19       | 7  | 5400.75  | 5424.83  | 5448.69  | —        | —        | 5339.37  | 5450.29  | 1.01x                 |
| biome-wasm | 1.10       | 6  | 912.04   | 913.55   | 914.92   | —        | —        | 910.87   | 915.86   | 5.96x                 |

**Files (intersection):** 951

**Throughput:** prettier 0.4 MB/s, tsv 30.1 MB/s, tsv_wasm 18.6 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.6 MB/s

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.31       | 3  | 3.25    | 3.25    | 3.25    | —       | —       | 3.25    | 3.26    | baseline                      |
| tsv-json                   | 0.53       | 5  | 1.89    | 1.89    | 1.90    | —       | —       | 1.88    | 1.90    | 1.72x                         |
| tsv_wasm-json              | 0.47       | 5  | 2.12    | 2.12    | 2.12    | —       | —       | 2.11    | 2.12    | 1.54x                         |
| tsv-json-no-locations      | 1.11       | 5  | 0.90    | 0.91    | 0.91    | —       | —       | 0.90    | 0.92    | 3.61x                         |
| tsv_wasm-json-no-locations | 0.92       | 5  | 1.08    | 1.09    | 1.09    | —       | —       | 1.08    | 1.09    | 3.00x                         |
| tsv-internal               | 8.71       | 44 | 0.11    | 0.12    | 0.12    | 0.12    | 0.12    | 0.11    | 0.12    | 28.3x                         |
| tsv_wasm-internal          | 5.76       | 23 | 0.17    | 0.17    | 0.18    | 0.18    | 0.18    | 0.17    | 0.18    | 18.8x                         |
| oxc-parser                 | 0.80       | 4  | 1.26    | 1.26    | 1.26    | —       | —       | 1.23    | 1.26    | 2.60x                         |
| oxc-parser-wasm            | 0.73       | 5  | 1.37    | 1.38    | 1.38    | —       | —       | 1.37    | 1.38    | 2.37x                         |
| yuku-parser                | 2.08       | 10 | 0.48    | 0.50    | 0.53    | 0.56    | 0.58    | 0.45    | 0.58    | 6.78x                         |
| yuku-parser-wasm           | 2.33       | 12 | 0.43    | 0.44    | 0.44    | 0.44    | 0.45    | 0.42    | 0.45    | 7.57x                         |
| swc                        | 0.58       | 5  | 1.72    | 1.72    | 1.72    | —       | —       | 1.71    | 1.72    | 1.90x                         |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 5.7 MB/s, tsv-json 9.8 MB/s, tsv_wasm-json 8.7 MB/s, tsv-json-no-locations 20.5 MB/s, tsv_wasm-json-no-locations 17.0 MB/s, tsv-internal 160.6 MB/s, tsv_wasm-internal 106.3 MB/s, oxc-parser 14.7 MB/s, oxc-parser-wasm 13.4 MB/s, yuku-parser 38.4 MB/s, yuku-parser-wasm 42.9 MB/s, swc 10.8 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv_wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv_wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv_wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 16.5x tsv-internal, tsv_wasm-json 12.2x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.08       | 7 | 13.09   | 13.11   | 13.14   | —       | —       | 13.04   | 13.15   | baseline              |
| tsv         | 2.30       | 8 | 0.44    | 0.44    | 0.45    | —       | —       | 0.43    | 0.45    | 30.1x                 |
| tsv_wasm    | 1.40       | 6 | 0.71    | 0.71    | 0.72    | —       | —       | 0.71    | 0.73    | 18.4x                 |
| oxfmt       | 1.11       | 6 | 0.91    | 0.91    | 0.91    | —       | —       | 0.90    | 0.91    | 14.5x                 |
| biome-wasm  | 0.22       | 5 | 4.57    | 4.57    | 4.58    | —       | —       | 4.57    | 4.58    | 2.86x                 |
| dprint-wasm | 0.27       | 5 | 3.72    | 3.73    | 3.73    | —       | —       | 3.71    | 3.73    | 3.52x                 |

**Files (intersection):** 2643

**Throughput:** prettier 1.4 MB/s, tsv 42.4 MB/s, tsv_wasm 25.9 MB/s, oxfmt 20.4 MB/s, biome-wasm 4.0 MB/s, dprint-wasm 5.0 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv_wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2645/2645 (100%), dprint-wasm 2645/2645 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 89.93      | 445  | 11.07    | 11.32    | 11.55    | 11.74    | 12.13    | 10.64    | 14.48    | baseline                     |
| tsv-json          | 49.06      | 240  | 20.22    | 20.76    | 21.21    | 21.69    | 22.58    | 19.72    | 24.28    | 0.55x                        |
| tsv_wasm-json     | 39.02      | 193  | 25.53    | 25.95    | 26.58    | 27.09    | 27.78    | 24.86    | 28.59    | 0.43x                        |
| tsv-internal      | 228.19     | 1141 | 4.38     | 4.40     | 4.45     | 4.48     | 4.52     | 4.32     | 4.55     | 2.54x                        |
| tsv_wasm-internal | 127.50     | 630  | 7.82     | 7.90     | 7.96     | 7.99     | 8.05     | 7.76     | 8.32     | 1.42x                        |
| postcss           | 80.17      | 395  | 12.43    | 12.68    | 13.02    | 13.23    | 13.69    | 11.94    | 16.48    | 0.89x                        |

**Files (intersection):** 55

**Throughput:** svelte/compiler 36.4 MB/s, tsv-json 19.9 MB/s, tsv_wasm-json 15.8 MB/s, tsv-internal 92.4 MB/s, tsv_wasm-internal 51.6 MB/s, postcss 32.5 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.7x tsv-internal, tsv_wasm-json 3.3x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.61       | 9   | 618.60   | 624.84   | 643.47   | —        | —        | 605.47   | 644.27   | baseline              |
| tsv        | 130.24     | 600 | 7.65     | 7.78     | 7.90     | 7.97     | 8.06     | 7.55     | 8.18     | 80.9x                 |
| tsv_wasm   | 73.06      | 285 | 13.66    | 13.93    | 14.00    | 14.03    | 14.23    | 13.55    | 14.31    | 45.4x                 |
| oxfmt      | 49.07      | 244 | 20.32    | 20.85    | 21.37    | 21.54    | 22.53    | 18.63    | 24.67    | 30.5x                 |
| biome-wasm | 9.82       | 48  | 101.42   | 102.72   | 103.98   | 104.48   | 104.83   | 100.13   | 104.84   | 6.10x                 |
| malva-wasm | 17.54      | 67  | 56.94    | 57.84    | 58.42    | 58.55    | 59.73    | 56.69    | 63.69    | 10.9x                 |

**Files (intersection):** 55

**Throughput:** prettier 0.7 MB/s, tsv 52.8 MB/s, tsv_wasm 29.6 MB/s, oxfmt 19.9 MB/s, biome-wasm 4.0 MB/s, malva-wasm 7.1 MB/s

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
| format svelte (951f) | **68.0x** prettier, **67.3x** oxfmt |
| format typescript (2643f) | **30.1x** prettier, **2.08x** oxfmt |
| format css (55f) | **80.9x** prettier, **2.65x** oxfmt |
| parse svelte (951f) | **2.55x** svelte/compiler, **2.30x** rsvelte-parse |
| parse typescript (2642f) | **1.72x** acorn-typescript, **0.66x** oxc-parser, **0.25x** yuku-parser, **0.91x** swc |
| parse css (55f) | **0.55x** svelte/compiler, **0.61x** postcss |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **42.1x** prettier, **7.05x** biome-wasm |
| format typescript (2643f) | **18.4x** prettier, **6.41x** biome-wasm, **5.22x** dprint-wasm |
| format css (55f) | **45.4x** prettier, **7.44x** biome-wasm, **4.17x** malva-wasm |
| parse svelte (951f) | **2.17x** svelte/compiler |
| parse typescript (2642f) | **1.54x** acorn-typescript, **0.65x** oxc-parser-wasm, **0.20x** yuku-parser-wasm |
| parse css (55f) | **0.43x** svelte/compiler, **0.49x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds), so it carries the same payload disclosure oxc-parser does — the mechanism matches `tsv-json` (serialize, cross, materialize) while the tree it produces is neither tsv’s loc-bearing drop-in shape nor its span-only wire; measured on the perf corpus its JSON is 0.64× `tsv-json`’s bytes. rsvelte-parse returns a compact JSON string the caller parses — the identical mechanism `tsv-json` measures (same serialize + boundary + `JSON.parse` cost) and within ~1.5% of its payload measured across the corpus (0.13% smaller in aggregate at the current pin, per-component median exactly 1.00 — the axis a throughput ratio integrates), so it is the one third-party parse row matched to tsv on BOTH axes. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since no Rust CSS parser exposes an AST to JS. Not payload-matched either: it keeps selectors and values as strings where `parseCss` (and so tsv) builds full ASTs — 0.38× tsv’s node count and at most 0.56× its JSON bytes on the perf corpus. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in every parse-capable package) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

## Unstable Rows

1 timed row(s) were not stable: a cv past 10% (std_dev / mean — `cv` after outlier removal; `cv (raw)` before it, which counts only under 30 raw samples, where one deviant sweep is a real share of the row) or a drift past 5% (the median of the second half of the timings against the first's — a cost that moved WHILE the row was measured, which the cleaned cv cannot see: a second mode is deleted or blended, not reported). Every `Nx` involving one of these divides a mean that may be neither mode — read it as approximate, and re-run before drawing a conclusion from it; a longer window does not converge a drifting row, it moves the answer.

| Row | cv | cv (raw) | drift | samples (cleaned/raw) |
| --- | ---: | ---: | ---: | ---: |
| parse/typescript/yuku-parser | 4.8% | 7.8% | -5.1% | 10/11 |

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
