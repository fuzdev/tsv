# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.4.2

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-09-15T17:47:22.394Z — tsv 0.3.0 (29a107e9)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@e8d37ffc1](https://github.com/fuzdev/corpora/tree/e8d37ffc18b8b8b743aaa93eca015871a7f6e98a) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_gitops/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 1.23       | 8   | 819.82   | 826.12   | 827.82   | —        | —        | 787.02   | 830.19   | baseline                     |
| tsv-json                    | 6.06       | 31  | 165.04   | 165.97   | 166.37   | 167.63   | 169.19   | 161.79   | 169.33   | 4.92x                        |
| tsv_wasm-json               | 5.89       | 30  | 169.69   | 170.51   | 171.55   | 172.51   | 173.95   | 167.54   | 174.33   | 4.79x                        |
| tsv-json-no-locations       | 8.44       | 42  | 118.54   | 119.16   | 120.11   | 120.33   | 122.38   | 116.50   | 123.31   | 6.85x                        |
| tsv_wasm-json-no-locations  | 7.85       | 38  | 127.44   | 128.15   | 128.89   | 129.49   | 131.26   | 125.72   | 131.41   | 6.38x                        |
| tsv-internal                | 50.42      | 230 | 19.83    | 19.90    | 20.05    | 20.25    | 20.60    | 19.71    | 20.86    | 41.0x                        |
| tsv_wasm-internal           | 32.88      | 139 | 30.42    | 30.47    | 30.68    | 31.21    | 31.77    | 30.30    | 32.10    | 26.7x                        |
| rsvelte-parse               | 2.08       | 10  | 482.64   | 483.77   | 486.06   | 492.44   | 497.54   | 478.24   | 498.81   | 1.69x                        |
| rsvelte-parse-skip-expr-loc | 3.05       | 16  | 325.48   | 332.81   | 336.48   | 337.44   | 338.03   | 321.13   | 338.18   | 2.48x                        |

**Files (intersection):** 951

**Throughput:** svelte/compiler 3.0 MB/s, tsv-json 14.6 MB/s, tsv_wasm-json 14.2 MB/s, tsv-json-no-locations 20.3 MB/s, tsv_wasm-json-no-locations 18.9 MB/s, tsv-internal 121.6 MB/s, tsv_wasm-internal 79.3 MB/s, rsvelte-parse 5.0 MB/s, rsvelte-parse-skip-expr-loc 7.4 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 8.3x tsv-internal, tsv_wasm-json 5.6x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ---------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier   | 0.23       | 8  | 4.26    | 4.34    | 4.39    | —       | —       | 4.13    | 4.44    | baseline              |
| tsv        | 12.47      | 59 | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 53.3x                 |
| tsv_wasm   | 8.78       | 41 | 0.11    | 0.11    | 0.11    | 0.11    | 0.12    | 0.11    | 0.12    | 37.5x                 |
| oxfmt      | 0.22       | 8  | 4.62    | 4.68    | 4.71    | —       | —       | 4.50    | 4.73    | 0.92x                 |
| biome-wasm | 0.95       | 8  | 1.06    | 1.06    | 1.06    | —       | —       | 1.03    | 1.06    | 4.07x                 |

**Files (intersection):** 951

**Throughput:** prettier 0.6 MB/s, tsv 30.1 MB/s, tsv_wasm 21.2 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.3 MB/s

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- |
| acorn-typescript           | 0.19       | 8  | 5316.74  | 5341.44  | 5346.98  | —        | —        | 5231.24  | 5351.52  | baseline                      |
| tsv-json                   | 0.88       | 8  | 1134.34  | 1138.72  | 1139.88  | —        | —        | 1129.59  | 1139.93  | 4.67x                         |
| tsv_wasm-json              | 0.89       | 8  | 1126.62  | 1130.68  | 1132.17  | —        | —        | 1123.70  | 1132.25  | 4.70x                         |
| tsv-json-no-locations      | 1.53       | 8  | 656.76   | 659.89   | 660.73   | —        | —        | 647.82   | 662.33   | 8.09x                         |
| tsv_wasm-json-no-locations | 1.46       | 8  | 688.00   | 692.19   | 692.62   | —        | —        | 675.47   | 692.89   | 7.72x                         |
| tsv-internal               | 9.99       | 50 | 100.18   | 100.32   | 100.46   | 100.53   | 100.68   | 99.61    | 100.70   | 52.9x                         |
| tsv_wasm-internal          | 6.93       | 35 | 144.30   | 144.54   | 144.77   | 144.87   | 145.24   | 143.75   | 145.41   | 36.7x                         |
| oxc-parser                 | 1.15       | 8  | 870.53   | 872.14   | 873.26   | —        | —        | 867.55   | 874.05   | 6.08x                         |
| oxc-parser-wasm            | 0.88       | 8  | 1135.76  | 1172.58  | 1186.68  | —        | —        | 1104.28  | 1197.18  | 4.64x                         |
| yuku-parser                | 2.91       | 13 | 338.75   | 359.89   | 379.14   | 399.14   | 420.56   | 332.46   | 425.91   | 15.4x                         |
| yuku-parser-wasm           | 3.57       | 15 | 278.99   | 289.84   | 297.47   | 304.69   | 324.09   | 273.72   | 328.94   | 18.9x                         |
| swc                        | 0.74       | 8  | 1349.01  | 1352.54  | 1353.94  | —        | —        | 1342.01  | 1355.07  | 3.93x                         |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 3.5 MB/s, tsv-json 16.2 MB/s, tsv_wasm-json 16.4 MB/s, tsv-json-no-locations 28.1 MB/s, tsv_wasm-json-no-locations 26.9 MB/s, tsv-internal 184.2 MB/s, tsv_wasm-internal 127.8 MB/s, oxc-parser 21.2 MB/s, oxc-parser-wasm 16.1 MB/s, yuku-parser 53.7 MB/s, yuku-parser-wasm 65.8 MB/s, swc 13.7 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv_wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv_wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv_wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 11.3x tsv-internal, tsv_wasm-json 7.8x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.08       | 8  | 13.30   | 13.36   | 13.40   | —       | —       | 13.21   | 13.43   | baseline              |
| tsv         | 2.23       | 11 | 0.45    | 0.45    | 0.45    | 0.45    | 0.45    | 0.45    | 0.45    | 29.6x                 |
| tsv_wasm    | 1.58       | 8  | 0.63    | 0.64    | 0.64    | —       | —       | 0.63    | 0.64    | 21.0x                 |
| oxfmt       | 1.09       | 7  | 0.92    | 0.92    | 0.93    | —       | —       | 0.91    | 0.95    | 14.5x                 |
| biome-wasm  | 0.23       | 8  | 4.33    | 4.34    | 4.37    | —       | —       | 4.28    | 4.37    | 3.07x                 |
| dprint-wasm | 0.31       | 8  | 3.22    | 3.22    | 3.22    | —       | —       | 3.21    | 3.22    | 4.14x                 |

**Files (intersection):** 2643

**Throughput:** prettier 1.4 MB/s, tsv 41.1 MB/s, tsv_wasm 29.1 MB/s, oxfmt 20.1 MB/s, biome-wasm 4.3 MB/s, dprint-wasm 5.7 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv_wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2645/2645 (100%), dprint-wasm 2645/2645 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 50.10      | 239  | 19.96    | 20.08    | 20.28    | 20.47    | 22.01    | 19.37    | 22.20    | baseline                     |
| tsv-json          | 61.23      | 263  | 16.27    | 16.62    | 17.34    | 17.79    | 18.30    | 16.01    | 19.17    | 1.22x                        |
| tsv_wasm-json     | 60.78      | 241  | 16.45    | 16.66    | 17.22    | 17.34    | 17.67    | 16.29    | 19.73    | 1.21x                        |
| tsv-internal      | 239.72     | 1062 | 4.17     | 4.18     | 4.20     | 4.22     | 4.31     | 4.16     | 8.18     | 4.78x                        |
| tsv_wasm-internal | 153.43     | 687  | 6.52     | 6.52     | 6.54     | 6.58     | 6.68     | 6.49     | 6.79     | 3.06x                        |
| postcss           | 72.60      | 299  | 13.76    | 14.08    | 14.79    | 14.98    | 17.67    | 13.45    | 18.98    | 1.45x                        |

**Files (intersection):** 55

**Throughput:** svelte/compiler 20.3 MB/s, tsv-json 24.8 MB/s, tsv_wasm-json 24.6 MB/s, tsv-internal 97.1 MB/s, tsv_wasm-internal 62.2 MB/s, postcss 29.4 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 3.9x tsv-internal, tsv_wasm-json 2.5x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 2.07       | 11  | 483.87   | 485.68   | 494.70   | 496.13   | 497.29   | 475.04   | 497.57   | baseline              |
| tsv        | 126.60     | 596 | 7.89     | 7.92     | 7.98     | 8.02     | 8.21     | 7.83     | 8.81     | 61.2x                 |
| tsv_wasm   | 87.86      | 387 | 11.38    | 11.41    | 11.50    | 11.54    | 11.73    | 11.29    | 12.05    | 42.5x                 |
| oxfmt      | 49.91      | 239 | 20.03    | 20.43    | 20.93    | 21.46    | 23.04    | 18.27    | 23.38    | 24.1x                 |
| biome-wasm | 11.56      | 45  | 86.20    | 87.70    | 89.20    | 90.15    | 103.58   | 85.41    | 114.80   | 5.59x                 |
| malva-wasm | 16.62      | 72  | 60.18    | 60.28    | 60.63    | 60.80    | 61.02    | 60.05    | 61.05    | 8.04x                 |

**Files (intersection):** 55

**Throughput:** prettier 0.8 MB/s, tsv 51.3 MB/s, tsv_wasm 35.6 MB/s, oxfmt 20.2 MB/s, biome-wasm 4.7 MB/s, malva-wasm 6.7 MB/s

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
| format svelte (951f) | **53.3x** prettier, **57.6x** oxfmt |
| format typescript (2643f) | **29.6x** prettier, **2.05x** oxfmt |
| format css (55f) | **61.2x** prettier, **2.54x** oxfmt |
| parse svelte (951f) | **4.92x** svelte/compiler, **2.92x** rsvelte-parse |
| parse typescript (2642f) | **4.67x** acorn-typescript, **0.77x** oxc-parser, **0.30x** yuku-parser, **1.19x** swc |
| parse css (55f) | **1.22x** svelte/compiler, **0.84x** postcss |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **37.5x** prettier, **9.22x** biome-wasm |
| format typescript (2643f) | **21.0x** prettier, **6.82x** biome-wasm, **5.06x** dprint-wasm |
| format css (55f) | **42.5x** prettier, **7.60x** biome-wasm, **5.29x** malva-wasm |
| parse svelte (951f) | **4.79x** svelte/compiler |
| parse typescript (2642f) | **4.70x** acorn-typescript, **1.01x** oxc-parser-wasm, **0.25x** yuku-parser-wasm |
| parse css (55f) | **1.21x** svelte/compiler, **0.84x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds), so it carries the same payload disclosure oxc-parser does — the mechanism matches `tsv-json` (serialize, cross, materialize) while the tree it produces is neither tsv’s loc-bearing drop-in shape nor its span-only wire; measured on the perf corpus its JSON is 0.64× `tsv-json`’s bytes. rsvelte-parse returns a compact JSON string the caller parses — the identical mechanism `tsv-json` measures (same serialize + boundary + `JSON.parse` cost) and within ~1.5% of its payload measured across the corpus (0.13% smaller in aggregate at the current pin, per-component median exactly 1.00 — the axis a throughput ratio integrates), so it is the one third-party parse row matched to tsv on BOTH axes. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since no Rust CSS parser exposes an AST to JS. Not payload-matched either: it keeps selectors and values as strings where `parseCss` (and so tsv) builds full ASTs — 0.38× tsv’s node count and at most 0.56× its JSON bytes on the perf corpus. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in every parse-capable package) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

## Unstable Rows

1 timed row(s) were not stable: a cv past 10% (std_dev / mean — `cv` after outlier removal; `cv (raw)` before it, which counts only under 30 raw samples, where one deviant sweep is a real share of the row) or a drift past 5% (the median of the second half of the timings against the first's — a cost that moved WHILE the row was measured, which the cleaned cv cannot see: a second mode is deleted or blended, not reported). The drift's sign names the mechanism: negative means the row got FASTER while measured (still warming up — under-warmed), positive means it got slower (degrading — a leak, a heap tipping over, thermal). Every `Nx` involving one of these divides a mean that may be neither mode — read it as approximate, and re-run before drawing a conclusion from it; a longer window does not converge a drifting row, it moves the answer.

| Row | cv | cv (raw) | drift | samples (cleaned/raw) |
| --- | ---: | ---: | ---: | ---: |
| parse/typescript/oxc-parser-wasm | 3.3% | 3.3% | +6.2% | 8/8 |

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
