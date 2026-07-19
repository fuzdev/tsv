# Validation-Suite Ratchet

> Gate the Svelte compiler's over-acceptance debt against Svelte's own validation suites

`compile_corpus_compare --ratchet` grades a compile run over
`../svelte/packages/svelte/tests/compiler-errors` and
`../svelte/packages/svelte/tests/validator` against a committed, **path-keyed** known-bug
snapshot. Every line is a known bug and the file shrinking is the goal — the same ratchet
shape as [gap_audit](gap_audit.md) and [blank_audit](blank_audit.md), reusing the same
generic `Ratchet` / `SnapshotKey` substrate.

## Why these suites

The ordinary `compile:corpus:compare` corpus is real components: overwhelmingly *valid*
Svelte, so it exercises the compiler's **emission** and barely touches its **validation**
surface. These two suites are the inverse — roughly two thirds of their files are
deliberately invalid, each authored to trip exactly one oracle rule. They sat in no gated
corpus at all, while the compiler's known validation holes lived as prose in
[checklist_svelte_compiler.md §The wider validation surface](checklist_svelte_compiler.md).

A file the oracle **rejects** that tsv nevertheless **compiles** is an
**over-acceptance** — a refusal-contract bug, since nothing invalid in runes mode may
compile. There are enough of them today that a green pass/fail gate is unreachable, hence
the ratchet.

## Running it

```bash
deno task compile:validation          # the gate
deno task compile:validation:update   # re-pin after fixing (or newly refusing) findings

# A subtree spot-check — reported, but NOT graded (see Narrowing below):
cargo run -p tsv_debug compile_corpus_compare --ratchet ../svelte/packages/svelte/tests/validator
```

Sidecar-dependent (every file needs an oracle compile), so it is **not** in
`deno task check` — the same reason `compile:corpus:compare` and `compile:fuzz` are
freestanding. It is also outside `conformance` / `conformance:all` / publish Step 3b: the
compiler arc ships no artifact and the branch is unmerged.

⚠️ Always a **separate invocation** from `compile:corpus:compare`, never extra roots on
it. Folding a ~2/3-invalid corpus into that run would corrupt its
`parity / achievable` denominator, which is the arc's headline number.

## The snapshot

`crates/tsv_debug/src/cli/commands/compile_validation_known.txt`, machine-generated,
colocated with the code that owns it. One TAB-delimited record per line:

```
KIND<TAB>ORACLE_CODE<TAB>PATH
```

`PATH` is relative to its suite root (`validator/samples/foo/input.svelte`), so the
snapshot does not encode how the root was spelled on the machine that pinned it.

`ORACLE_CODE` is part of the key, not decoration: a file that starts being rejected by a
*different* rule (an oracle pin bump, a rewritten upstream sample) reads as one retired
line plus one new one and gets re-triaged, rather than silently matching the old entry.

### Why a path key works here

`compile_fuzz`'s findings are *generated* mutants, so a path key would be meaningless
there — a corpus edit rewrites which mutants exist, and the key has to be
oracle-code + normalized shape. These files are **authored and committed upstream** at
stable paths, so the path simply *is* the finding's identity.

### The four kinds

| kind | pinnable | meaning |
| --- | --- | --- |
| `OVER-ACCEPT` | **yes** | the oracle rejected it, tsv compiled it — the debt being ratcheted down |
| `MISMATCH` | **never** | both sides compiled and the canonical code differs — absolute, always fails |
| `ORACLE-ERROR` | **yes** | the **oracle itself** threw where it should have rejected (no `svelte.dev/e/` code) — upstream's bug |
| `HARNESS-ERROR` | **never** | any other harness failure — tsv's, the canonicalizer's, or the machine's |

A `MISMATCH` is unpinnable for the same reason a `PANIC` is in `gap_audit`: the invariant
it breaks is absolute, so `--update` must never launder one into the list whose shrinking
is the goal. It also fails by name in the verdict, redundantly with the pinnability rule,
so relaxing one can't silently ungate it.

`HARNESS-ERROR` exists because pinnability is decided by *whose bug it is*, and only one
harness failure is upstream's. The bucket that carries the oracle's throw also carries a
tsv compiler self-check firing (`tsv-corrupt-output`, `tsv-type-erasure-leak` — each a
compiler bug that fails its run everywhere else in this repo), a tsv parse over-rejection
(`tsv-parse`), a canonicalizer failure (`canonicalize-ours`, `canonicalize-oracle`,
`oracle-recanonicalize`, `oracle-non-idempotent`), and environment failures (`read`,
`oracle-sidecar`). Were those pinnable, a slice that introduced a corrupt output would
fail once as a `NEW` key and could then be laundered by the next `--update` into a list
whose header tells the reader an errored line is *upstream's* bug. Classification is by
error kind, defaulting to the **unpinnable** side, so a harness error kind added later
fails until someone decides it belongs upstream.

## The one `ORACLE-ERROR` line

`validator/samples/silence-warnings-2` makes the pinned oracle (svelte 5.56.4) **throw**:
it carries a `svelte-ignore` for a warning whose construction path dereferences an unset
source locator, dying in `state.js`'s `locator` with *"An impossible situation occurred"*.

Verified directly against the pin: that source compiles fine at the **default**
(auto-detected) mode and at `runes: false`, under both `generate` targets. It only throws
under `runes: true` — which the sidecar always forces, the oracle being runes-only by
design. Svelte's own harness does not force runes, which is why the sample is green
upstream. **It is not tsv's bug**, and it is permanent here until the pin moves.

It is pinned as its own kind rather than added to an exclusion list, so that it stays
visible (a snapshot line plus a paragraph in the file's header) *and* so a different
future harness error — on this file or any other — is a key the snapshot has never seen
and **fails**.

⚠️ The cost, stated plainly: an errored file gets no oracle verdict, so `classify` never
probes tsv on it. A pinned `ORACLE-ERROR` file could be hiding an over-acceptance of its
own. That is inherent to "the oracle cannot speak here", not a choice the ratchet makes.

## What is deliberately not pinned

`refused` and `fenced` counts. A refusal is not a defect — it is the honest "not yet" the
refusal contract rests on — and its buckets churn with every compiler slice, so pinning
them would fail on ordinary forward progress and get the gate turned off. The refusal
surface is already reported by the ordinary run and re-priced by `--census`.

## The verdict

Deliberately **not** the ordinary `exit_verdict`, which fails on any over-acceptance —
that is the very debt this gate ratchets, so reusing it would make the gate permanently
red. Two rules instead:

1. a `MISMATCH` fails, unconditionally;
2. the grade must hold — no new key, no stale key, no unpinnable key.

A blanket harness-`error` term is deliberately absent: an `ORACLE-ERROR` is a pinnable key
here, so the expected one must not also trip a blanket term, while an unexpected one
already fails — as a `NEW` key if it is another `ORACLE-ERROR`, and as an unpinnable key
if it is a `HARNESS-ERROR`.

Both `exit_verdict` and the ratchet verdict are pure functions of the report, so both are
unit-tested without a live sidecar.

## Narrowing

The snapshot is path-keyed against a fixed corpus, so grading it against a *different* one
would read every unreached line as stale. Passing explicit positional paths is therefore a
**narrowing**: the run still reports, but it is neither graded nor pinnable, and
`--update` refuses it outright.

⭐ **Not graded is not un-gated.** Only the ratchet *comparison* is skipped — the terms
that need no snapshot still fire, so a narrowed run exits non-zero on a `MISMATCH` or an
unpinnable `HARNESS-ERROR`. What it does not gate is the over-acceptance debt: a subtree
reaches an arbitrary slice of the ratcheted balance, so gating it would be permanently
red, which is the same reason the full gate does not reuse `exit_verdict`.

`RatchetArgs::narrowing_flags` is the single definition of "narrowed", read at both
decision points, with `every_narrowing_input_disqualifies_a_run` as the backstop —
anything added that changes *which* files are compared must be listed there, or `--update`
will pin a partial set and silently unpin real bugs.

A missing `../svelte` checkout is fail-closed either way it can be missing, by two
different mechanisms:

- the root path **does not exist** — discovery fails first, printing
  `Error: path not found: ../svelte/packages/svelte/tests/compiler-errors` and exiting
  non-zero *before any file is walked*. The run never reaches the ratchet at all;
- the roots exist but are **empty** (a partial or sparse checkout) — zero files are
  walked, the run does reach the ratchet, and every pinned line grades as `STALE`.

Neither passes vacuously; only the second is the wall-of-stale-entries case. A missing
snapshot file is a hard read error, never an empty set.

## Triage workflow

- **`NEW`** — a regression. Either tsv started compiling something the oracle rejects, or
  a harness error appeared. Fix it; do not re-pin.
- **`STALE`** — the finding stopped firing. Usually a win (a validation rule you ported,
  or a new refusal that covers the shape). Confirm it is a win rather than a corpus that
  moved, then `deno task compile:validation:update`.
- **`MISMATCH`** / **`HARNESS-ERROR`** — never pin. Each is tsv's own bug (or, for
  `read` / `oracle-sidecar`, a broken environment), not a line in the upstream-debt list.

`--update` prints a yield line (`over-acceptances −retired +added`) so a slice's effect on
the debt is visible without diffing the file. It counts `OVER-ACCEPT` keys only — a
retired `ORACLE-ERROR` is not an over-acceptance win, and is reported on its own line.

## Scope, and what a green run does not prove

- Only the two suites, only `.svelte` component files (`.svelte.ts` modules are excluded
  by the shared walk).
- The key is the *finding*, not a count — so a green run proves no new finding **shape**,
  never that the compiler's validation is correct on anything outside these suites.
- The suites are static and version-pinned to the `../svelte` checkout; a pin bump can
  legitimately move many lines at once. `deno task pins:audit` is what keeps the checkout
  and the sidecar pin aligned.
