# Pinned Gate Counts

> Every graded gate and harvest enforces a committed expected count, so a change
> in *what gets graded* fails loudly instead of shifting inside a green run.

A gutted or refreshed suite checkout, a discovery bug, a tsv behavior change, or a
systemic sidecar/FFI failure eating a whole language all move a count. This is
`scripts/validate_artifacts.ts`'s tight-bounds philosophy applied to counts: every
real move in a number is a deliberate, visible edit.

## Where the numbers live

- **`benches/js/lib/gate_counts.ts`** — every Deno-side count, one per consumer:
  - **ts-repo**: `scanned` + `accept_parity` + `over_acceptance_parser` +
    `over_acceptance_checker`, plus `units_scanned` + `units_accept_parity` for the
    `@filename` multi-file units, a population the single-file counts cannot see
    ([conformance_tsc.md](conformance_tsc.md)). The two over-acceptance halves pin the
    WIDENING axis the first two structurally cannot see — those fix only how many
    tsc-VALID files tsv accepts and leave the split of the rest free — and are split by
    tsc's LIVE parser rather than by the `TS1xxx` code range.
  - **the fixtures gates**: `scanned` + `both_accept` + `over_acceptance`, the last for
    the same reason — a new over-acceptance comes out of `parity` and moves neither of
    the other two.
  - **`strip:corpus`**: EXACT `files` graded + `refusals` (`STRIP_CORPUS_PINS`), over the
    strippable TypeScript of the `gates` view's snapshot tiers.
  - **the six harvests**, all exact: wpt block count, test262 positive count, the
    ts-repo corpus + rejects counts, svelte-rejects count, prettier-jsx count,
    svelte-styles block count (the last over the `../corpora` snapshot's perf view). The
    ts-repo pair is graded by tsc itself, so its harvest stamps the **tsc version**
    alongside the checkout commit — a tsc bump can move a file between the two lists
    with the corpus unchanged.
  - **the CSS reject count** `diagnostics/css_over_acceptance.ts` grades over — derived
    live from pinned inputs rather than harvested, and the one pin whose list filters
    nothing (see `CSS_REJECTS_PIN`), but stamped and graded on the harvests' cadence
    all the same (`css:over-acceptance:pin`).

  **The suite-derived pins have exactly one cadence.** Every count above measured over
  a sibling checkout (the five suite harvests' plus the CSS reject count) is re-derived
  by `deno task bench:pins:suites` and by nothing in `deno task check`, whose two
  sibling-checkout legs (`roundtrip:audit:prettier`, `discovery:audit`) re-derive no
  pin — so a checkout that moves leaves the pin describing the previous corpus with
  every committed-tree gate green until that group runs. (The svelte-styles block count
  is the one sibling-measured pin outside that group: the same
  stamp-and-fail-before-writing posture, re-derived by `deno task
  bench:harvest:svelte-styles`, which `conformance` chains later beside the corpus legs
  that read its cache.)

  Two things make that safe rather than merely documented: the group is a preflight of
  `deno task conformance` (so a release cannot ship the old number), and each leg is
  freshness-stamped on the git OBJECT — its HEAD commit, or for the `../corpora`
  snapshot its `collections/` tree id — of **every** checkout it reads. For the two
  reject pins that is three apiece, neither list the one the pin is named after (CSS:
  `../svelte`, `../prettier`, `../wpt`; Svelte: `../svelte`, `../prettier`,
  `../prettier-plugin-svelte`, since both prettier suites' `.html` is Svelte-language
  corpus) — so a move between upstream releases, where `pins:audit`'s version check
  sees nothing, still re-grades. A contributor left out of a stamp is the whole
  failure: its pull leaves the stamp reading fresh over a corpus that moved under it.

  A count pin is never a substitute for a commit in a stamp either: an edit to an
  existing suite file moves the corpus without moving the count. Nor is a checkout a
  substitute for the loader's own filters: the grades over the conformance view stamp a
  fingerprint of the filter modules too (`corpus_filter_fingerprint`), since a validity
  filter that drops or re-admits a file moves the count with nothing else moving.

  Two pins are re-DERIVED a second time on their own surface: `TEST262_POSITIVES_PIN`
  by `conformance:test262` (its Rust twin) and `CSS_REJECTS_PIN` by the conformance
  coverage run (`bench:conformance`), whose oracle row's `parse/css` skips are the
  reject set. Three more get a weaker cache-staleness check: `TS_REPO_REJECTS_PIN` from
  `ts_repo_over_acceptance.ts`, and `SVELTE_REJECTS_PIN` / `PRETTIER_JSX_PIN` from the
  conformance coverage run, which refuses to publish over an exclusion cache whose size
  is not its pin (`bench_preflight.ts` `enforce_exclusion_caches`). `deno task doctor` reports a
  stamp whose recorded checkout id is behind its checkout.
- **`benches/js/lib/corpus_format_pins.txt` + `corpus_parse_pins.txt`** — the two
  corpus gates' counts, one row per corpus SOURCE (a `../corpora` collection, a prettier
  suite, the `svelte_styles` harvest) and language, generated by the gate's own
  `--update` (`lib/corpus_pins.ts` holds the grammar):
  - **`corpus:compare:format --all`**: EXACT `match` / `known` / `partial` / `unknown` /
    `errors` / `expected_errors` per row — every bucket but SAFETY, which gates over every
    file on its own, so a passing row pins its file count too. The un-triaged divergence
    backlog is pinned, so a new unexplained divergence fails until fixed or cataloged, and
    a shrink is re-pinned to record the win.
  - **`corpus:compare:parse --all`**: EXACT `compared` + each parse-failure bucket
    (`tsv_errors`, `canonical_errors`, `both_errors`) per row.

  Per source rather than per language total, so placing a collection ADDS rows and moves
  none, and a failure names the collection it happened in. A row exists iff the run saw a
  file of that language from that source; a missing or extra row fails like a moved count.
- **Rust-side counts are consts** — grep `REGRESSION PIN`. test262 (discovered +
  graded-manifest + the `--gate` positive count, `POSITIVE_PASSED_PIN`), `fixtures_validate`
  (total fixtures — protecting the primary gate against a discovery collapse) and
  `variant_audit` (`VARIANTS_GRADED_MIN`) live in their own commands, while the
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
  layer; under it sits `check_graded_nonzero` (same module), which every corpus-walking
  audit calls unconditionally on its own graded count, so it needs no pin.
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
  test262, `strip:corpus`, and the harvests. Their inputs are pinned checkouts (version-gated by
  `pins:audit`) or deliberately-updated ones, so the counts are deterministic — a
  drop is a regression or gutted input, a rise is a suite refresh or behavior
  change; both must be re-pinned deliberately. No slack: slack lets small
  regressions creep and silently widens after every refresh.
- **Minimums** (shrink fails, growth passes): the committed-fixtures audits
  (`fixtures_validate`, `swallow_audit`) — genuine growth minimums, since fixture counts
  only grow with reviewed additions, and shrinkage is the discovery regression the pin
  guards. A minimum is only sound where the metric can't decrease except by regression.
- **Failure-bucket pins** (exact `!==`): every `corpus:compare:* --all` row, `match` and
  `compared` included, over the whole `gates` view (deterministic on aligned checkouts —
  every file comes from a pinned checkout, which is why the corpus is a snapshot rather
  than the author's working trees, whose ordinary edits would make the gate a re-pin
  treadmill). A rise in `partial` / `unknown` / `tsv_errors` fails until triaged (fix it,
  add a divergence detector/sanction, or consciously re-pin a legitimately-unsupported new
  file); a drop fails too, so a fix ratchets the pin deliberately — and an exact `match`
  sees a file arriving from `known` as well as one leaving. A snapshot refresh moves many
  rows at once and is re-pinned as one deliberate corpus move, beside the new
  `../corpora` `collections/` tree id in `GATE_CHECKOUT_IDS`.

**SAFETY always gates** — content loss fails `corpus:compare:format --all` over
EVERY file. Data loss is never churn.

Pins apply only to FULL runs (default suite root, `--all`, default harvest source) —
subtree and filtered runs legitimately grade a slice. Harvest pins fail **before**
writing, so a wrong cache never replaces a good one — except the wpt harvest, which writes
first and REMOVES the whole cache on a pin miss, so loaders see absent rather than
wrong-sized. CI runs only the committed-tree pins (`check.yml` is a clean checkout — no
sibling clones); the rest are dev-machine gates at conformance/publish cadence.

## Update ritual

For the hand-edited constants (`gate_counts.ts`, the Rust consts), the ritual is the
artifact size bounds': the failure message prints expected vs got — update the constant,
and record **what moved and why** beside it as an `X → Y:` note
in the neighbours' style: which file entered or left, in which direction, whether it
left by getting better or worse, and what the A/B over the full corpus showed. A bare
number is unauditable — the next re-pin cannot tell a recorded win from an absorbed
regression — so the attribution is the constant's semantics, not its history.

**The two corpus gates re-pin by regenerating, not by editing.** `deno task
corpus:compare:format --all --update` and `deno task corpus:compare:parse --all --update`
rewrite their pins file from the run and print every row that moved. `--update` refuses a
narrowed run, and any run that could write a wrong table: a source absent from it (an
un-harvested `svelte_styles` cache), a red verdict (format: SAFETY, an environment error;
parse: an undocumented group, a wire verdict mismatch, a `loc` definition violation), or
a per-source tally that disagrees with the run's own per-language stats. The file carries
no notes: its diff names which source and bucket moved, and the attribution — which files
changed bucket and why — goes in the commit message, measured as below. Never hand-edit a
pins file; a row a hand edit got wrong fails the next full run as a moved count. What is
in a bucket now is the run's to say, not a pin's: `--all --json` lists the
`unknown` / `partial` / `errors` files (`.unknown[].path` and so on).

⚠️ **Attribution, not a changelog.** Dates, branch names, PR numbers and commit SHAs
stay out (the repo-wide rule against process notes in docs and comments applies here);
the change's own narrative belongs in the **commit message**. What stays in-file is the
sentence a reader needs to know what the number counts — including a harvest pin's
provenance stamp (`Measured <date>: ../wpt at <commit>`), which is the pin's identity,
not its history.

**An entry whose number a corpus refresh has replaced is superseded, and goes with it.**
The chain of `X → Y` notes attributes the number as it stands: once a tier change or a
snapshot refresh moves the corpus wholesale, the moves inside the corpus it replaced
attribute nothing a reader can audit, and the surviving chain should start at the tier
the current value was measured in. A pin that enumerates its backlog states the CURRENT
membership — a list that later entries silently retire members from is worse than none,
since it reads as the answer to "what is in this bucket".

⚠️ **A merged number is a re-measurement, never a sum.** Two re-pins developed apart are
each measured against their own tree, so their deltas are only *predictive* of the merged
tip: verify the mover sets are disjoint per file, take the reading on the merged tree, and
record the chain as the sequential steps it ends up being. That verification is a
precondition for the entry, not a note to leave in it.

⚠️ **A staged-tree corpus A/B is blind to the suite files `.prettierignore` hides.**
`tsv format --list` honors `.gitignore` / `.prettierignore` and the prettier repo ignores
its own test fixtures, so a rig enumerating with `--list` sees hundreds fewer files than
the `gates` view holds — an A/B reading ZERO movers there is not evidence the gate
passes. Enumerate the suites with `find`, or diff the `--all --json` bucket lists.

When a checkout moves, re-record its **id** (its HEAD commit; for `../corpora`, the
`collections/` tree id) in `GATE_CHECKOUT_IDS` in the same change
(`git -C ../<repo> rev-parse --short HEAD`; for the snapshot,
`git -C ../corpora rev-parse --short HEAD:collections`) — that struct is the single
provenance record for what a pin was measured against (upstream version files only bump
at release) — and run `deno task bench:pins:suites` there too, so the suite-derived pins
move with it. Each entry's `pins` list is graded by
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

**Re-measure at re-pin time.** The corpus legs read pinned checkouts, but a number
cited from an earlier run on another machine is still a guess about this one's
alignment. Take the reading in the same change that writes the constant, and re-pin as
one deliberate corpus refresh — the checkout ids (the `../corpora` snapshot's
`collections/` tree id above all), the counts, and the attribution note landing together.

## Why both the pins AND the checkout alignment exist

They guard different granularities. Checkout alignment
([`pins:audit:checkouts`](audits.md#checkout-alignment-audit-pinsauditcheckouts))
compares `package.json` versions — but an upstream repo's version only bumps at
release, so commits between releases change the SUITE without changing the version. A
pull inside that window — test inputs added at the same declared version — moves what
is graded, and only the count pin can see it.
Conversely the count pins can't tell one release from another if the counts happen
to coincide. Version alignment catches release-level skew; count pins catch
commit-level suite drift within a version window.
