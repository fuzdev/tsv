# tsv benchmark results — cross-runtime

**Generated:** 2026-11-07T07:39:29.624Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.9.8: 872e4d3a @ 2026-11-07T04:38:21.464Z (tsv 0.8.1) — corpora dce40bd21
- `node` 24.14.1: 872e4d3a @ 2026-11-07T05:39:05.072Z (tsv 0.8.1) — corpora dce40bd21
- `bun` 1.4.4: 872e4d3a @ 2026-11-07T06:43:27.964Z (tsv 0.8.1) — corpora dce40bd21
- `conformance` (node, coverage-only): 872e4d3a @ 2026-11-07T07:38:20.031Z (tsv 0.8.1)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

**Native build:** every native tsv row (FFI under Deno, N-API under Node/Bun) measures a profile-guided build, trained on the corpus snapshot's `origin: own` collections — none of which any sibling’s corpus holds (zzz, fuz_app, fuz_blog, fuz_code, fuz_css, docs.fuz.dev, fuz_repos, fuz_template, fuz_ui, fuz_util, mdz, gro, svelte-docinfo, tsv.fuz.dev, ryanatkn.com, earbetter, cosmicplayground). The FFI library's profile is its own: the library itself, in process under Deno through the bench’s own FFI wrapper — parse, parse_internal and format on every file. The N-API addon's is the shipped addon's: the addon itself — parse on every file and format --jobs 1 over the tree through the @fuzdev/tsv loader’s cli.js, the type strip and import records in process — the training the published addon gets. WASM rows are not profile-guided.

**Not measured everywhere:** bun — oxc-parser-raw. The implementation behind each row failed to load on the runtime(s) named, so it contributes no measurement there — a row thinner than its neighbours, or missing outright, is a load failure rather than a speed result. The per-runtime report’s `unavailable` carries the impl and the cause.

**Unstable:** 4 per-runtime measurement(s) were not stable, so every ratio through them is unreadable — `parse/typescript/tsv-wasm` bun (cv 4.0%, raw 4.0%, drift +0.5%, pass spread 9.5%, n=24); `parse/typescript/oxc-parser-wasm` bun (cv 3.1%, raw 3.1%, drift +5.6%, pass spread 0.5%, n=24); `parse/typescript/oxc-parser-raw` node (cv 3.9%, raw 4.2%, drift +4.1%, pass spread 6.8%, n=27); `parse/css/svelte/compiler` deno (cv 3.1%, raw 3.3%, drift +0.2%, pass spread 6.0%, n=754). The cell is marked `⚠` in its table. A drift is a cost that moved WHILE the row was measured (the median of the second half of its timings against the first’s — negative: it got faster, still warming up; positive: it got slower, degrading); the cleaned cv cannot see it, and a longer window moves such a row’s answer rather than converging it. A pass spread is how far apart the row’s passes sat — each a fresh process — so a wide one is a level that depends on the process the row was drawn in. Re-run the runtime before reading the row, and read the per-runtime report’s §Unstable Rows for the row’s own detail.

**Within noise:** 3 per-runtime delta(s) are smaller than the two measurements' combined variation, so they are not runtime effects — `format/typescript/oxfmt` deno/node (0.4% vs 2.2% noise, n=3/3); `format/css/oxfmt` deno/node (1.4% vs 2.0% noise, n=3/3); `format/css/oxfmt` deno/bun (1.7% vs 2.1% noise, n=3/3). Read those cells as "no difference". The noise behind each side is the spread of its row's pass means (`entries[].pass_mean_ns` in the per-runtime JSON) where the row was timed in several fresh processes, and its sweep-level `cv` only where it was not — NOT that report's §Unstable Rows, which lists only rows past its own thresholds and so names none of these: a cell lands here whenever the delta is small relative to the noise. `n` counts what each side was read from — passes, or cleaned sweeps for a one-pass sibling — and a side under 3 passes or 10 sweeps is left unclassified rather than called quiet on an estimate that thin. Every pair of runtimes is classified, not only each against the ratio base, and a row named under **Unstable** above is never classified here.

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 1.4 | 1.3 | 1.2 | 0.87x | 0.85x |
| tsv | 4.0 | 3.7 | 7.0 | 0.91x | 1.74x |
| tsv-wasm | 3.1 | 3.1 | 5.4 | 1.01x | 1.73x |
| tsv+locations | 2.8 | 2.8 | 4.3 | 0.98x | 1.54x |
| tsv-wasm+locations | 2.3 | 2.4 | 3.7 | 1.06x | 1.59x |
| tsv-internal | 37.7 | 36.8 | 40.7 | 0.98x | 1.08x |
| tsv-wasm-internal | 18.4 | 19.8 | 21.8 | 1.08x | 1.18x |
| rsvelte-parse | 1.0 | 0.9 | 1.2 | 0.95x | 1.30x |
| rsvelte-parse-skip-expr-loc | 1.6 | 1.5 | 2.0 | 0.95x | 1.25x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.92x | 1.14x |
| tsv | 10.6 | 10.8 | 11.1 | 1.01x | 1.04x |
| tsv-wasm | 4.6 | 5.2 | 5.3 | 1.13x | 1.16x |
| oxfmt | 0.1 | 0.1 | 0.1 | 0.94x | 1.19x |
| biome-wasm | 0.5 | 0.4 | 0.5 | 0.88x | 1.03x |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.6 | 0.4 | 0.5 | 0.76x | 0.89x |
| tsv | 0.8 | 0.7 | 1.5 | 0.86x | 1.79x |
| tsv-wasm | 0.7 | 0.6 | 1.2 ⚠ | 0.97x | 1.77x ⚠ |
| tsv+locations | 0.7 | 0.6 | 1.0 | 0.90x | 1.55x |
| tsv-wasm+locations | 0.5 | 0.5 | 0.9 | 0.98x | 1.60x |
| tsv-internal | 7.8 | 7.1 | 8.3 | 0.91x | 1.07x |
| tsv-wasm-internal | 3.8 | 4.4 | 4.6 | 1.16x | 1.21x |
| oxc-parser | 0.5 | 0.5 | 1.0 | 0.90x | 1.89x |
| oxc-parser-wasm | 0.5 | 0.5 | 0.7 ⚠ | 0.95x | 1.44x ⚠ |
| oxc-parser-raw | 1.9 | 1.8 ⚠ | — | 0.94x ⚠ | — |
| yuku-parser | 1.5 | 1.8 | 2.1 | 1.23x | 1.40x |
| yuku-parser-wasm | 1.7 | 2.4 | 2.8 | 1.38x | 1.60x |
| swc | 0.4 | 0.4 | 0.6 | 0.91x | 1.54x |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.0 | 0.0 | 0.0 | 0.89x | 1.02x |
| tsv | 2.0 | 2.0 | 2.1 | 0.98x | 1.01x |
| tsv-wasm | 0.9 | 1.1 | 1.1 | 1.15x | 1.17x |
| oxfmt | 0.9 | 0.9 | 0.9 | 1.00x | 1.03x |
| biome-wasm | 0.1 | 0.1 | 0.1 | 0.98x | 1.10x |
| dprint-wasm | 0.2 | 0.2 | 0.2 | 1.14x | 1.17x |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 50.4 ⚠ | 47.5 | 27.2 | 0.94x ⚠ | 0.54x ⚠ |
| tsv | 31.5 | 27.1 | 53.5 | 0.86x | 1.70x |
| tsv-wasm | 24.8 | 24.4 | 46.7 | 0.98x | 1.89x |
| tsv+locations | 25.0 | 22.6 | 36.9 | 0.90x | 1.48x |
| tsv-wasm+locations | 20.6 | 21.0 | 33.9 | 1.02x | 1.64x |
| tsv-internal | 215.6 | 200.4 | 234.0 | 0.93x | 1.09x |
| tsv-wasm-internal | 108.3 | 121.6 | 144.4 | 1.12x | 1.33x |
| postcss | 46.1 | 44.4 | 35.6 | 0.96x | 0.77x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 1.0 | 1.0 | 1.2 | 0.97x | 1.17x |
| tsv | 122.1 | 113.9 | 126.9 | 0.93x | 1.04x |
| tsv-wasm | 60.9 | 69.3 | 79.3 | 1.14x | 1.30x |
| oxfmt | 29.1 | 28.7 | 29.6 | 0.99x | 1.02x |
| biome-wasm | 5.7 | 5.5 | 6.7 | 0.97x | 1.16x |
| malva-wasm | 12.5 | 13.7 | 12.2 | 1.10x | 0.98x |
