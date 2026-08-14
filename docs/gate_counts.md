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
  nothing (see `CSS_REJECTS_PIN`). The
  ts-repo pair is graded by tsc itself, so its harvest stamps the **tsc version**
  alongside the checkout commit — a tsc bump can move a file between the two lists
  with the corpus unchanged.
- **Rust-side counts are consts** — grep `REGRESSION PIN`. test262 (discovered +
  graded-manifest) and `fixtures_validate` (total fixtures — protecting the primary
  gate against a discovery collapse) live in their own commands, while the
  as-authored audits (`swallow_audit`, `fabrication_audit`, `census_audit`,
  `width_audit`, `comment_audit`) share `FIXTURES_FORMATTED_MIN` in
  `crates/tsv_debug/src/audit/vacuity.rs` — formatted files, closing their
  vacuous-pass. One const because they walk one corpus under one skip policy; five
  would drift apart in slack, which is the collapse the pin exists to catch —
  `comment_audit`'s own `REGISTERED_MIN` had drifted 27% below its live count
  before it was made to pass the shared pin too. That is the **default-corpus**
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
update the constant and say why in the **commit message**. That is where a pin
move's history belongs; do NOT narrate it as an in-file comment — the `X→Y (date):
…` running changelogs were swept out, and `gate_counts.ts` docstrings stay semantic.

When a checkout moves, re-record its **commit** in `GATE_CHECKOUT_COMMITS` in the
same change (`git -C ../<repo> rev-parse --short HEAD`) — that struct is the single
provenance record for what a pin was measured against (upstream version files only
bump at release).

When re-pinning after a suite refresh, glance at the full bucket table, not just the
changed number — a count move can mask offsetting changes. (The per-file gates —
unexpected over-rejections, stale ledgers, SAFETY — catch tsv-side regressions
independently, but the glance is cheap.) Never re-pin to absorb an unexplained move:
that is the regression the pin exists to catch.

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
