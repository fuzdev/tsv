# Compiler Tooling

> The sidecar-dependent harnesses that grade `tsv_svelte_compile` against the canonical Svelte compiler — the wide-net corpus run, the differential fuzzer, and the type-eraser comment census. The `deno task` entry points are indexed in [CLAUDE.md §Fixtures](../CLAUDE.md#fixtures-rust--deno-based); this doc is the full reference.

None of these run in `deno task check` — they need the Deno sidecar, and that gate is pure Rust. The two pure-Rust compiler audits (`conformance:audit:compiler`, `canonicalize:audit`) are gated there and live in [audits.md](audits.md); the validation-suite ratchet, which shares this pipeline but grades against a committed snapshot, has its own doc: [compile_validation_ratchet.md](compile_validation_ratchet.md).

## Corpus Comparison (`compile:corpus:compare`)

```bash
# compile_corpus_compare - the compile-parity wide net: compile every .svelte under the given roots
# with the canonical compiler (oracle) AND tsv, comparing the canonical reprints of both sides.
# Buckets per file: parity (byte-exact OR comment-POSITION-tolerated — tsv's comment placement vs the
# oracle's; not a bug, surfaced in a separate comment_position sub-count) / refused (sub-bucketed by
# refusal reason — a clean "not yet" UNLESS the reason is a deliberate runes-only fence
# (`Refusal::is_deliberate_fence`: the legacy directive syntax — a legacy `on:`/`let:` — and the legacy
# slot system — a `<slot>` / `<svelte:fragment>` / `<svelte:component>` / `<svelte:self>` tag, or a
# named `slot="…"` on a component child — each superseded by the oracle in Svelte 5), which is
# never a gap; those files are counted as `fenced` and SUBTRACTED from the achievable-parity
# denominator. NOT fenced: `<svelte:boundary>` (a first-class Svelte 5 feature and a real gap)) /
# oracle-rejected (legacy mode, invalid syntax; out
# of scope) / MISMATCH (both compiled, canonical CODE differs — always a bug by the refusal contract) /
# error (harness failure).
# Every oracle-rejected file is also probed with tsv's compile(): a success is an OVER-ACCEPTANCE —
# nothing invalid in runes mode may compile, so it is a refusal-contract BUG, reported in a loud
# section and GATED like a mismatch. A FAILURE there is not the only readout that probe
# yields: when tsv ALSO declines, its reason is kept and reported as the
# `Oracle-rejected, tsv refused (by tsv reason)` sub-bucket (`oracle_rejected_tsv_refusals`
# in `--json`), the complement of `over_acceptance` within `oracle_rejected`. It is the only
# tsv-side readout for an oracle-rejected file — `compile_compare --json` emits nothing there
# — and it is what tells a refusal that catches the shape under test apart from one firing
# for an unrelated reason (a distinction whose absence produced a false refutation).
# The TARGET SET line prints the subtraction mechanically: oracle_accepted − fenced = achievable, plus
# parity as a % of it. `fenced` counts FIRST refusals, so it is a FLOOR — a file whose fence sits
# behind an earlier refusal is equally unreachable but uncounted (no sound cheap detector: a node walk
# over-counts component `on:`/`let:` and SSR-dropped `{:catch}` regions, a regex over-counts comments),
# leaving `achievable` too large and the parity rate a conservative UNDER-estimate. The refusal
# `refusal_census` SIZES that floor without moving it: per refused file whose first refusal was not itself a
# fence, it asks whether a fence is present anyway, reported as a separate NON-participating line
# (`≥N further refused files CONTAIN a fenced construct`). Deliberately not subtracted — the census
# reaches the fenced special-element TAGS but neither `RunesOnlyFence` nor `ComponentNamedSlot` (it
# never inspects an attribute list), and it over-detects in a dropped `{:catch}` where those tags
# COMPILE, so its residual error is not one-directional. Subtracting would raise the published parity
# rate with zero behavior change, on a partial and unsigned signal.
# Exit codes: 0 clean, 1 FAILURE (mismatch or over-acceptance), 2 harness error. Sidecar-dependent —
# kept out of `deno task check`; the `compile:corpus:compare` deno task points it at the real-repo
# corpus + Svelte suites. `--json` carries the full per-file path list per refusal / oracle-reject /
# over-acceptance bucket plus the `target_set` object, so a bucket's population (and a slice's parity
# estimate) is checkable.
cargo run -p tsv_debug compile_corpus_compare <paths...>
# Also: --list, --json.
```

## Validation-Suite Ratchet (`compile:validation`)

The same pipeline as the corpus run above, pointed at Svelte's own `compiler-errors` +
`validator` suites and graded against a committed path-keyed known-bug snapshot instead of
a pass/fail verdict. It has its own reference doc — snapshot format, the four finding
kinds, what is deliberately not pinned, narrowing, and the triage workflow:
[compile_validation_ratchet.md](compile_validation_ratchet.md).

⚠️ Always a **separate invocation** from the corpus run above, never extra roots on it —
folding a ~2/3-invalid corpus in would corrupt that run's `parity / achievable` denominator.

```bash
cargo run -p tsv_debug compile_corpus_compare --ratchet            # the gate
cargo run -p tsv_debug compile_corpus_compare --ratchet --update   # re-pin
deno task compile:validation                                       # the on-demand tasks
```

## Differential Compile Fuzzer (`compile:fuzz`)

```bash
# compile_fuzz - the DIFFERENTIAL compile fuzzer: generate feature cross-products from the
# compile fixtures and grade each mutant against the canonical compiler. The compiler's
# adversarial leg. `compile_corpus_compare` is a wide net over REAL components, so it
# exercises every feature and still misses nearly every feature PAIR — every interaction
# bug found in this arc was corpus-invisible while the full corpus was green.
#
# Operators are AST/FEATURE level, never byte level: a mutant must stay oracle-COMPILABLE to
# grade anything, so each operator splices a whole well-formed construct at an offset read
# off tsv's own parse, and the document is re-anchored between operators. Eleven of them, each
# crossing two axes — a template read re-bound by a wrapping {#each}; an instance-script name
# re-bound by a block {@const}; a generated name ($$payload/$$props/$$slots/$0) declared in
# user scope; a construct injected into a server-DROPPED region ({:catch}, a <svelte:boundary>
# pending/failed snippet); a dropped {#snippet} exported from a module script (both the
# `export const` and the bare-specifier form — only the second reaches the oracle's
# snippet_invalid_export rule); a subtree wrapped in a new scope (two of the five wraps move
# it INTO a dropped region); a comment injected where a rewrite may re-span it; a subtree
# duplicated (generated-name ordering vs emission order); a directive added beside a spread;
# one exotic code point dropped into the JS/CSS/attribute/template positions whose languages
# disagree about whether it is whitespace; and the cross-product engine — grafting one seed's
# template AND instance script into another, guarded on name collision and on a TS donor
# needing a TS host. Seeds are
# `tests/fixtures_compile`, chosen because many fixtures are ALREADY 2-3-way feature crosses:
# mutating within a composed seed reaches interactions that layering onto a single-feature
# one does not.
#
# Grading: MISMATCH (both compiled, canonical code differs) and OVER-ACCEPTANCE (the oracle
# REJECTED it, tsv compiled it) are both bugs by the refusal contract and both exit 1; a tsv
# refusal is a clean "not yet" and never a finding; a tsv PARSE rejection is bucketed and
# reported but not gated (a frontend question); a mutant whose JS does not PARSE
# (`js_parse_error`) is a generator defect, bucketed as `harness_invalid_js` and never
# gated — reporting a harness regression as a compiler bug is this tool's worst failure
# mode. The parity bar is `compare_canonical`, the same comment-position-tolerant bar
# `compile_compare` uses.
#
# ⚠️ THE GATE IS CURRENTLY RED, BY DESIGN OF THE FINDINGS, NOT AS AN ORDINARY GREEN GATE.
# A `--seed 0 --iterations 20000` run reports over-acceptances across several oracle error
# codes plus mismatches, so it ALWAYS exits 1 today — run it for the current tally rather
# than trusting a figure in prose, which drifts with every slice. It is a discovery tool
# with an open work list, not a regression gate — which is also why it is on demand rather
# than in `deno task check`. The findings are cataloged in
# docs/checklist_svelte_compiler.md §The wider validation surface + §Mismatch classes
# under mutation. Turning it into a real gate wants a known-bug RATCHET keyed on the
# oracle error codes (gap_audit / blank_audit style) — the recommended follow-up slice.
#
# Throughput: tsv's compile runs FIRST and a refusal skips the sidecar entirely — a refusal is
# definitionally outside the target set, and tsv's compile is ~10-40x faster than a warm
# oracle round trip. That is the ONE lever; there is deliberately no batching protocol and no
# result cache (a content-addressed cache would be sound, the oracle being pinned
# deterministic, but has a near-0% hit rate on fresh mutants). The report prints the measured
# pass-through rate, since it is what the throughput model rests on. Measured: ~68% of
# mutants survive the pre-filter, and 3 concurrent sidecar slots at ~0.83 ms per round trip
# sustain ~218-235K oracle calls/min — well above the 35-75K originally predicted, because
# that prediction was calibrated for REAL-FILE-sized inputs and this generates 50-200-byte
# mutants. Not a measurement error; no further throughput work is indicated.
#
# Determinism: every mutant is generated up front, single-threaded, from per-seed-file
# path-keyed PRNG streams scheduled round-robin; grading then fans out over the sidecar pool
# and results are re-sorted by index, so the report is a pure function of --seed +
# --iterations + the corpus, independent of --jobs. Corpus-add stability (a fixture
# add/rename changes only THAT file's mutants) holds for ten operators and is pinned by a
# test; the donor GRAFT is outside it by construction — a cross-product engine reads the
# whole corpus, so a corpus edit changes which donor a draw selects.
cargo run --profile corpus -p tsv_debug compile_fuzz                 # tests/fixtures_compile
cargo run --profile corpus -p tsv_debug compile_fuzz --iterations 20000 --dump-dir /tmp/cf
deno task compile:fuzz                                              # the on-demand task
# Also: --seed, --max-mutations N, --limit N, --jobs N, --max-findings N, --list, --json.
# Build with `--profile corpus` (release + panic=unwind) so a panic in tsv's compile is caught
# and REPORTED as a finding rather than killing the run. Sidecar-dependent, so NOT in
# `deno task check` (which is the pure-Rust fixture gate).
```

## Type-Eraser Comment Census (`erase_comment_census`)

```bash
# erase_comment_census - size the type-eraser's comment-refusal haircut over a corpus (pure
# Rust, no Deno). Per lang="ts" component: collects the spans type erasure drops (TS-only
# statements, `: T` annotations, type params/args, as/satisfies/! tails, type-only
# imports/exports, declare items) and counts comments intersecting an erased span's refusal
# window — the span extended to the next surviving token, so `let x: Foo /* c */ = v` counts
# while a leading JSDoc on an erased interface (which survives erasure) does not. The census
# measures the FORWARD half of that window only, while the compiler's real refusal window is
# bidirectional (it also reaches BACKWARD over a detached erased region — a return type, an
# `implements` clause, a `<T>` list — where a comment can sit between the region and the token
# before it). So the exposure rate this reports is a LOWER BOUND on the true refusal rate.
# Also flags cheaply-detectable non-TS blockers (directives/spread, special elements, module
# scripts, option/select, instance exports) to approximate "type stripping is this file's only
# blocker"; runes/derived/evaluator refusals are NOT detected, so that bucket is an approximation.
cargo run --release -p tsv_debug -- erase_comment_census ../fuz_ui ../zzz
# Also: --verbose (per exposed file), --json.
```
