# Typechecker (experimental)

> An **experimental** from-scratch TypeScript binder + checker (`tsv_check`) and the pure-Rust harness that grades it against tsgo's committed error baselines. Everything here is in development and **may never ship** — nothing in tsv's published artifacts depends on it, and none of its gates run at release cadence.

## Status

**Experimental, in development, may never ship.** `tsv_check` is a research crate: a
TypeScript binder and checker written from scratch in Rust, targeting exact TS7/tsgo
error conformance. The semantic phases land family by family; the pipeline skeleton
(parse → lower+bind → check → sort/dedup) is real, most of the type engine is not.
Treat it as a bet that has not paid out yet, not as a roadmap item with a date.

**Not in any shipped artifact.** No format or parse artifact links the crate —
`tsv_cli`, `tsv_ffi`, `tsv_wasm`, and `tsv_napi` never reference it. The only
consumer is `tsv_debug`, the dev-tooling binary (the workspace root reaches it only through
its dev-dependency on `tsv_debug`). Verify with:

```bash
cargo tree -i tsv_check   # tsv_debug (+ the workspace root, via its dev-dependency on tsv_debug)
```

It stays a workspace member, so `cargo check` / `cargo test` / `cargo clippy` cover it
like every other crate (its unit and integration tests are self-contained — synthetic
inline lib sources, no external oracle).

**Zero cost to the product.** The existing parser and formatter are never modified in
service of checking. A checker need that would change `tsv_ts`'s AST, spans, or
performance is the checker's problem to solve on its own side — that invariant is what
keeps an experiment that may never ship from taxing the things that already do.

**The oracle is tsgo.** Correctness is defined by TS7/tsgo's committed `.errors.txt`
error baselines over the tsc test corpus, read from a pinned `../typescript-go`
checkout (oracle pin `168e7015`, baselines under
`testdata/baselines/reference/submodule`). Observable behavior — which diagnostics
exist, their codes, spans, and order — is ported from tsgo; representation (dense u32
ids, SoA side columns, arena borrowing) is tsv's own.

## Gating status

The `tsc_conformance` deno tasks are **on-demand tools for typechecker sessions**,
nothing more:

- **NOT** in `deno task check` (which is external-oracle-free by design).
- **NOT** in `deno task conformance` / `conformance:all`, and **not release-gating** —
  `scripts/publish.ts` Step 3b neither runs them nor preflights their oracle.
- `../typescript-go` is therefore **not** a release-required checkout. `deno task
  doctor` reports its readiness under an explicitly optional section; an absent
  checkout is informational, never a failure, even under `--strict`.

The one thing doctor still warns about is a **broken** checkout: `../typescript-go`
present but its `testdata/baselines/reference/submodule` baselines missing, which
makes every command here fail. Absence says "you don't run these gates"; a
directory that looks like the oracle but isn't would mislead a typechecker
session, so it keeps its ⚠. An unmaterialized `_submodules/TypeScript` corpus or
absent bundled libs stay informational — `query` and `roundtrip` work without
them, and `index` / `run` disclose the gap themselves.

Until the typechecker ships, no ordinary dev or release flow pays for it.

**Setup** (only needed to run the gates):

```bash
git clone https://github.com/microsoft/typescript-go ../typescript-go   # the committed .errors.txt baselines
git -C ../typescript-go submodule update --init   # _submodules/TypeScript (corpus inputs, for `run`)
```

`query` and `roundtrip` run on a bare checkout. `index`, `run`, and `check-test` also need the
materialized `_submodules/TypeScript` corpus, and `run` additionally needs
`internal/bundled/libs` (the lib `.d.ts` set each variant resolves against).

## The harness (`tsc_conformance`)

A pure-Rust harness (no Deno) over tsgo's committed `.errors.txt` baselines. The
oracle-side tools (`query` / `roundtrip` / `index`) are **zero checker code** — they
read, re-render, and self-check the baselines and the corpus inputs. `run` and
`check-test` drive `tsv_check` against those same baselines.

Distinct from the parser-conformance surfaces: `deno task conformance:ts-repo` grades
tsv's *parser* against the tsc corpus, while this reads tsgo's *checker* error
output — the seam `tsv_check` emits through.

```bash
cargo run -p tsv_debug tsc_conformance query histogram           # per-TS-code instance counts + totals
cargo run -p tsv_debug tsc_conformance query tests-by-code 2454  # baselines mentioning a code
cargo run -p tsv_debug tsc_conformance query denominators        # test-identity / variant / JSX sizing
cargo run -p tsv_debug tsc_conformance roundtrip                 # parse every baseline → re-render → byte-compare
cargo run -p tsv_debug --quiet tsc_conformance run               # the conformance sweep over tsv_check
cargo run -p tsv_debug --quiet tsc_conformance check-test duplicateVar --variant target=es2015  # inner dev loop: one test, diagnostics vs baseline
cargo run -p tsv_debug tsc_conformance index                     # the corpus-INPUT side: directives, @filename units, varyBy variants
deno task conformance:tsc-roundtrip                              # roundtrip, as a deno task
deno task conformance:tsc-check                                  # run + writes benches/js/results/report.tsc-conformance.{json,md}
deno task conformance:tsc-check:update                           # re-pin the count snapshot + refresh the report (full runs only; refuses a red run)
# Common options: --path <typescript-go> (default ../typescript-go), --json; --verbose on roundtrip/index only.
# roundtrip: filter by path substring (skips the pins). run: triage filters --test <substr> /
#   --code <n> / --variant k=v / --family {dup,flow,all} skip the pins (invariant gates still
#   hold); --emit-manifest <path>, --report <path> (full-run only), --update (full-run only).
# check-test: --variant k=v (one variant); --dump-flow dumps the first unit's control-flow
#   graph as Graphviz DOT instead of the diagnostic diff.
```

**`roundtrip`** proves the `.errors.txt` parser + renderer port in one move: parse every
baseline, re-render it, byte-compare. The 14 ANSI `pretty=true` baselines take their own
colored model but stay in the denominator, so round-trip is 100%.

**`index`** proves three gates against the on-disk baselines: the baseline join, the
unit-text round-trip, and the exact denominator pins.

**`run`** sweeps every in-scope variant (single-file, non-JSX, non-JS-flavored,
non-skipped) through parse → lower+bind → check → sort/dedup. It grades expect-clean
variants (zero-diagnostic) plus two families as codes+spans multisets — the bind/merge
duplicate-conflict family (TS2300/2451/2567/2528 + merge-path codes) and the flow family
(TS7027 unreachable code, TS7028 unused label). `extra = 0` is a hard gate; `missing` is
classified by deferred cause (`merge` / `lib` / `deferred_late_bound` / `deferred_cfa` /
`other`, the last a HARD zero — any unclassified miss is a cascade or construction bug).
It also publishes the parse-divergence census, runs each test `catch_unwind`-wrapped on a
generous-stack worker (tracked parser crashes live in a pinned `CRASH_EXCLUSIONS`
ledger), and drops per-test `.diff` artifacts under `target/tsc_conformance/diffs/` on
failure.

The committed `benches/js/results/report.tsc-conformance.{json,md}` is regenerated only
by the on-demand `conformance:tsc-check` and `conformance:tsc-check:update` tasks, so it
tracks typechecker-session activity rather than releases — expect it to lag the working
tree between sessions, and refresh it deliberately when the numbers it carries are the
point. (A re-pin refreshes it by construction, so the pins and the artifact move
together.)

## Pins & re-pinning

Every **full** (unfiltered) run enforces exact **two-sided** pins — a count that moves in
either direction fails. They live in two places, and the split is the point: what *drifts*
is machine-regenerated, what is a *contract* stays a hand-edited const.

**The snapshot** — `crates/tsv_debug/src/cli/commands/tsc_conformance_pins.txt`, a
committed `key = value` file the harness compiles in. It carries the tsv-side census and
denominators: in-scope tests / variants, expect-clean, baselined-parsed, the
parse-rejected buckets, script retries, the lib-base sizing, the family and sub-family
match/missing partitions, the deferred missing causes, the related-info match count, and
the carve-outs. Every one of these shifts by construction — a parser fix moves what
parses, a tsgo pull moves the corpus — so they are regenerated rather than hand-edited:

```bash
deno task conformance:tsc-check:update   # = tsc_conformance run --update
```

`--update` rewrites the file from the run's measured counts, prints one `old -> new`
line per changed key (or reports no drift when the file is already byte-identical), and
refreshes the committed `report.tsc-conformance.{json,md}` alongside it, so the artifact
and the pins never disagree. Three refusals keep it honest:

- **a narrowed run** — any of `--test` / `--code` / `--variant` / `--family` makes the
  counts a slice of what the snapshot means;
- **a red run** — the invariant gates below must be green first, so only drift is ever
  machine-written;
- **an unpinned oracle** — the checkout's baseline count must match the oracle-side pin,
  since these counts are denominators *of that corpus*. Moving typescript-go is a
  deliberate two-step: re-pin the oracle-side consts, then re-pin the counts.

A malformed snapshot (a bad line, an unknown / duplicate / missing key) fails a normal
run before the sweep, naming the offending line — but only *warns* under `--update`,
which then regenerates from zero (`0 -> N` per key). Regeneration is the fix for a file
mangled by a bad merge, so it must not be blocked behind the file it repairs.

**The Rust consts** — never machine-written, each for its own reason:

- The **semantically-zero gates** in `crates/tsv_debug/src/cli/commands/tsc_conformance/pins.rs`:
  `family_extra`, the unclassified (`other`) misses, `family_span_mismatch`, the
  related-info `missing` / `extra` / `span_mismatch`, the four lib error channels, and zero
  panics. A zero here is a contract, not a measurement — a red one means the run is broken,
  so `--update` refuses rather than pinning it. (`extra`, `missing other`, panics, stale
  crash exclusions, and the lib channels gate on a filtered triage run too.)
- **The crash-exclusion count**, which lives beside the `CRASH_EXCLUSIONS` ledger it
  describes, in `crates/tsv_debug/src/tsc_conformance/runner/grade.rs`. Ledger and count
  move together in one deliberate edit, and `--update` touches neither. A run also reports
  any entry that no longer panics — drop those and re-pin in the same change, since a
  stale exclusion silently carves a test out of the sweep forever.
- The **oracle-side pins** — the baseline count, the roundtrip pass count, the ANSI
  `pretty` carve-out, and the `INDEX_*` denominators. These move only on a deliberate
  typescript-go bump, which is a ritual rather than drift, so they stay hand-edited.

Triage filters (`--test` / `--code` / `--variant` / `--family`, and `roundtrip`'s path
filter) skip both pin blocks, since a narrowed run legitimately grades a slice; the hard
invariant gates still hold on those runs.

**Re-pinning is the normal ritual, not an alarm.** Run the update task and read what
moved. That is **not** a regression as long as:

- the hard gates stay zero — `extra`, `missing other`, `family_span_mismatch`, the four
  lib error channels, and the crash count;
- the family match/missing partition still adds up: per-family `(match, missing)` pins
  agree with the aggregates, and each missing is classified into a *deferred* cause
  rather than `other`.

A drop in `deferred_late_bound` / `deferred_cfa` (matches gained) is a real improvement —
re-pin it and say what earned it in the commit message.

Pin history belongs in commit messages, never in an in-file changelog comment.

## See also

- [crates/tsv_check/CLAUDE.md](../crates/tsv_check/CLAUDE.md) — the crate itself:
  position and invariants, module map, the binder/merge/check phases, and which tool
  answers which question.
- [crates/tsv_debug/CLAUDE.md](../crates/tsv_debug/CLAUDE.md) — the harness's module
  map: the baseline side, the corpus-input side, and the checker leg.
- [conformance_test262.md](conformance_test262.md) — the *parser*-side conformance
  surface, for contrast.
