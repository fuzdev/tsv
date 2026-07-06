/**
 * Pinned gate counts — committed EXPECTED numbers for the diagnostic gates and
 * harvests, so a change in what gets graded (a gutted or refreshed suite
 * checkout, a discovery bug, a tsv behavior change, a systemic sidecar/FFI
 * failure) fails loudly instead of shifting inside a green run. This is
 * `scripts/validate_artifacts.ts`'s tight-bounds philosophy applied to counts:
 * every real move in a number is a deliberate, visible edit here.
 *
 * Two semantics, chosen per surface by whether its inputs are deterministic:
 *
 * - **Exact pins** (`*_PINS` / `*_PIN`) — surfaces whose inputs are pinned or
 *   committed: the fixtures gates and harvests (suite checkouts are
 *   version-gated by `deno task pins:audit`; `tests/fixtures` is committed) and
 *   ts-repo/test262/wpt (checkouts updated deliberately). Any mismatch — up or
 *   down — fails: a drop is a regression or a gutted input; a rise is a suite
 *   refresh or behavior change that must be re-pinned deliberately. No slack:
 *   slack would let small regressions creep, and after a suite refresh it
 *   silently widens.
 * - **Minimums** (`*_MIN`) — the one non-deterministic surface: the
 *   `corpus:compare:* --all` gates corpus includes LIVE dev repos (zzz, the fuz
 *   ecosystem, svelte/kit source) that grow with ordinary work. Growth passes
 *   silently; any drop below the pinned current value fails. Re-pin to the
 *   current value whenever you touch these (e.g. at release) so the minimum
 *   stays tight — a long-unpinned minimum slowly re-accumulates slack.
 *
 * Pins are enforced only on FULL runs (default suite root, `--all`, default
 * harvest source) — a subtree or filtered run legitimately grades a slice.
 * Harvest pins fail BEFORE writing, so a broken cache never replaces a good
 * one. CI note: `.github/workflows/check.yml` runs `deno task check` on a
 * clean checkout (no sibling clones), so of these only the committed-tree Rust
 * pins (fixtures_validate, swallow_audit) execute in CI — the rest are
 * dev-machine gates at conformance/publish cadence.
 *
 * Update ritual: when a pin fails after a deliberate change (canonical bump,
 * checkout refresh, tsv behavior change, corpus restructure), the failure
 * message prints expected vs got — update the constant + its measured-on
 * comment in the same change, and say why in the commit. Never re-pin to
 * absorb an unexplained move: that is the regression the pin exists to catch.
 *
 * The Rust-side pins (test262 discovery + graded manifest, `fixtures_validate`
 * fixture count, `swallow_audit` formatted-file count) live as consts in their
 * commands — grep `REGRESSION PIN`. See benches/js/CLAUDE.md §Pinned gate
 * counts.
 */

import type { Language } from './types.ts';

/** Exact expected counts for a fixtures parse-conformance gate (`lib/fixtures_gate.ts`). */
export interface GatePins {
	/** Suite inputs discovered under the default root. */
	scanned: number;
	/** Both-accept count — also catches an oracle collapse (everything "parity") that `scanned` can't see. */
	both_accept: number;
}

/**
 * conformance:svelte-fixtures — measured 2026-07-06 on the ../svelte checkout at
 * `svelte@5.56.4-3` (main; drifted past the pinned 5.56.1 — `pins:audit` fails
 * until it's aligned). ⚠ RE-PIN these when aligning the checkout to the pinned
 * tag; at a tag-aligned checkout they become stable.
 */
export const SVELTE_FIXTURES_PINS: GatePins = { scanned: 3375, both_accept: 3273 };

/** conformance:ts-fixtures — measured 2026-07-06 (../acorn-typescript at 1.0.10). */
export const TS_FIXTURES_PINS: GatePins = { scanned: 201, both_accept: 181 };

/** conformance:ts-repo — measured 2026-07-06 (../typescript checkout; re-pin on deliberate pulls). */
export const TS_REPO_PINS = { scanned: 768, accept_parity: 424 };

/**
 * corpus:compare:parse --all — MINIMUM per-language `compared` (both sides
 * parsed and the ASTs diffed); the corpus is live dev repos, so growth passes
 * and any drop fails. Pinned at the exact 2026-07-06 measured values — re-pin
 * to current when touching the corpus so the minimum stays tight.
 */
export const CORPUS_PARSE_COMPARED_MIN: Record<Language, number> = {
	svelte: 1213,
	typescript: 4174,
	css: 147,
};

/**
 * corpus:compare:format --all — MINIMUM per-language exact `match` count (same
 * live-corpus semantics as `CORPUS_PARSE_COMPARED_MIN`). Pinned at the exact
 * 2026-07-06 measured values.
 */
export const CORPUS_FORMAT_MATCH_MIN: Record<Language, number> = {
	svelte: 978,
	typescript: 3906,
	css: 107,
};

/** bench:harvest:wpt — exact `<style>` blocks from the default `../wpt/css`. Measured 2026-07-06. */
export const WPT_CSS_HARVEST_PIN = 22_310;

/** bench:harvest:test262 — exact expected-positive files in the cache list. Measured 2026-07-06 (of 46,544 graded). */
export const TEST262_POSITIVES_PIN = 42_113;

/**
 * bench:harvest:svelte-rejects — exact reject count. Measured 2026-07-06
 * (142 of 5488 conformance-view Svelte files). Exact and two-sided by nature:
 * fewer means the svelte/compiler oracle stopped rejecting (broken
 * import/config), more means it started rejecting wholesale — either way the
 * cache would corrupt the published coverage number.
 */
export const SVELTE_REJECTS_PIN = 142;
