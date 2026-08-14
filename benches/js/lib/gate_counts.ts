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
 * fixture count) live as consts in their commands — grep `REGRESSION PIN`. The
 * as-authored audits' formatted-file count is one shared const,
 * `FIXTURES_FORMATTED_MIN` in `crates/tsv_debug/src/audit/sweep.rs`: they walk
 * one corpus under one skip policy, so a per-audit pin would only let their
 * slack drift apart. See docs/gate_counts.md.
 */

import type { Language } from './types.ts';

/**
 * The sibling checkouts the counts below were measured against, by git commit.
 *
 * The counts are only meaningful relative to the inputs that produced them, and an
 * upstream `package.json` version bumps only at RELEASE — so commits landing between
 * releases change the graded suite without changing the version. `pins:audit`'s version
 * check is blind to that window, which is exactly how these pins went stale silently: a
 * `../svelte` pull added three test inputs at the same declared version, and `../kit` +
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
	'../svelte': {
		commit: '20b341f10',
		pins: 'SVELTE_FIXTURES_PINS, CORPUS_FORMAT_*, CORPUS_PARSE_*'
	},
	'../acorn-typescript': { commit: '923b213', pins: 'TS_FIXTURES_PINS' },
	'../typescript': {
		commit: '637d5746b',
		pins: 'TS_REPO_PINS, TS_REPO_CORPUS_PIN, TS_REPO_REJECTS_PIN'
	},
	'../kit': { commit: 'b27c82f9c', pins: 'CORPUS_FORMAT_*, CORPUS_PARSE_*' },
	'../svelte.dev': { commit: '996bd63e4', pins: 'CORPUS_FORMAT_*, CORPUS_PARSE_*' },
	'../prettier': { commit: '1dcd0b05d', pins: 'CORPUS_FORMAT_*, CORPUS_PARSE_*' },
	'../prettier-plugin-svelte': { commit: '7809486', pins: 'CORPUS_FORMAT_*, CORPUS_PARSE_*' }
};

/** Exact expected counts for a fixtures parse-conformance gate (`lib/fixtures_gate.ts`). */
export interface GatePins {
	/** Suite inputs discovered under the default root. */
	scanned: number;
	/** Both-accept count — also catches an oracle collapse (everything "parity") that `scanned` can't see. */
	both_accept: number;
}

/** conformance:svelte-fixtures — `scanned` suite inputs + `both_accept`; provenance in `GATE_CHECKOUT_COMMITS`. */
export const SVELTE_FIXTURES_PINS: GatePins = { scanned: 3392, both_accept: 3297 };

/** conformance:ts-fixtures — provenance in `GATE_CHECKOUT_COMMITS` (../acorn-typescript, oracle @sveltejs/acorn-typescript). */
export const TS_FIXTURES_PINS: GatePins = { scanned: 226, both_accept: 202 };

/**
 * conformance:ts-repo — `scanned` corpus files + `accept_parity` (tsv/tsc-baseline agreement);
 * provenance in `GATE_CHECKOUT_COMMITS` (../typescript). A rise on the pinned corpus is a parity
 * gain, not a suite refresh; a drop is USUALLY a regression — but read the other buckets before
 * treating it as one, because `accept_parity` counts only the agreeing-ACCEPT half. A file leaving
 * for `parity reject` — tsv learning to refuse something tsc's baseline refuses too — drops this
 * number with agreement unchanged; only which side of it moved. `GAPS UNEXPECTED` staying 0 is the
 * reading that settles it, since that is the bucket a real over-rejection lands in.
 *
 * `scanned` includes 61 `.d.ts` cases, which this gate grades (`DECLARATIONS` in
 * `diagnostics/ts_repo_compare.ts` argues why, and why the bench harvest does not). They are also
 * where 29 of the over-acceptances come from — statements in an ambient context, tsc's TS1036: an
 * early error tsv defers by policy, not a gap.
 *
 * `over_acceptance` (tsv accepts, tsc's baseline says invalid) is pinned for the axis the other two
 * cannot see. `scanned` and `accept_parity` together fix how many files tsv accepts *among the
 * tsc-valid ones*; the reject / over-accept / beyond-acorn split of the remainder is free. So a
 * parser WIDENING — a fix that also starts accepting something tsc rejects — moves only this
 * number, and without a pin nothing anywhere reports it. That is the standing hazard of every
 * over-rejection fix: the new acceptance arrives unguarded. A rise here is not automatically wrong
 * (tsv defers early errors by policy), but it must be a decision, not a side effect.
 *
 * Two of them are `asyncDeclare_es{5,6}.ts` — `declare async function foo(): Promise<void>;`,
 * tsc's TS1040. Like the TS1036 group above it is an ambient-context early error tsv defers: tsc's
 * PARSER builds the signature with `[DeclareKeyword, AsyncKeyword]` and reports an empty
 * `parseDiagnostics`, so the TS1xxx code in the baseline is a CHECKER grammar error and this gate's
 * code-range heuristic reads it as a parser rejection it is not. `accept_parity` is unmoved by
 * design — a file tsc's baseline calls invalid was never in that bucket.
 */
export const TS_REPO_PINS = { scanned: 13708, accept_parity: 12284, over_acceptance: 488 };

/**
 * corpus:compare:parse --all — MINIMUM per-language `compared` (both sides
 * parsed and the ASTs diffed); the corpus is live dev repos, so growth passes
 * and any drop fails.
 */
export const CORPUS_PARSE_COMPARED_MIN: Record<Language, number> = {
	svelte: 1371,
	typescript: 4356,
	css: 168
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
	css: 3
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
 * in docs/gate_counts.md.
 */
export const CORPUS_FORMAT_MATCH_MIN: Record<Language, number> = {
	// 500 → 499: one reproducible file, `svelte.dev/.../lib/components/PageControls.svelte`,
	// where an inline element's content is a fill (`<Icon … /> Edit this page on GitHub`) and so
	// stops letting its render-free content boundary select the layout — the deliberate rule in
	// conformance_prettier_svelte.md §Svelte: Inline content block-style, and the only file the change
	// moves across the whole reproducible subset. The detector already explains it
	// (`inline_sibling_newline_flow` + `svelte_boundary_ws_trim`), so it lands in `known`, not
	// `unknown` — that count is unmoved.
	//
	// 499 → 498: one reproducible file,
	// `prettier-plugin-svelte/test/printer/samples/event-handler-comments.html`, whose whole
	// content is `on:click={// comment⏎() => {…}}` — a leading line comment in a braced head,
	// which now hangs its value one level in per conformance_prettier.md §Uniform
	// Forced-Continuation Indent (extended from the block heads to the whole braced family).
	// It is the only file the change moves across the reproducible subset: the one other
	// pinned `.svelte` carrying the shape, `svelte/.../parser-legacy/samples/javascript-comments`,
	// already diverged on the trailing comments prettier strips there. The
	// `forced_continuation_indent` detector was widened with the clause in the same change, so
	// it lands in `known` — the `unknown` count is unmoved.
	svelte: 498,
	// 2332 → 2334: two reproducible files, `prettier/tests/format/js/comments/11273.js` and
	// `.../trailing-jsdocs.js`, whose divergence was a container-end trailing comment RUN the
	// author glued onto one line and tsv split onto two. That run's separator now asks the
	// source (docs/comments.md §Trailing and dangling runs), so a glued pair keeps its line as
	// it does in prettier. Both files leave `known` — the count that moves against this one.
	typescript: 2334,
	css: 89
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
	svelte: 7,
	// 109 records a drop of five from 114, in two steps, each verified by diffing the
	// `unknown` lists before and after rather than by the count alone:
	//   -2  a binary operand of an `as`/`satisfies` cast takes prettier's continuation
	//       indent — clears `typescript/as/assignment2.ts` and
	//       `typescript/satisfies-operators/assignment.ts`, the two mirror files.
	//   -3  a ternary operand reached from one of prettier's `ancestorNameMap` value
	//       positions expands its parens — clears `typescript/as/ternary.ts`,
	//       `typescript/satisfies-operators/ternary.ts` and `typescript/ternaries/indent.ts`.
	// A third step takes it to 108: `js/empty-statement/body.js` leaves the bucket entirely,
	// its `with (a);` now refused as the sloppy-mode statement it is rather than misparsed as
	// a call, so the file lands in `errors`. The two `js/identifier/for-of/*.js` files left
	// the same round by MATCHING (`match` 4269 → 4271) once a `let` heading a for-in/of head
	// kept its parens.
	// `js/identifier/parentheses/let.js` moves to `errors` in that round too (its last line is
	// `with (let[0] = 1);`) but moves no pin: it was neither matching nor unknown before, which
	// is what the flat +2 on `match` says. Four pinned files now land in `errors` for the same
	// deliberate refusal, and `errors` is not itself pinned — so a real parse regression that
	// dropped files there would move no count. That hole is pre-existing, not new here.
	// Neither step added an unknown. `js/ternaries/indent-after-paren.js` stays unknown for
	// an unrelated pre-existing reason (a parenthesized ternary CALLEE takes the flat-paren
	// bare-callee path in `call_formatting.rs`, not the chain base), but shrank from
	// 107/126 differing lines to 61/92; its `diff_summary` names a representative hunk, not
	// a total, so that string growing is not the file getting worse.
	//
	// 108 → 109: `js/comments/jsdoc-nestled.js` arrives from `partial`, and it arrives by
	// getting BETTER (7 differing lines → 3). Its container-end glued runs now keep their
	// line, which cleared two of its three hunks; what is left is the one prettier feature
	// tsv does not implement — `mergeNestledJsdocComments`, which at PARSE time fuses two
	// **zero-gap** indentable block comments into one, so prettier prints `*//**` where tsv
	// prints `*/ /**`. That hunk is PRE-EXISTING and unmoved (it is the file's leading-comment
	// run, untouched here); it was simply never the file's only divergence before, so a
	// broader detector matched the file and it graded `partial`. tsv cannot borrow prettier's
	// fix — a parse-time merge would break the acorn AST contract — so nestling is a printer
	// question at every comment run, tracked separately rather than pinned as a detector here.
	//
	// 109 → 110: `js/comments-closure-typecast/styled-components.js` arrives from `known`,
	// and it too arrives by getting BETTER. The file is an own-line JSDoc cast on a
	// styled-components tagged template; the own-line-cast hang (break after `=`, comment and
	// cast indented one level) made tsv's head lines match prettier byte-for-byte where tsv
	// used to trail the cast comment on the `=` line with `(styled.div)` stranded at column 0
	// — the shape `comment_position` was claiming. What is left is two COMPOSED sanctioned
	// divergences in one file: the cast parens tsv preserves (`(styled.div)`; prettier strips
	// them) and the tagged-template body tsv keeps verbatim (prettier's
	// `embeddedLanguageFormatting` recognizes styled-components tags and reformats the
	// embedded CSS). Neither detector reaches the residue: `jsdoc_type_cast_parens` keys on
	// the same-line `*/ (` spelling, not the own-line-comment form, and
	// `template_embedded_verbatim` recognizes only the explicit language tags
	// (html/css/graphql/gql), not prettier's styled-components heuristic (`styled.div`,
	// `styled(Component)`, `keyframes`, `createGlobalStyle`). Both halves are cataloged
	// behavior, so the arrival is pinned; widening the detectors is a follow-up with its own
	// overmatch questions, not a precondition for the pin.
	//
	// 110 → 109: `js/preserve-line/argument-list.js` LEAVES the bucket — prettier's own
	// adversarial test for exactly this behavior, now matching byte-for-byte. tsv asked
	// prettier's `anyArgEmptyLine` only at the BOTTOM of its call dispatcher, below every
	// specialized layout's early return, so an author blank between arguments survived a
	// plain argument list and was silently eaten by each hug / expand-first / expand-last /
	// composition path; prettier asks it ABOVE them all and returns `allArgsBrokenOut()`.
	// Hoisting it as a decline conjunct is the whole change. A/B'd against a HEAD-equivalent
	// binary over the full corpus: this file is the ONLY move in any bucket — nothing was
	// added to `unknown`, and `partial` / `safety` / `errors` are byte-identical.
	typescript: 109,
	css: 23
};

/**
 * corpus:compare:format --all — EXACT per-language `partial` divergence count over the
 * REPRODUCIBLE subset (same semantics as `CORPUS_FORMAT_UNKNOWN_PIN`). svelte is 0 because
 * all 5 live svelte partials — the fuz fill-family `.svelte` pages — are in the non-gating
 * WARN, not the gate.
 */
export const CORPUS_FORMAT_PARTIAL_PIN: Record<Language, number> = {
	svelte: 1,
	// 37 → 34: the three `js/comments/between-head-and-body/*.js` files are `with`-statement
	// comment tests, so all three now land in `errors` rather than being partially compared
	// through a call-expression misparse.
	//
	// 34 → 33: `js/comments/jsdoc-nestled.js` leaves for `unknown` — not a loss but a
	// shrink, its remaining hunk too narrow for the detector that used to match it. The
	// reasoning is on `CORPUS_FORMAT_UNKNOWN_PIN`, which moves the other way in the same step.
	typescript: 33,
	css: 9
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
 * bench:harvest:ts-repo — exact size of the tsc-corpus VALID list: single-file
 * `.ts` under `../typescript/tests/cases/{conformance,compiler}` that both tsc's
 * parser and tsc's own `.errors.txt` baselines call well-formed. Measured
 * 2026-08-09: ../typescript at 637d5746b, oracle tsc 6.0.3, over 9,414 single-file
 * `.ts`. A move means a checkout pull, a tsc bump, or a grading change in
 * `harvest_ts_repo.ts` — re-pin deliberately, never absorb. The full bucket
 * breakdown is the harvest's own final line, not repeated here: a hand-copied
 * tally goes stale silently, which is the failure this pin exists to prevent.
 */
export const TS_REPO_CORPUS_PIN = 8_097;

/**
 * bench:harvest:ts-repo — exact size of the tsc-corpus REJECTS list (files tsc's
 * PARSER rejects), the corpus `diagnostics/ts_repo_over_acceptance.ts` grades
 * over. Same measurement as {@link TS_REPO_CORPUS_PIN}. Deliberately NOT a corpus
 * entry: accepting these is the failure, so folding them into a coverage
 * denominator would score permissiveness as fidelity.
 */
export const TS_REPO_REJECTS_PIN = 519;

/**
 * bench:harvest:svelte-rejects — exact reject count. Measured 2026-07-06:
 * ../svelte at 8fb7ceeba, oracle svelte@5.56.4 (142 of 5648 conformance-view
 * Svelte files, re-verified after the ryanatkn.com + webdevladder.net + mdz
 * corpus additions — all their Svelte is valid); re-harvested unchanged at
 * ../svelte 20b341f10, oracle svelte@5.56.9. Fewer = the svelte/compiler oracle stopped rejecting (broken
 * import/config); more = it started rejecting wholesale — either way the cache
 * would corrupt the published coverage number.
 */
export const SVELTE_REJECTS_PIN = 142;

/**
 * The conformance CSS corpus's REJECT count — files `svelte/compiler`'s `parseCss`
 * refuses — which `diagnostics/css_over_acceptance.ts` grades every `parse/css`
 * tool over. Deterministic given the pins that build the corpus: ../prettier's
 * checkout commit ({@link GATE_CHECKOUT_COMMITS}), {@link WPT_CSS_HARVEST_PIN},
 * and the svelte oracle version.
 *
 * Unlike {@link SVELTE_REJECTS_PIN} this list filters NOTHING — `parseCss` is not
 * a validity oracle in either direction (it accepts malformed CSS and rejects
 * valid modern CSS it doesn't implement), so excluding its rejects would drop
 * files tsv also fails and flatter tsv's own coverage. The list exists to give
 * the CSS surface the over-acceptance axis coverage can't show, and the pin is
 * what makes the reference row's grammar moving VISIBLE instead of silently
 * reshaping the published `parse/css` numbers.
 */
export const CSS_REJECTS_PIN = 239;
