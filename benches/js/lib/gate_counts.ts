/**
 * Pinned gate counts — committed EXPECTED numbers for the diagnostic gates and
 * harvests, so a change in what gets graded (a gutted or refreshed suite
 * checkout, a discovery bug, a tsv behavior change, a systemic sidecar/FFI
 * failure eating a whole language) fails loudly instead of shifting inside a
 * green run. This is `scripts/validate_artifacts.ts`'s tight-bounds philosophy
 * applied to counts: every real move in a number is a deliberate, visible edit.
 *
 * Three pin categories, chosen per surface:
 *
 * - **Exact pins** (`*_PINS` / `*_PIN`) — surfaces whose inputs are pinned or
 *   committed: the fixtures gates and harvests (suite checkouts version-gated
 *   by `deno task pins:audit`) and ts-repo/test262/wpt (checkouts updated
 *   deliberately). Any mismatch — up or down — fails. No slack: slack lets
 *   small regressions creep and silently widens after every refresh.
 * - **Minimums** (`*_MIN`) — success counts. Two flavors: the FORMAT `match`
 *   minimum (`CORPUS_FORMAT_MATCH_MIN`) is over the REPRODUCIBLE subset (pinned
 *   framework + prettier suites), so it's exact-on-aligned-checkouts — the
 *   minimum is only there so a fixed win needn't re-pin; over pinned inputs a
 *   drop is always a real regression. The PARSE `compared` minimum
 *   (`CORPUS_PARSE_COMPARED_MIN`) and the committed-fixtures audits stay
 *   genuine live-growth minimums (dev repos / reviewed fixture diffs grow, so
 *   growth passes, a drop fails) — except `SVELTE_STYLES_BLOCKS_MIN`, which
 *   counts pure input material off daily-churning repos, so a small drop only
 *   warns and only a >10% collapse fails (see its comment).
 * - **Failure-bucket pins** (`*_PIN`, exact two-sided `!==`): the triage buckets
 *   on `corpus:compare:* --all`. The FORMAT `unknown`/`partial` pins are over
 *   the REPRODUCIBLE subset (deterministic on aligned checkouts — the live dev
 *   repos are a non-gating WARN); the PARSE tsv-side parse-failure pin stays over
 *   the live corpus (a tsv over-rejection of real code is a regression wherever
 *   it occurs). A rise fails until triaged — fix it, add a divergence
 *   detector/sanction, or consciously re-pin (a legitimately-unsupported new
 *   file); a drop also fails, so the pin ratchets DOWN deliberately and wins
 *   stay recorded. **SAFETY (content loss) always gates over EVERY file,
 *   reproducible or live — data loss is never churn.**
 *
 * Pins are enforced only on FULL runs (default suite root, `--all`, default
 * harvest source) — a subtree or filtered run legitimately grades a slice.
 * Harvest pins fail BEFORE writing, so a wrong cache never replaces a good
 * one (the `SVELTE_STYLES_BLOCKS_MIN` drift band still holds this: only a
 * collapse fails-before-writing; a small shrink warns and writes valid data).
 * CI note: `.github/workflows/check.yml` runs on a clean checkout (no
 * sibling clones), so of these only the committed-tree Rust pins
 * (fixtures_validate via the integration test, swallow_audit) execute in CI —
 * the rest are dev-machine gates at conformance/publish cadence.
 *
 * Update ritual: the failure message prints expected vs got — update the
 * constant and say why in the COMMIT MESSAGE (that is where a pin move's
 * history lives — do NOT narrate it as an in-file comment; keep these
 * docstrings semantic). When a checkout moves, re-record its commit in
 * `GATE_CHECKOUT_COMMITS` in the same change (`git -C ../<repo> rev-parse
 * --short HEAD`) — that struct is the single provenance record for what a pin
 * was measured against. When re-pinning after a suite refresh, glance at the
 * full bucket table, not
 * just the changed number — a count move can mask offsetting changes (the
 * per-file gates — unexpected over-rejections, stale ledgers, SAFETY — catch
 * tsv-side regressions independently, but the glance is cheap). A
 * failure-bucket-pin trip on a single `--all` run can be the known FFI/sidecar
 * heisenbug (see
 * benches/js/CLAUDE.md §Known Issues) — confirm on the single repo before
 * treating it as real. Never re-pin to absorb an unexplained move.
 *
 * The Rust-side pins (test262 discovery + graded manifest, `fixtures_validate`
 * fixture count, `swallow_audit` formatted-file count) live as consts in their
 * commands — grep `REGRESSION PIN`. See benches/js/CLAUDE.md §Pinned gate
 * counts.
 */

import type { Language } from './types.ts';

/**
 * The sibling checkouts the counts below were measured against, by git commit.
 *
 * The counts are only meaningful relative to the inputs that produced them, and an
 * upstream `package.json` version bumps only at RELEASE — so commits landing between
 * releases change the graded suite without changing the version. `pins:audit`'s version
 * check is blind to that window, which is exactly how these pins went stale silently: a
 * `../svelte` pull added three test inputs at the same declared `5.56.4`, and `../kit` +
 * `../svelte.dev` moved under the corpus pins with no version signal at all.
 *
 * So `pins:audit` also compares each checkout's HEAD against the commit recorded here and
 * WARNS on a move. That is deliberately a warning, not a failure: the count pins are the
 * gate (they fail on any real move in what's graded), and this exists to make a count-pin
 * trip *diagnosable* — "the corpus moved" vs "tsv regressed" is otherwise a reverse-
 * engineering exercise. An absent checkout, or one that isn't a git repo, is skipped, so
 * clean machines and CI still pass.
 *
 * Re-record a commit in the same change that re-pins the counts it explains.
 */
export const GATE_CHECKOUT_COMMITS: Record<string, { commit: string; pins: string }> = {
	'../svelte': { commit: 'b4d1583ae', pins: 'SVELTE_FIXTURES_PINS, CORPUS_FORMAT_*, CORPUS_PARSE_*' },
	'../acorn-typescript': { commit: '312d079', pins: 'TS_FIXTURES_PINS' },
	'../typescript': { commit: '637d5746b', pins: 'TS_REPO_PINS' },
	'../kit': { commit: 'da5b08ea7', pins: 'CORPUS_FORMAT_*, CORPUS_PARSE_*' },
	'../svelte.dev': { commit: 'c21c2d0f0', pins: 'CORPUS_FORMAT_*, CORPUS_PARSE_*' },
	'../prettier': { commit: '1dcd0b05d', pins: 'CORPUS_FORMAT_*, CORPUS_PARSE_*' },
	'../prettier-plugin-svelte': { commit: '7809486', pins: 'CORPUS_FORMAT_*, CORPUS_PARSE_*' },
};

/** Exact expected counts for a fixtures parse-conformance gate (`lib/fixtures_gate.ts`). */
export interface GatePins {
	/** Suite inputs discovered under the default root. */
	scanned: number;
	/** Both-accept count — also catches an oracle collapse (everything "parity") that `scanned` can't see. */
	both_accept: number;
}

/** conformance:svelte-fixtures — `scanned` suite inputs + `both_accept`; provenance in `GATE_CHECKOUT_COMMITS`. */
export const SVELTE_FIXTURES_PINS: GatePins = { scanned: 3378, both_accept: 3283 };

/** conformance:ts-fixtures — provenance in `GATE_CHECKOUT_COMMITS` (../acorn-typescript, oracle @sveltejs/acorn-typescript). */
export const TS_FIXTURES_PINS: GatePins = { scanned: 207, both_accept: 186 };

/**
 * conformance:ts-repo — `scanned` corpus files + `accept_parity` (tsv/tsc-baseline agreement);
 * provenance in `GATE_CHECKOUT_COMMITS` (../typescript). A rise on the pinned corpus is a parity
 * gain, not a suite refresh; a drop is a regression.
 */
export const TS_REPO_PINS = { scanned: 768, accept_parity: 429 };

/**
 * corpus:compare:parse --all — MINIMUM per-language `compared` (both sides
 * parsed and the ASTs diffed); the corpus is live dev repos, so growth passes
 * and any drop fails.
 */
export const CORPUS_PARSE_COMPARED_MIN: Record<Language, number> = {
	svelte: 1371,
	typescript: 4356,
	css: 168,
};

/**
 * corpus:compare:parse --all — EXACT per-language tsv-side parse-failure
 * count. Up = tsv newly rejects real corpus code (a drop-in regression — or a
 * legitimately-unsupported new corpus file: triage with
 * `diagnostics/skip_triage.ts`, then re-pin consciously). Down = a parse gap
 * closed; re-pin so the win stays recorded.
 */
export const CORPUS_PARSE_TSV_ERRORS_PIN: Record<Language, number> = {
	svelte: 0,
	typescript: 9,
	css: 3,
};

/**
 * corpus:compare:format --all — per-language MINIMUM exact-`match` count, enforced over
 * the REPRODUCIBLE subset only: the version-pinned `framework` + `prettier_fixture` tiers
 * (../kit, ../svelte, ../svelte.dev, ../prettier, ../prettier-plugin-svelte — the checkouts
 * `GATE_CHECKOUT_COMMITS` tracks and `pins:audit` verifies). The live dev repos are a
 * NON-GATING WARN (`corpus_compare_format.ts`), so their churn never shifts a pin — an
 * aligned machine measures these EXACTLY. A shrink fails (a formatter/oracle collapse in
 * pinned code); a rise re-pins to keep the floor tight. It stays a minimum (not exact) only
 * so a fixed win needn't re-pin to pass — over pinned inputs a `match` DROP is always a real
 * regression, never live-corpus growth. Provenance in `GATE_CHECKOUT_COMMITS`; split rationale
 * in benches/js/CLAUDE.md §Pinned gate counts.
 */
export const CORPUS_FORMAT_MATCH_MIN: Record<Language, number> = {
	svelte: 514,
	typescript: 2332,
	css: 90,
};

/**
 * corpus:compare:format --all — EXACT per-language `unknown` divergence count over the
 * REPRODUCIBLE subset (framework + prettier suites; see `CORPUS_FORMAT_MATCH_MIN`). Both
 * directions fail: a rise = a new unexplained divergence (fix it, catalog a detector in
 * `lib/divergence/patterns.ts`, or consciously re-pin a legitimately-unsupported new pinned
 * suite file); a drop = the backlog shrank, re-pin to record the win. Live dev-repo unknowns
 * are the non-gating WARN, not here. A single-run trip can be the FFI/sidecar heisenbug —
 * confirm on the single repo first. Same reproducible subset + provenance as
 * `CORPUS_FORMAT_MATCH_MIN`.
 */
export const CORPUS_FORMAT_UNKNOWN_PIN: Record<Language, number> = {
	svelte: 6,
	typescript: 117,
	css: 22,
};

/**
 * corpus:compare:format --all — EXACT per-language `partial` divergence count over the
 * REPRODUCIBLE subset (same semantics as `CORPUS_FORMAT_UNKNOWN_PIN`). svelte is 0 because
 * all 5 live svelte partials — the fuz fill-family `.svelte` pages — are in the non-gating
 * WARN, not the gate.
 */
export const CORPUS_FORMAT_PARTIAL_PIN: Record<Language, number> = {
	svelte: 1,
	typescript: 44,
	css: 9,
};

/**
 * bench:harvest:svelte-styles — MINIMUM extracted `<style>` block count. Live
 * corpus like `CORPUS_PARSE_COMPARED_MIN`, but with a DRIFT BAND: the perf-view
 * source is the author's own daily-churning repos and the count is pure input
 * material (not a tsv success count), so an ordinary refactor dropping a
 * `<style>` block is benign — unlike the other minimums, a small shrink here
 * isn't a regression. Growth always passes; a shrink within 10% of the pin WARNS
 * and still writes (re-pin here when convenient to silence it); only a COLLAPSE
 * below 90% — broken extraction or a gutted corpus — fails before the cache is
 * written. The harvest owns that band (`* 0.9`); this stays the exact measured
 * value.
 */
export const SVELTE_STYLES_BLOCKS_MIN = 264;

/** bench:harvest:wpt — exact `<style>` blocks from the default `../wpt/css`. Measured 2026-07-06: ../wpt at 7437c7bc. */
export const WPT_CSS_HARVEST_PIN = 22_310;

/**
 * bench:harvest:test262 — exact expected-positive files in the cache list. Measured 2026-07-06: ../test262 at 7153986f (46,544 graded).
 * Mirrors the Rust `POSITIVE_PASSED_PIN` (crates/tsv_debug/src/cli/commands/test262.rs) that the
 * `conformance:test262` release gate enforces — same positive count, keep the two in lockstep on a test262 pull.
 */
export const TEST262_POSITIVES_PIN = 42_113;

/**
 * bench:harvest:svelte-rejects — exact reject count. Measured 2026-07-06:
 * ../svelte at 8fb7ceeba, oracle svelte@5.56.4 (142 of 5648 conformance-view
 * Svelte files, re-verified after the ryanatkn.com + webdevladder.net + mdz
 * corpus additions — all their Svelte is valid). Fewer = the svelte/compiler oracle stopped rejecting (broken
 * import/config); more = it started rejecting wholesale — either way the cache
 * would corrupt the published coverage number.
 */
export const SVELTE_REJECTS_PIN = 142;
