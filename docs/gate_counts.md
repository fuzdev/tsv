# Pinned Gate Counts

> Every graded gate and harvest enforces a committed expected count, so a change
> in *what gets graded* fails loudly instead of shifting inside a green run.

A gutted or refreshed suite checkout, a discovery bug, a tsv behavior change, or a
systemic sidecar/FFI failure eating a whole language all move a count. This is
`scripts/validate_artifacts.ts`'s tight-bounds philosophy applied to counts: every
real move in a number is a deliberate, visible edit.

## Where the numbers live

- **`benches/js/lib/gate_counts.ts`** — every Deno-side count, one per consumer:
  the fixtures gates (`scanned` + `both_accept` + `over_acceptance`), ts-repo
  (`scanned` + `accept_parity` + `over_acceptance` — the last one pins the WIDENING
  axis the other two structurally cannot see, since they fix only how many
  tsc-VALID files tsv accepts and leave the split of the rest free; the fixtures
  gates carry it for the same reason, a new over-acceptance there coming out of
  `parity` and moving neither of their other two),
  `corpus:compare:parse --all` (minimum per-language `compared`
  + EXACT per-language tsv-side parse-failure counts), `corpus:compare:format
  --all` (minimum per-language `match` + EXACT per-language `unknown`/`partial`
  counts — the un-triaged divergence backlog is pinned, so a new unexplained
  divergence fails until fixed/cataloged and a shrink is re-pinned to record the
  win), and the five harvests (wpt block count, test262 positive count, the
  ts-repo corpus + rejects counts, svelte-rejects count — exact; svelte-styles block
  count — a live-corpus MINIMUM with a drift band, since its source is the perf-view
  dev repos: a small shrink warns and still writes, only a >10% collapse fails), plus
  the CSS reject count `diagnostics/css_over_acceptance.ts` grades over — derived
  live from pinned inputs rather than harvested, and the one pin whose list filters
  nothing (see `CSS_REJECTS_PIN`), but stamped and graded on the harvests' cadence
  all the same (`css:over-acceptance:pin`). The
  ts-repo pair is graded by tsc itself, so its harvest stamps the **tsc version**
  alongside the checkout commit — a tsc bump can move a file between the two lists
  with the corpus unchanged.

  **The suite-derived pins have exactly one cadence.** Every count above that is
  measured over a sibling checkout (the four harvests' plus the CSS reject count) is
  re-derived by `deno task bench:pins:suites` and by nothing in `deno task
  check`, which never touches a checkout — so a checkout that moves leaves the pin
  describing the previous corpus with every committed-tree gate green until that
  group runs. Two things make that safe rather than merely documented: the group is
  a preflight of `deno task conformance` (so a release cannot ship the old number),
  and each leg is freshness-stamped on the checkout COMMIT — every checkout it
  reads, which for the CSS pin is three (`../svelte`, `../prettier`, `../wpt`) — so
  a move between upstream releases, where `pins:audit`'s version check sees nothing,
  still re-grades. A count pin is never a substitute for a commit in a stamp: an
  edit to an existing suite file moves the corpus without moving the count. Two of
  the pins are also graded a second time on their own surface:
  `TEST262_POSITIVES_PIN` by `conformance:test262` (its Rust twin) and
  `CSS_REJECTS_PIN` by the conformance coverage run (`bench:conformance`), whose
  oracle row's `parse/css` skips are the reject set. `deno task doctor` reports a
  stamp whose recorded commit is behind its checkout.
- **Rust-side counts are consts** — grep `REGRESSION PIN`. test262 (discovered +
  graded-manifest) and `fixtures_validate` (total fixtures — protecting the primary
  gate against a discovery collapse) live in their own commands, while the
  as-authored audits (`swallow_audit`, `fabrication_audit`, `census_audit`,
  `width_audit`, `comment_audit`) share `FIXTURES_FORMATTED_MIN` in
  `crates/tsv_debug/src/audit/vacuity.rs` — formatted files, closing their
  vacuous-pass. One const because they walk one corpus under one skip policy;
  separate consts would drift apart in slack, which is the collapse the pin exists
  to catch — `comment_audit`'s own `REGISTERED_MIN` had drifted 27% below its live
  count before it was made to pass the shared pin too. (`razor_audit` reuses the
  const as its floor over a seed list it resolves itself, `.svelte` files only, so
  it carries structural slack — the `.svelte` subset sits above the corpus-wide
  pin, and a Svelte-only discovery collapse smaller than that slack would pass; a
  razor-scoped pin is the tightening if that ever bites.) That is the **default-corpus**
  layer; under it sits `check_graded_nonzero` (same module), which every
  corpus-walking audit calls unconditionally on its own graded count and which
  therefore needs no pin at all.
- **`tsc_conformance`** (the largest set) splits its pins by what they mean, and
  gates the ON-DEMAND experimental-typechecker tasks, not a release leg. The
  drifting tsv-side counts (denominators, parse-divergence census, family
  partitions, carve-outs) live in the machine-regenerated snapshot
  `crates/tsv_debug/src/cli/commands/tsc_conformance_pins.txt`, rewritten by `deno
  task conformance:tsc-check:update`. The oracle-side pins (baseline / roundtrip /
  pretty + the `INDEX_*` denominators) and the semantically-zero invariant gates
  stay hand-edited consts in `cli/commands/tsc_conformance/pins.rs`; the
  crash-exclusion count sits beside its ledger in
  `tsc_conformance/runner/grade.rs`. Re-pin ritual: [typechecker.md](typechecker.md).

## Semantics — three pin categories, chosen per surface

- **Exact pins** (mismatch in either direction fails): the fixtures gates, ts-repo,
  test262, and the harvests. Their inputs are pinned checkouts (version-gated by
  `pins:audit`) or deliberately-updated ones, so the counts are deterministic — a
  drop is a regression or gutted input, a rise is a suite refresh or behavior
  change; both must be re-pinned deliberately. No slack: slack lets small
  regressions creep and silently widens after every refresh.
- **Minimums** (shrink fails, growth passes; carve-out:
  `SVELTE_STYLES_BLOCKS_MIN` warns on a small shrink and fails only on a >10%
  collapse, since it counts pure input material off daily-churning repos). Two
  cases, differing in WHY a minimum is right:
  1. `CORPUS_FORMAT_MATCH_MIN` is over the **reproducible** subset (pinned
     framework + prettier), so it's really exact-on-aligned-checkouts — the minimum
     exists only so a fixed win needn't re-pin; over pinned inputs a `match` DROP is
     always a real regression. ⚠️ It is NOT a live-growth minimum. The tempting
     framing — "the corpus is LIVE dev repos that GROW with ordinary work, so a
     minimum stays tight" — is FALSE, and is what drives a re-pin treadmill: a
     minimum is only sound if the metric can't decrease, but `match` **shrinks** the
     moment a live edit adds a divergence. That is why the format pins sit on the
     reproducible subset.
  2. `CORPUS_PARSE_COMPARED_MIN` and the committed-fixtures audits
     (`fixtures_validate`, `swallow_audit`) ARE genuine growth minimums —
     `compared`/fixture counts only grow with reviewed additions, and shrinkage is
     the discovery regression the pin guards.
- **Failure-bucket pins** (exact `!==`): the `corpus:compare:* --all` triage
  buckets. The **format** `unknown`/`partial` pins are over the **reproducible**
  subset (deterministic on aligned checkouts — live dev-repo divergences are a
  non-gating WARN); the **parse** tsv-side parse-failure pin stays over the live
  corpus (a tsv over-rejection of real code is a regression wherever it occurs). A
  rise fails until triaged (fix it, add a divergence detector/sanction, or
  consciously re-pin a legitimately-unsupported new file); a drop also fails, so a
  fixed divergence ratchets the pin DOWN deliberately.

**SAFETY always gates** — content loss fails `corpus:compare:format --all` over
EVERY file, reproducible or live. Data loss is never churn; the reproducibility
split is only about the layout/count pins.

Pins apply only to FULL runs (default suite root, `--all`, default harvest source) —
subtree and filtered runs legitimately grade a slice. Harvest pins fail **before**
writing, so a wrong cache never replaces a good one (the `SVELTE_STYLES_BLOCKS_MIN`
drift band still holds this: only a collapse fails-before-writing; a small shrink
warns and writes valid data). CI runs only the committed-tree pins (`check.yml` is a
clean checkout — no sibling clones); the rest are dev-machine gates at
conformance/publish cadence.

## Update ritual

Same as the artifact size bounds: the failure message prints expected vs got —
update the constant, and record **what moved and why** beside it as an `X → Y:` note
in the neighbours' style: which file entered or left, in which direction, whether it
left by getting better or worse, and what the A/B over the full corpus showed. A bare
number is unauditable — the next re-pin cannot tell a recorded win from an absorbed
regression — so the attribution is the constant's semantics, not its history.

⚠️ **Attribution, not a changelog.** Dates, PR numbers and commit SHAs stay out (the
`X→Y (date): …` entries were swept out for that reason, and the repo-wide rule against
process notes in docs and comments applies here); the change's own narrative belongs in
the **commit message**. What stays in-file is the sentence a reader needs to know what
the number counts — including a harvest pin's provenance stamp (`Measured <date>:
../wpt at <commit>`), which is the pin's identity, not its history.

When a checkout moves, re-record its **commit** in `GATE_CHECKOUT_COMMITS` in the
same change (`git -C ../<repo> rev-parse --short HEAD`) — that struct is the single
provenance record for what a pin was measured against (upstream version files only
bump at release) — and run `deno task bench:pins:suites` there too, so the
suite-derived pins move with it. Each entry's `pins` list is graded by
`benches/js/lib/gate_counts_test.ts` (in `test:deno`): every exported pin must be
named by some checkout (or by the test's `UNTRACKED_PINS`, with a reason), and every
name must exist — a new pin cannot land without its provenance, and a rename cannot
leave a ghost.

When re-pinning after a suite refresh, glance at the full bucket table, not just the
changed number — a count move can mask offsetting changes. (The per-file gates —
unexpected over-rejections, stale ledgers, SAFETY — catch tsv-side regressions
independently, but the glance is cheap.) Never re-pin to absorb an unexplained move:
that is the regression the pin exists to catch.

**Attribute the move before you re-pin, even when the cause is archaeological.** A
deliberate change explains itself; a move nobody can name still can be attributed by
re-running an older tree — `git archive` it into a scratch checkout, point it at the
*same* oracle checkouts, and diff the **per-file bucket sets**, not the totals. Which
files changed bucket, and in which direction, is the attribution; totals can offset and
say nothing. ⚠️ `git archive` stamps commit-time mtimes, so a scratch tree sharing a
`CARGO_TARGET_DIR` with the live one reuses the newer binary and measures nothing —
`touch` the extracted tree, or give it its own target dir, and confirm the binary is
actually fresh rather than trusting cargo's exit code.

**Re-measure at re-pin time.** The corpus legs read live working trees, so their counts
drift with ordinary work in the scanned repos and a number cited from an earlier run is
already a guess. Take the reading in the same change that writes the constant, and
re-pin as one deliberate corpus refresh — the checkout commits, the counts, and the
attribution note landing together.

## Why both the pins AND the checkout alignment exist

They guard different granularities. Checkout alignment
([`pins:audit:checkouts`](audits.md#checkout-alignment-audit-pinsauditcheckouts))
compares `package.json` versions — but an upstream repo's version only bumps at
release, so commits landing between releases change the SUITE without changing the
version (proven on day one: a `../svelte` pull added one test fixture at the same
declared version — the count pin caught it; the version check couldn't).
Conversely the count pins can't tell one release from another if the counts happen
to coincide. Version alignment catches release-level skew; count pins catch
commit-level suite drift within a version window.
