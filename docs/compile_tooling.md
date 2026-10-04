# Compiler Tooling

> The tooling that grades `tsv_svelte_compile` against the canonical Svelte compiler — the one-file comparison tools, the wide-net corpus run and the differential fuzzer (all sidecar-dependent), plus the pure-Rust type-eraser comment census. The `deno task` entry points are indexed in [CLAUDE.md §Fixtures](../CLAUDE.md#fixtures-rust--deno-based); this doc is the full reference.

None of these run in `deno task check` — everything but the census needs the Deno sidecar, and that gate is pure Rust; the census is a sizing tool, not a gate. The two pure-Rust compiler audits (`conformance:audit:compiler`, `canonicalize:audit`) are gated there and live in [audits.md](audits.md), as does [`compile:fixtures:validate`](audits.md#compile-fixture-validation-compilefixturesvalidate) — split across both cadences, its parity legs pure Rust in `check` and its oracle-freshness leg sidecar-bound in `conformance`. The validation-suite ratchet, which shares the corpus pipeline but grades against a committed snapshot, has its own doc: [compile_validation_ratchet.md](compile_validation_ratchet.md).

The compiler is **runes-only**, so every tool below compiles in runes mode; the parity bar throughout is `compare_canonical` over `tsv_svelte_compile::canonicalize_js` reprints (an intent-erased reprint of both sides, tolerating a comment-POSITION difference — tsv's comment placement vs the oracle's — so a remaining diff is a real code difference).

## One-file tools

Single-language commands: input is a path, `--content`, or `--stdin`, with no `--parser`.

```bash
cargo run -p tsv_debug canonical_compile file.svelte [--target server|client] [--css] [--dev] [--json] [--content|--stdin]
cargo run -p tsv_debug compile_compare file.svelte [--target server|client] [--content|--stdin] [--json]
cargo run -p tsv_debug render_compare a.svelte b.svelte   # two inputs: --content (repeatable) and/or paths; --json
cargo run -p tsv_debug compile_fixture_init tests/fixtures_compile/feature/case --content '<p>text</p>'  # also --stdin, --force
cargo run -p tsv_debug compile_fixtures_validate [pattern...]   # --list, --json
```

- **`canonical_compile`** — compile with the canonical compiler as a **deterministic oracle**: a fixed `cssHash` (`'svelte-tsvhash'`) and a constant filename make the output byte-identical run to run. Errors exit non-zero.
- **`compile_compare`** — tsv's compile vs the canonical compiler, diffing the canonicalized JS of both. Exit codes: 0 parity, 1 real diff, 2 error — including a component shape tsv doesn't cover yet, where it prints the oracle's canonical form as the target. `--json` emits `{ target, parity, comment_position_tolerated, ours_status, refusal: { bucket, message } | null, hunks }`. The ad-hoc one-file view; durable expectations live in the compile fixtures (`tests/fixtures_compile`).
- **`render_compare`** — do TWO Svelte sources render the same page? The pairwise triage arm of the render-equivalence oracles ([audits.md](audits.md), `render:audit`). Tiers: **identical** (compiled server JS byte-equal) / **cosmetic** (bytes differ, render key equal — same page) / **visible** (render keys differ). The key is static (no SSR execution), so a component with unresolvable imports still grades. Exit codes: 0 same render, 1 visible, 2 error.
- **`compile_fixture_init`** — create or reinit a compile fixture (`tests/fixtures_compile/<feature>/<case>/`): prettier-formats the runes component, oracle-compiles it (server target, non-dev), and writes `input.svelte` + `expected_server.js` (the canonicalized oracle JS) + `expected.css` (styled components only). Expected files are **always** oracle-generated, never hand-written.
- **`compile_fixtures_validate`** — the three per-fixture checks (oracle freshness, ours parity, expected idempotence) and their split gating: [audits.md §Compile-Fixture Validation](audits.md#compile-fixture-validation-compilefixturesvalidate).

## Corpus Comparison (`compile:corpus:compare`)

```bash
cargo run -p tsv_debug compile_corpus_compare <paths...>
# Also: --list, --json, --census
```

The compile-parity wide net: compile every `.svelte` under the given roots with the canonical compiler (oracle) AND tsv, comparing the canonical reprints of both sides. The `compile:corpus:compare` deno task points it at the whole `../corpora` snapshot (every collection), the private live roots it cannot carry, and the Svelte suites. Sidecar-dependent, so kept out of `deno task check`.

**Buckets per file:**

- **parity** — byte-exact OR comment-POSITION-tolerated (not a bug; surfaced in a separate `comment_position` sub-count).
- **refused** — sub-bucketed by refusal reason; a clean "not yet" UNLESS the reason is a deliberate runes-only fence (`Refusal::is_deliberate_fence`): the legacy directive syntax (a legacy `on:`/`let:`) and the legacy slot system (a `<slot>` / `<svelte:fragment>` / `<svelte:component>` / `<svelte:self>` tag, or a named `slot="…"` on a component child), each superseded by the oracle in Svelte 5. A fence is never a gap: those files are counted as `fenced` and SUBTRACTED from the achievable-parity denominator. NOT fenced: `<svelte:boundary>` (a first-class Svelte 5 feature and a real gap).
- **oracle-rejected** — legacy mode, invalid syntax; out of scope.
- **MISMATCH** — both compiled, canonical CODE differs; always a bug by the refusal contract.
- **error** — harness failure.

**Over-acceptance.** Every oracle-rejected file is also probed with tsv's `compile()`: a success is an OVER-ACCEPTANCE — nothing invalid in runes mode may compile, so it is a refusal-contract BUG, reported in a loud section and GATED like a mismatch. When tsv ALSO declines, its reason is kept and reported as the `Oracle-rejected, tsv refused (by tsv reason)` sub-bucket (`oracle_rejected_tsv_refusals` in `--json`), the complement of `over_acceptance` within `oracle_rejected`. It is the only tsv-side readout for an oracle-rejected file — `compile_compare --json` emits nothing there — and it is what tells a refusal that catches the shape under test apart from one firing for an unrelated reason (a distinction whose absence produced a false refutation).

**The target set.** The TARGET SET line prints the subtraction mechanically: `oracle_accepted − fenced = achievable`, plus parity as a % of it. `fenced` counts FIRST refusals, so it is a FLOOR — a file whose fence sits behind an earlier refusal is equally unreachable but uncounted (no sound cheap detector: a node walk over-counts component `on:`/`let:` and SSR-dropped `{:catch}` regions, a regex over-counts comments), leaving `achievable` too large and the parity rate a conservative UNDER-estimate. The `refusal_census` SIZES that floor without moving it: per refused file whose first refusal was not itself a fence, it asks whether a fence is present anyway, reported as a separate NON-participating line (`≥N further refused files CONTAIN a fenced construct`). Deliberately not subtracted — the census reaches the fenced special-element TAGS but neither `RunesOnlyFence` nor `ComponentNamedSlot` (it never inspects an attribute list), and it over-detects in a dropped `{:catch}` where those tags COMPILE, so its residual error is not one-directional. Subtracting would raise the published parity rate with zero behavior change, on a partial and unsigned signal.

**Exit codes**: 0 clean, 1 FAILURE (mismatch or over-acceptance), 2 harness error. `--json` carries the full per-file path list per refusal / oracle-reject / over-acceptance bucket plus the `target_set` object, so a bucket's population (and a change's parity estimate) is checkable. `--census` is the sole-blocker refusal census: per refusal class, sole-blocker vs co-blocker counts over the oracle-accepted, tsv-refused files — the re-pricing diagnostic for deciding which refusal class to take on (incompatible with `--ratchet`).

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
cargo run --profile corpus -p tsv_debug compile_fuzz                 # tests/fixtures_compile
cargo run --profile corpus -p tsv_debug compile_fuzz --iterations 20000 --dump-dir /tmp/cf
deno task compile:fuzz                                              # the on-demand task
# Also: --seed, --max-mutations N, --limit N, --jobs N, --max-findings N, --list, --json
```

The compiler's **adversarial leg**: generate feature cross-products from the compile fixtures and grade each mutant against the canonical compiler. `compile_corpus_compare` is a wide net over REAL components, so it exercises every feature and still misses nearly every feature PAIR — every interaction bug the fuzzer has found was corpus-invisible while the full corpus was green. Build with `--profile corpus` (release + panic=unwind) so a panic in tsv's compile is caught and REPORTED as a finding rather than killing the run. Seed paths resolve like the other corpus tools' (globs accepted, overlapping paths deduplicated). Sidecar-dependent, so NOT in `deno task check`.

⚠️ **THE GATE IS CURRENTLY RED, BY DESIGN OF THE FINDINGS — not an ordinary green gate.** A `--seed 0 --iterations 20000` run reports over-acceptances across several oracle error codes plus mismatches, so it ALWAYS exits 1 today — run it for the current tally rather than trusting a figure in prose. It is a discovery tool with an open work list, not a regression gate, which is also why it is on demand. The findings are cataloged in [checklist_svelte_compiler.md](checklist_svelte_compiler.md) §The wider validation surface + §Mismatch classes under mutation. Turning it into a real gate wants a known-bug RATCHET keyed on the oracle error codes (gap_audit / blank_audit style) — the recommended follow-up.

**Operators** are AST/FEATURE level, never byte level: a mutant must stay oracle-COMPILABLE to grade anything, so each operator splices a whole well-formed construct at an offset read off tsv's own parse, and the document is re-anchored between operators. Each crosses two axes:

- a template read re-bound by a wrapping `{#each}`;
- an instance-script name re-bound by a block `{@const}`;
- a generated name (`$$payload`/`$$props`/`$$slots`/`$0`) declared in user scope;
- a construct injected into a server-DROPPED region (`{:catch}`, a `<svelte:boundary>` pending/failed snippet);
- a dropped `{#snippet}` exported from a module script (both the `export const` and the bare-specifier form — only the second reaches the oracle's `snippet_invalid_export` rule);
- a subtree wrapped in a new scope (two of the five wraps move it INTO a dropped region);
- a comment injected where a rewrite may re-span it;
- a subtree duplicated (generated-name ordering vs emission order);
- a directive added beside a spread;
- one exotic code point dropped into the JS/CSS/attribute/template positions whose languages disagree about whether it is whitespace;
- the cross-product engine — grafting one seed's template AND instance script into another, guarded on name collision and on a TS donor needing a TS host.

Seeds are `tests/fixtures_compile`, chosen because many fixtures are ALREADY 2-3-way feature crosses: mutating within a composed seed reaches interactions that layering onto a single-feature one does not.

**Grading.** MISMATCH (both compiled, canonical code differs) and OVER-ACCEPTANCE (the oracle REJECTED it, tsv compiled it) are both bugs by the refusal contract and both exit 1. A tsv refusal is a clean "not yet" and never a finding; a tsv PARSE rejection is bucketed and reported but not gated (a frontend question); a mutant whose JS does not PARSE (`js_parse_error`) is a generator defect, bucketed as `harness_invalid_js` and never gated — reporting a harness regression as a compiler bug is this tool's worst failure mode.

**Throughput.** tsv's compile runs FIRST and a refusal skips the sidecar entirely — a refusal is definitionally outside the target set, and tsv's compile is ~10-40x faster than a warm oracle round trip. That is the ONE lever; there is deliberately no batching protocol and no result cache (a content-addressed cache would be sound, the oracle being pinned deterministic, but has a near-0% hit rate on fresh mutants). The report prints the measured pass-through rate, since the throughput model rests on it. Measured: ~68% of mutants survive the pre-filter, and 3 concurrent sidecar slots at ~0.83 ms per round trip sustain ~218-235K oracle calls/min (mutants are 50-200 bytes, far smaller than real files, which is why the rate is high); no further throughput work is indicated.

**Determinism.** Every mutant is generated up front, single-threaded, from per-seed-file path-keyed PRNG streams scheduled round-robin; grading then fans out over the sidecar pool and results are re-sorted by index, so the report is a pure function of `--seed` + `--iterations` + the corpus, independent of `--jobs`. Corpus-add stability (a fixture add/rename changes only THAT file's mutants) holds for every operator but the donor graft and is pinned by a test; the graft is outside it by construction — a cross-product engine reads the whole corpus, so a corpus edit changes which donor a draw selects.

## Type-Eraser Comment Census (`erase_comment_census`)

```bash
cargo run --release -p tsv_debug -- erase_comment_census ../corpora/collections/fuz_ui ../corpora/collections/zzz
# Also: --verbose (per exposed file), --json
```

Sizes the type-eraser's comment-refusal haircut over a corpus (pure Rust, no Deno). Per `lang="ts"` component it collects the spans type erasure drops (TS-only statements, `: T` annotations, type params/args, `as`/`satisfies`/`!` tails, type-only imports/exports, `declare` items) and counts comments intersecting an erased span's refusal window — the span extended to the next surviving token, so `let x: Foo /* c */ = v` counts while a leading JSDoc on an erased interface (which survives erasure) does not.

The census measures the FORWARD half of that window only, while the compiler's real refusal window is bidirectional (it also reaches BACKWARD over a detached erased region — a return type, an `implements` clause, a `<T>` list — where a comment can sit between the region and the token before it). So the exposure rate it reports is a **LOWER BOUND** on the true refusal rate. It also flags cheaply-detectable non-TS blockers (directives/spread, special elements, module scripts, option/select, instance exports) to approximate "type stripping is this file's only blocker"; runes/derived/evaluator refusals are NOT detected, so that bucket is an approximation.
