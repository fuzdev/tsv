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
 * - **Minimums** (`*_MIN`) — success counts on the live `corpus:compare:*
 *   --all` corpus (dev repos that grow with ordinary work) and the
 *   committed-fixtures audits (additions are ordinary reviewed diffs). Growth
 *   passes; any drop below the pinned current value fails — except
 *   `SVELTE_STYLES_BLOCKS_MIN`, which counts pure input material off
 *   daily-churning repos, so a small drop only warns and only a >10% collapse
 *   fails (see its comment).
 * - **Failure-bucket pins** (`*_PIN`, exact — the same two-sided `!==` as the
 *   exact pins above, but on the live corpus rather than a deterministic
 *   input): the triage buckets on `corpus:compare:* --all` (unknown/partial
 *   divergences, tsv-side parse failures). A rise fails until triaged — fix
 *   it, add a divergence detector/sanction, or consciously re-pin (a
 *   legitimately-unsupported new corpus file); a drop (bucket shrank —
 *   divergences fixed) also fails, so the pin ratchets DOWN deliberately and
 *   wins stay recorded.
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
 * constant + its measured-on comment (INCLUDING the checkout commit, `git -C
 * ../<repo> rev-parse --short HEAD` — upstream version files only bump at
 * release, so the commit is the only precise statement of what a pin was
 * measured against) in the same change, and say why in the commit message.
 * When re-pinning after a suite refresh, glance at the full bucket table, not
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

/** Exact expected counts for a fixtures parse-conformance gate (`lib/fixtures_gate.ts`). */
export interface GatePins {
	/** Suite inputs discovered under the default root. */
	scanned: number;
	/** Both-accept count — also catches an oracle collapse (everything "parity") that `scanned` can't see. */
	both_accept: number;
}

/**
 * conformance:svelte-fixtures — measured 2026-07-07: ../svelte at 8fb7ceeba (svelte@5.56.4-3), oracle svelte@5.56.4.
 * both_accept 3276→3277: legacy `<!-- -->` HTML comments (CDO/CDC) in `<style>` now parse (css-cdo-cdc gap
 * fixed — parseCss swallows the marker span; tsv matches), so `css/samples/comment-html` moved from
 * over-rejection to both-accept.
 * both_accept 3274→3276: whitespace after `{` before a block/tag marker (`{ #if}`) now parses
 * (svelte-block-ws gap fixed — the `if-block-whitespace-{legacy,runes}` pair moves to both-accept).
 * both_accept 3273→3274: leading-symbol attribute names (`<p }>`) now parse (svelte-attr-name gap fixed),
 * so `validator/samples/attribute-invalid-name` moved from over-rejection to both-accept.
 */
export const SVELTE_FIXTURES_PINS: GatePins = { scanned: 3375, both_accept: 3277 };

/** conformance:ts-fixtures — measured 2026-07-08: ../acorn-typescript at 312d079 (v1.0.11), oracle @sveltejs/acorn-typescript@1.0.11. */
export const TS_FIXTURES_PINS: GatePins = { scanned: 207, both_accept: 186 };

/** conformance:ts-repo — measured 2026-07-06: ../typescript at 637d5746b. */
export const TS_REPO_PINS = { scanned: 768, accept_parity: 424 };

/**
 * corpus:compare:parse --all — MINIMUM per-language `compared` (both sides
 * parsed and the ASTs diffed); the corpus is live dev repos, so growth passes
 * and any drop fails. Measured 2026-07-07 after admitting `.d.ts` files and
 * the curated-entry `/build/` dirs to the corpus (typescript 4250→4356; svelte
 * and css unchanged). Oracle svelte@5.56.4.
 */
export const CORPUS_PARSE_COMPARED_MIN: Record<Language, number> = {
	svelte: 1372,
	typescript: 4356,
	css: 168,
};

/**
 * corpus:compare:parse --all — EXACT per-language tsv-side parse-failure
 * count. Up = tsv newly rejects real corpus code (a drop-in regression — or a
 * legitimately-unsupported new corpus file: triage with
 * `diagnostics/skip_triage.ts`, then re-pin consciously). Down = a parse gap
 * closed; re-pin so the win stays recorded. Measured 2026-07-08 on the merged
 * tree: main's ryanatkn.com + webdevladder.net + mdz + `.d.ts`/`/build/` corpus
 * additions (all parse clean) plus the acorn-typescript 1.0.11 upgrade.
 * typescript 16→9 as parser over-rejections closed. svelte 1→0: the merged
 * css-cdo-cdc / svelte-block-ws / svelte-attr-name parser fixes moved the last
 * over-rejected corpus file to both-accept.
 */
export const CORPUS_PARSE_TSV_ERRORS_PIN: Record<Language, number> = {
	svelte: 0,
	typescript: 9,
	css: 5,
};

/**
 * corpus:compare:format --all — MINIMUM per-language exact `match` count (same
 * live-corpus semantics as `CORPUS_PARSE_COMPARED_MIN`). Measured 2026-07-07
 * after the `.d.ts` corpus admission (typescript 3983→4086; svelte/css unchanged).
 */
export const CORPUS_FORMAT_MATCH_MIN: Record<Language, number> = {
	svelte: 1111,
	typescript: 4086,
	css: 126,
};

/**
 * corpus:compare:format --all — EXACT per-language `unknown` + `partial`
 * divergence counts (the un-triaged surface the WARN line reports). Up = a new
 * unexplained divergence landed — fix it, catalog a detector
 * (`lib/divergence/patterns.ts`), or consciously re-pin for a new corpus file.
 * Down = the backlog shrank; re-pin to record it. A single-run trip can be the
 * FFI/sidecar heisenbug — confirm on the single repo first. Measured
 * 2026-07-06 after adding the ryanatkn.com + webdevladder.net + mdz entries.
 * svelte 7→8: +1 = tsv.fuz.dev BenchmarksCrossRuntime.svelte, the sanctioned
 * inline-element expansion class (render-safe; flagged unknown only because no
 * detector pattern claims it yet). typescript 180→179: +1 mdz
 * mdz.benchmark.ts (needs-triage class, recorded in internal notes —
 * param-list explosion before an unbreakable-overflow arrow body, where
 * prettier keeps the params inline and accepts the overflow), −2 live-corpus
 * drift (same-day svelte.dev worktree reset + tsv.fuz.dev wip churn). The
 * svelte partial 3→4 below is mdz's docs introduction page (fill-boundary
 * family, residual hunks unexplained — pattern-extension candidate).
 *
 * typescript unknown 179→184 + partial 63→65 (2026-07-07): the `.d.ts` corpus
 * admission — all 7 new entries are declaration files, 0 errors, 0 SAFETY.
 * Unknown +5: svelte/src/index.d.ts (the tracked `?`-position conditional-type
 * comment merge bug), kit/src/types/private.d.ts (the tracked union `{}`
 * spacing bug), kit/src/runtime/app/paths/types.d.ts +
 * svelte/src/ambient.d.ts + svelte/src/server/index.d.ts (the needs-triage
 * generic-param / conditional-type break-strategy classes). Partial +2:
 * kit/src/exports/public.d.ts + kit/src/types/internal.d.ts
 * (tabs_only_alignment family, residual hunks unexplained).
 */
export const CORPUS_FORMAT_UNKNOWN_PIN: Record<Language, number> = {
	svelte: 8,
	typescript: 184,
	css: 23,
};
export const CORPUS_FORMAT_PARTIAL_PIN: Record<Language, number> = {
	svelte: 4,
	typescript: 65,
	css: 8,
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
 * value. Measured 2026-07-07: 265→264 as ../tsv.fuz.dev (bb01070) refactored the
 * `.legend` `<style>` block out of the benchmarks +page.svelte alongside its
 * section.
 */
export const SVELTE_STYLES_BLOCKS_MIN = 264;

/** bench:harvest:wpt — exact `<style>` blocks from the default `../wpt/css`. Measured 2026-07-06: ../wpt at 7437c7bc. */
export const WPT_CSS_HARVEST_PIN = 22_310;

/** bench:harvest:test262 — exact expected-positive files in the cache list. Measured 2026-07-06: ../test262 at 7153986f (46,544 graded). */
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
