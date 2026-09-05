/**
 * Pinned gate counts — committed EXPECTED numbers for the diagnostic gates and
 * harvests, so a change in what gets graded (a gutted or refreshed suite
 * checkout, a discovery bug, a tsv behavior change, a systemic sidecar/FFI
 * failure eating a whole language) fails loudly instead of shifting inside a
 * green run. This is `scripts/validate_artifacts.ts`'s tight-bounds philosophy
 * applied to counts: every real move in a number is a deliberate, visible edit.
 *
 * Three pin categories, chosen per surface — exact pins (`*_PINS` / `*_PIN`),
 * minimums (`*_MIN`), and failure-bucket pins (exact two-sided `!==`). What each means,
 * which surface takes which, and why (the pinned-snapshot rule, SAFETY gating over EVERY
 * file) is stated once in docs/gate_counts.md §Semantics; the per-constant docstrings
 * below carry only what is specific to that constant.
 *
 * Pins are enforced only on FULL runs (default suite root, `--all`, default harvest
 * source) — a subtree or filtered run legitimately grades a slice. Harvest pins fail
 * BEFORE writing, so a wrong cache never replaces a good one. `.github/workflows/check.yml`
 * runs on a clean checkout (no sibling clones), so of these only the committed-tree Rust
 * pins (fixtures_validate via the integration test, swallow_audit) execute in CI — the
 * rest are dev-machine gates at conformance/publish cadence.
 *
 * Update ritual — the full procedure is docs/gate_counts.md §Update ritual; what governs
 * THIS FILE is the shape of the note beside a constant. Record what moved as an `X → Y:`
 * attribution in the neighbours' style: which file entered or left, in which direction,
 * and what the A/B over the full corpus showed, so the next re-pin can tell a recorded win
 * from an absorbed regression. Attribution, not a changelog — dates, branch names, PR
 * numbers, commit SHAs and the change's own narrative belong in the COMMIT MESSAGE. An
 * entry whose number a corpus refresh has replaced is superseded and goes with it, and a
 * pin that enumerates its backlog states the CURRENT membership rather than a list later
 * entries correct. Re-record a moved checkout's id in `GATE_CHECKOUT_IDS` in the same
 * change — that struct is the single provenance record for what a pin was measured
 * against. Never re-pin to absorb an unexplained move; a failure-bucket-pin trip on a
 * single `--all` run can be the known FFI/sidecar heisenbug (benches/js/CLAUDE.md §Known
 * Issues), so confirm on the single repo before treating it as real.
 *
 * The Rust-side pins (test262 discovery + graded manifest, `fixtures_validate` fixture
 * count) live as consts in their commands — grep `REGRESSION PIN`. The as-authored
 * audits' formatted-file count is one shared const, `FIXTURES_FORMATTED_MIN` in
 * `crates/tsv_debug/src/audit/vacuity.rs`: they walk one corpus under one skip policy, so
 * a per-audit pin would only let their slack drift apart.
 */

import { CORPORA_ROOT, CORPORA_TREE } from './corpora.ts';
import type { Language } from './types.ts';

/**
 * The sibling checkouts the counts below were measured against, by git object: the
 * checkout's HEAD commit, or — with `tree` set — the id of that subtree at HEAD
 * (`git rev-parse HEAD:<tree>`), for a checkout whose corpus is one subtree and whose
 * other commits must not read as a corpus move. Both abbreviated; compared by prefix.
 *
 * The counts are only meaningful relative to the inputs that produced them, and an
 * upstream `package.json` version bumps only at RELEASE — so commits landing between
 * releases change the graded suite without changing the version. `pins:audit`'s version
 * check is blind to that window, so a pull inside it can leave every pin here describing
 * a suite that moved under it (docs/gate_counts.md §Why both the pins AND the checkout
 * alignment exist).
 *
 * So `pins:audit:checkouts` also compares each checkout's git object against the id recorded
 * here and WARNS on a move. That is deliberately a warning, not a failure: the count pins are the
 * gate (they fail on any real move in what's graded), and this exists to make a count-pin
 * trip *diagnosable* — "the corpus moved" vs "tsv regressed" is otherwise a reverse-
 * engineering exercise. An absent checkout, or one that isn't a git repo, is skipped, so
 * clean machines and CI still pass.
 *
 * `../corpora` is the real-code snapshot (`fuzdev/corpora`): every `real` and
 * `framework` corpus entry reads one of its collections, and one object id pins them
 * all — the author's dev repos included, which as live working trees admit no pin at all
 * (an ordinary edit moves their counts, so a gate over them is a re-pin treadmill). The
 * id is its `collections/` TREE's, not a commit's: the corpus is those bytes, so a tooling
 * or doc commit in the snapshot repo leaves a byte-identical tree and must not move every
 * pin here. A snapshot refresh is a corpus move like any other: re-record the tree id
 * here, re-run the corpus gates, re-pin.
 *
 * Re-record an id in the same change that re-pins the counts it explains. The
 * harvest-derived pins named beside each checkout ({@link SVELTE_REJECTS_PIN},
 * {@link CSS_REJECTS_PIN}, {@link TS_REPO_CORPUS_PIN}, {@link TS_REPO_REJECTS_PIN},
 * {@link WPT_CSS_HARVEST_PIN}, {@link TEST262_POSITIVES_PIN}) are re-derived by
 * `deno task bench:pins:suites` — a `deno task conformance` preflight, and nothing
 * in `deno task check` — so run it in that same change rather than leaving the move
 * for the conformance cadence to find (docs/gate_counts.md §Where the numbers live);
 * {@link SVELTE_STYLES_BLOCKS_PIN} rides `bench:harvest:svelte-styles` the same way.
 *
 * `pins` is graded by `gate_counts_test.ts`: every pin exported here must be named
 * (or glob-matched) by some checkout, and every name here must exist — so a new pin
 * cannot land without saying which checkout it was measured against, and a rename
 * cannot leave a ghost.
 */
export const GATE_CHECKOUT_IDS: Record<
	string,
	{ hash: string; tree?: string; pins: readonly string[] }
> = {
	// The real-code snapshot: every snapshot-tier corpus entry (`real`, `framework`,
	// `third_party`), so every corpus pin plus the styles harvest measured over the perf
	// view. Pinned by its `collections/` tree (see above); what each collection vendors is
	// its manifest.
	[CORPORA_ROOT]: {
		tree: CORPORA_TREE,
		hash: '5f40c547c',
		pins: ['CORPUS_FORMAT_*', 'CORPUS_PARSE_*', 'SVELTE_STYLES_BLOCKS_PIN']
	},
	// `../svelte` feeds the conformance view alone (its `tests` tree); its
	// `packages/svelte/src` is the snapshot's `svelte` collection.
	'../svelte': {
		hash: '5ccdfe355',
		pins: ['SVELTE_FIXTURES_PINS', 'SVELTE_REJECTS_PIN', 'CSS_REJECTS_PIN']
	},
	'../acorn-typescript': { hash: '923b213', pins: ['TS_FIXTURES_PINS'] },
	'../typescript': {
		hash: '637d5746b',
		pins: ['TS_REPO_PINS', 'TS_REPO_CORPUS_PIN', 'TS_REPO_REJECTS_PIN']
	},
	// Both prettier suites are Svelte-language inputs in the conformance view —
	// prettier's `tests/format/html` and the plugin's `test` are `.html` files the
	// loader reads as Svelte — so both feed {@link SVELTE_REJECTS_PIN} as well as
	// the CSS and corpus pins: of its 145 rejects, 40 come from ../prettier and 7
	// from ../prettier-plugin-svelte. A pin lists EVERY checkout it was measured
	// over, not just the one it is named after; `gate_counts_test.ts` grades that
	// each pin names at least one, which cannot see a missing second.
	'../prettier': {
		hash: '1dcd0b05d',
		pins: ['SVELTE_REJECTS_PIN', 'CSS_REJECTS_PIN', 'CORPUS_FORMAT_*', 'CORPUS_PARSE_*']
	},
	'../prettier-plugin-svelte': {
		hash: '7809486',
		pins: ['SVELTE_REJECTS_PIN', 'CORPUS_FORMAT_*', 'CORPUS_PARSE_*']
	},
	// The two suite-only checkouts: no version file to align, so their harvest
	// stamps are the only other place the commit is recorded — listed here so
	// `pins:audit:checkouts` names them when they move, like every other input.
	'../wpt': { hash: '7437c7bc7', pins: ['WPT_CSS_HARVEST_PIN', 'CSS_REJECTS_PIN'] },
	'../test262': { hash: '7153986fc', pins: ['TEST262_POSITIVES_PIN'] }
};

/** Exact expected counts for a fixtures parse-conformance gate (`lib/fixtures_gate.ts`). */
export interface GatePins {
	/** Suite inputs discovered under the default root. */
	scanned: number;
	/** Both-accept count — also catches an oracle collapse (everything "parity") that `scanned` can't see. */
	both_accept: number;
	/**
	 * Over-acceptance count (tsv accepts, the oracle rejects) — the deferred
	 * early-error frontier.
	 *
	 * A *finding* here is not gated (that is the deliberate deferral), but the COUNT
	 * is, because nothing else moves when the frontier grows: a new over-acceptance
	 * comes out of `parity` (both rejected, now only the oracle does), leaving both
	 * `scanned` and `both_accept` untouched. Without this pin the one direction the
	 * gate is *supposed* to tolerate is also the one direction it cannot see, so tsv
	 * could drift into accepting more of the oracle's parse errors release after
	 * release with every gate green. Lower it deliberately when a gap is closed.
	 */
	over_acceptance: number;
}

/** conformance:svelte-fixtures — `scanned` suite inputs + `both_accept`; provenance in `GATE_CHECKOUT_IDS`. */
export const SVELTE_FIXTURES_PINS: GatePins = {
	// `scanned` counts the checkout's graded `.svelte` inputs, which move without the declared
	// version moving — the version-window this file's header describes, since the checkout
	// declares 5.56.9 while carrying commits published after that release.
	//
	// One over-acceptance is an ORACLE-SKEW artifact rather than frontier growth, and is
	// expected to fall away on its own: `parser-modern/samples/css-nth-of-minified`, which
	// exercises the upstream fix that parses `:nth-child(2n of.important)` with no whitespace
	// after `of`. The checkout carries that fix; the pinned npm oracle (svelte@5.56.9)
	// predates it and rejects the file, so tsv — which accepts it, agreeing with CURRENT
	// Svelte — grades as over-accepting. Lower this deliberately when the canonical pin
	// next moves past the fix.
	scanned: 3406,
	both_accept: 3308,
	over_acceptance: 17
};

/** conformance:ts-fixtures — provenance in `GATE_CHECKOUT_IDS` (../acorn-typescript, oracle @sveltejs/acorn-typescript). */
export const TS_FIXTURES_PINS: GatePins = { scanned: 226, both_accept: 202, over_acceptance: 8 };

/**
 * conformance:ts-repo — `scanned` corpus files + `accept_parity` (tsv/tsc-baseline agreement);
 * provenance in `GATE_CHECKOUT_IDS` (../typescript). A rise on the pinned corpus is a parity
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
export const TS_REPO_PINS = { scanned: 13708, accept_parity: 12284, over_acceptance: 487 };

/**
 * corpus:compare:parse --all — EXACT per-language `compared` (both sides parsed and
 * the ASTs diffed) over the gates view: the `../corpora` snapshot + the prettier
 * suites, all pinned, so any move is a corpus refresh (re-pin with the new
 * `GATE_CHECKOUT_IDS` commit) or a one-language parse collapse that the
 * cross-language total would hide.
 */
export const CORPUS_PARSE_COMPARED_PIN: Record<Language, number> = {
	// The typescript denominator spans the whole JS/TS family `tsv format` discovers
	// (`.mts`/`.cts`/`.mjs`/`.cjs` joined `.ts`/`.js`), which admits five prettier-suite files:
	// `typescript/top-level-await/test.{mts,cts}`, `js/top-level-await/test.{mjs,cjs}` (all four
	// compared) and `js/babel-plugins/pipeline-operator-hack.cjs` (rejected by both parsers, so
	// counted nowhere). Two more on disk sit under the excluded `_errors_/`. The snapshot holds
	// none yet; a collection gaining one now grades instead of being skipped.
	//
	// 1353 / 4289 / 172 → 3450 / 5255 / 185: the six third-party collections join the gates as
	// the `third_party` tier (flowbite-svelte, layerchart, layercake, svelte-ux, svelte-maplibre,
	// language-tools — 2097 / 966 / 13 files), every one of them compared, and no group
	// undocumented: the one file that was, `svelte-ux/…/src/docs/Layout.svelte`, is the
	// module-comment duplication onto a statement-less instance script, whose shifted indices
	// the `svelte_instance_comment_duplication` matcher now admits (docs/conformance_svelte.md
	// §Comment Attachment Differences). The tsv-side failure counts below did not move, and
	// neither did the snapshot's `collections/` tree id — nothing it vendors changed, only
	// which of it the view reads.
	svelte: 3450,
	typescript: 5255,
	css: 185
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
 * corpus:compare:format --all — per-language MINIMUM exact-`match` count over the whole
 * gates view: the `../corpora` snapshot (the author's repos, the framework source and
 * the third-party libraries) and the prettier suites — every one a checkout `GATE_CHECKOUT_IDS` tracks and
 * `pins:audit:checkouts` verifies, so an aligned machine measures these EXACTLY. A shrink
 * fails (a formatter/oracle collapse in pinned code); a rise re-pins to keep the floor
 * tight. It stays a minimum (not exact) only so a fixed win needn't re-pin to pass — over
 * pinned inputs a `match` DROP is always a real regression. Provenance in
 * `GATE_CHECKOUT_IDS`; rationale in docs/gate_counts.md.
 */
export const CORPUS_FORMAT_MATCH_MIN: Record<Language, number> = {
	// 1047 → 2701: the six third-party collections join the gates as the `third_party` tier;
	// 1654 of their 2097 svelte files match (flowbite-svelte 929 of 1296, layerchart 333 of 336,
	// layercake 159 of 177, svelte-ux 161 of 198, svelte-maplibre 72 of 90), 438 are `known`
	// (prettier-shaped code the ecosystem repos never carry), and 5 are `unknown` — see the
	// unknown pin, which names them. `partial` is unmoved and SAFETY is 0 over every file.
	//
	// 2701 → 2703: `flowbite-svelte/.../dialog/Dialog.svelte` and
	// `.../bottom-navigation/BottomNavItem.svelte` arrive from `unknown` (5 → 3 there, which
	// names the change and the measurement).
	//
	// 2703 → 2704: `flowbite-svelte/.../stepper/TimelineStepper.svelte` arrives from
	// `unknown` — the lone-container hug reaching its last state. Reasoning on
	// `CORPUS_FORMAT_UNKNOWN_PIN`.
	svelte: 2704,
	// 4169 → 5124 and (css) 125 → 133: the `third_party` tier — see svelte. 955 of its 966
	// typescript files match (flowbite-svelte 338 of 338, layerchart 188 of 190, layercake 65 of
	// 66, svelte-ux 100 of 100, svelte-maplibre 57 of 57, language-tools 207 of 215), none
	// `known`, 11 `unknown` (named on the unknown pin); 8 of its 13 css files match and the
	// other 5, all layerchart's, are `known`.
	//
	// 5124 → 5128: `language-tools/…/svelte-check/src/incremental.ts`,
	// `language-tools/…/svelte2tsx/nodes/ExportedNames.ts`,
	// `prettier/tests/format/js/binary-expressions/inline-object-array.js` and
	// `prettier/tests/format/js/variable_declarator/multiple.js` arrive from `unknown`
	// (114 → 110 there, which names the change and the measurement).
	//
	// 5128 → 5134: the numbers-only array fill is prettier's `printArrayElementsConcisely`
	// in both halves it was missing. (a) Each item's comma moved INSIDE the fill content
	// (`[print(item), ","]` then a bare `line`), so the fill's pairwise measure counts the
	// next item's comma and the break lands where that comma would pass column 100 — before,
	// a `comma_line` separator measured the next item bare and the line ran to 101. (b) An
	// author's blank line between two items now separates them with the blank pair rather
	// than a `line` (prettier's `isLineAfterElementEmpty` → `[hardline, hardline]`), so the
	// blank survives and its hard break expands the array — before, the fill packed straight
	// through it. Six files leave for `match`, five from `unknown`
	// (`layercake/src/_data/unemployment.js` and prettier's `js/arrays/numbers-in-args.js`,
	// `numbers-in-assignment.js`, `numbers3.js` for (a); `js/arrays/preserve_empty_lines.js`
	// for (b)) and one from `partial` (`fuz_ui/src/lib/project_stats_data.ts`, whose explained
	// hunk was `fill_101_boundary`); nothing arrives anywhere, and the svelte + css bucket
	// lists are file-for-file identical (pre-change tree vs tip, `--all --json` set-diffed).
	// The third fix in that round — prettier refuses the fill when a signed literal's own
	// argument carries a comment — moves no count: no corpus file spells one.
	//
	// 5134 → 5136: the binaryish CONTINUATION-INDENT batch — ten parent positions that took no
	// continuation indent where prettier's binaryish fall-through gives one (`yield` / `yield*`
	// argument, `case` test, `for…of` / `for…in` right, `class extends`, bare expression
	// statement, labeled-statement body, `export default`, `export =`, a default parameter
	// value). Four files move, ALL in the prettier suite, each verified per file:
	//   `js/binary-expressions/short-right.js` → **match** (a bare `Math.abs(…) > 1;`
	//     statement — the expression-statement position).
	//   `typescript/conformance/types/functions/functionImplementationErrors.ts` → **match**.
	//   `typescript/conformance/types/functions/functionImplementations.ts` → `known` (what is
	//     left of it is detector-explained).
	//   `typescript/arrow/16067.ts` → `partial` → `unknown`, an improvement read backwards: its
	//     `a || …` statement hunk is FIXED, and the residue is the pre-existing curried-arrow
	//     body indent, which no detector explains — so the file stops being partly-explained
	//     and becomes wholly-unexplained. `compare` on it shows only that class.
	// Measured by diffing the `--all --json` bucket lists across the two corpus-profile
	// builds — these four are the only moves in any bucket, and `safety` / `errors` /
	// `expected_errors` are identical file-for-file.
	//
	// 5136 → 5139: the cast-seed first-argument hug. Three files arrive from `unknown`
	// (`language-tools/…/svelte-check/src/options.ts` and prettier's own tests for the rule,
	// `typescript/argument-expansion/argument_expansion.ts` +
	// `typescript/satisfies-operators/argument-expansion.ts`); a fourth,
	// `cosmicplayground/src/lib/notes.ts`, leaves `partial` for `known`. Reasoning on
	// `CORPUS_FORMAT_UNKNOWN_PIN`. Measured by formatting the gates view with a pre-change and
	// a tip `--profile corpus` CLI and byte-diffing the two trees: those four are the ONLY
	// movers among the 9,060 files `tsv format` accepts — every gates file but the ~245 `.html`
	// the harness routes through the Svelte printer, whose every bucket is unmoved.
	//
	// 5139 → 5140: the lone-literal call argument keeps its break point.
	// `language-tools/…/svelte2tsx/src/svelte2tsx/addComponentExport.ts` arrives from
	// `unknown` — its `${returnType(⏎'events'⏎)}` interpolation spans lines, so `${` hugs a
	// non-qualifying expression and the call is the only thing that can break. Reasoning on
	// `CORPUS_FORMAT_UNKNOWN_PIN`. ONE mover in any bucket: measured by diffing the
	// `--all --json` bucket lists across a pre-change and a tip `--profile corpus` build over
	// the whole 9,305-file gates view, where `partial` / `safety` / `errors` /
	// `expected_errors` come back identical file-for-file. The rule's lone-`function`-expression
	// siblings (plain call, `new`, the member-chain spelling, and prettier's flat-parameter rule
	// for them) are corpus-NEUTRAL: `fn(function () {})` and `obj.m({})` past the print width
	// are shapes no gates-view file holds, so the +1 is the literal fix alone.
	typescript: 5140,
	// ⚠️ A short `svelte_styles` cache understates every css count at once and reads exactly
	// like a regression: the harvest is a CORPUS INPUT, not a measurement of tsv, and a
	// standalone `corpus:compare:format --all` is the one entry point that does not chain it
	// (`conformance` does, late, beside the legs that read it). Re-harvest before believing a
	// css shortfall.
	css: 133
};

/**
 * corpus:compare:format --all — EXACT per-language `unknown` divergence count over the
 * gates view (the snapshot + prettier suites; see `CORPUS_FORMAT_MATCH_MIN`). Both
 * directions fail: a rise = a new unexplained divergence (fix it, catalog a detector in
 * `lib/divergence/patterns.ts`, or consciously re-pin a legitimately-unsupported new pinned
 * file); a drop = the backlog shrank, re-pin to record the win. The author's own repos are
 * gated here like every other snapshot collection. A single-run trip can be the FFI/sidecar
 * heisenbug — confirm on the single repo first. Same corpus + provenance as
 * `CORPUS_FORMAT_MATCH_MIN`.
 */
export const CORPUS_FORMAT_UNKNOWN_PIN: Record<Language, number> = {
	// The two open items, neither given a detector on purpose (a detector would assert a
	// sanction nothing has decided):
	//   `flowbite-svelte/src/lib/forms/button-toggle/ButtonToggle.svelte` — a seven-key
	//     shorthand object pattern assigned an object literal: prettier keeps the pattern flat
	//     (it fits) and breaks after `=`; tsv breaks the pattern and hugs the literal — the
	//     pattern group's fits walk measures what follows flat, where prettier's stops at the
	//     first line of the rest in break mode.
	//   `layerchart/packages/layerchart/src/lib/components/Text/Text.html.svelte` — whitespace
	//     only: a multi-line class value's `{expr}` continuation line keeps the author's SPACE
	//     indentation where prettier re-indents it with tabs.
	//
	// 0 → 5: the `third_party` tier arrives with five backlog items, the two above plus
	// `BottomNavItem.svelte`, `Dialog.svelte` and `TimelineStepper.svelte`.
	//
	// 5 → 3: `Dialog.svelte` and `BottomNavItem.svelte` LEAVE the bucket by MATCHING (`match`
	// 2701 → 2703). The multi-declarator list is a doc-tree `indent` now rather than literal
	// indent text after each hardline, so a break INSIDE a declarator lands one level past it
	// instead of at the statement's column (`declarations/variable/multiple/init_long`); and a
	// NESTED ternary's branch binaries no longer inherit the outer ternary's return/call
	// continuation indent (`expressions/ternary/nested_binary_branch_long`). Measured by
	// formatting every gates-view file with the pre-change and post-change binaries and
	// diffing the outputs: six files in the whole view change at all (these two, the two
	// typescript movers below, and two prettier-suite files that also leave `unknown` for
	// `match` — see the typescript pin), so nothing arrived in any bucket and `partial` /
	// `safety` / `errors` are unmoved.
	//
	// 3 → 2: `TimelineStepper.svelte` LEAVES for `match` (`match` 2703 → 2704) — "the
	// most-expanded hug fallback past width", now reached. Its `{circle({ status, class: … })}`
	// sits at column 186 inside a class attribute, so the hug's OWN first line is over width
	// and prettier drops the object into broken parens; tsv's lone-container arm was an
	// unconditional hug with no state below it, and an `is_truly_empty` special case that saw
	// only the empty containers. Both are retired for prettier's three-state ladder
	// (`ArgOpener::lone_hug_ladder`, shared with the plain-call / `new` / member-chain
	// function-expression arms). ONE mover in any bucket, by the same `--all --json`
	// bucket-list diff described on `CORPUS_FORMAT_MATCH_MIN`.
	svelte: 2,
	// Six of the `third_party` arrivals are still open, all of them the member-chain /
	// assignment / binaryish break-priority cluster, pinned here as the gate's backlog rather
	// than sanctioned:
	//   `language-tools/…/typescript/features/CompletionProvider.ts` — a declarator whose init
	//     is a `this.x.call()?.a?.b…` chain: prettier keeps the head on the `=` line and breaks
	//     the chain; tsv breaks after `=` and indents the whole chain.
	//   `language-tools/…/typescript/features/FoldingRangeProvider.ts` — `!!this.x.call()?.a
	//     ?.b?.c` in a declarator: a one-call chain is poorly breakable, so prettier breaks
	//     after `=` and keeps it whole; tsv breaks before the last `?.prop`.
	//   `language-tools/…/typescript/features/RenameProvider.ts` — `lang.call(a, b)
	//     ?.definitions?.[0]`: prettier keeps the call flat and breaks before `?.definitions`;
	//     tsv breaks the call's arguments.
	//   `language-tools/…/typescript-plugin/src/source-mapper.ts` — a for-of head
	//     destructuring `{0: a, 2: b, 3: c}` of `this.mappings[i]`: prettier keeps the pattern
	//     flat and breaks the member; tsv breaks the pattern (the ButtonToggle.svelte fits-walk
	//     difference again, see the svelte pin).
	//   `layerchart/…/components/Chart/Chart.shared.svelte.ts` — a type alias with three
	//     constrained or defaulted params: prettier's `isComplexTypeAliasParams` takes
	//     `break-lhs` (the params break, `=` stays on the closing line); tsv keeps them flat
	//     and breaks after `=`.
	//   `layerchart/…/utils/canvas.svelte.test.ts` — a hugged last-argument `function (this:
	//     T, ...args: any) {…}` whose params do not fit: prettier keeps the hug and breaks the
	//     params (its expanded-state fits measures nested groups in break mode); tsv breaks
	//     every argument.
	//
	// 103 → 114: eleven arrive with the `third_party` tier — the six above plus
	// `addComponentExport.ts`, `ExportedNames.ts`, `incremental.ts`, `options.ts` and
	// `layercake/src/_data/unemployment.js`, the one OVER-WIDTH output of the group.
	//
	// 114 → 110: FOUR files LEAVE the bucket by MATCHING (`match` 5124 → 5128), the
	// binaryish continuation-indent cluster. `language-tools/…/svelte-check/src/incremental.ts`
	// — an inlining logical chain with earlier operators at an assignment position takes
	// prettier's `samePrecedenceSubExpression` indent (`expressions/logical/inline_chain_long`).
	// `language-tools/…/svelte2tsx/nodes/ExportedNames.ts` — an alternate-nested ternary's
	// test takes prettier's `printBranch` indent plus `printTernaryTest`'s `align(2)`
	// (`expressions/ternary/nested_test_long`; it did reproduce minimally after all). Two
	// prettier-suite files leave on the same change as `Dialog.svelte` (the multi-declarator
	// list as a doc-tree `indent`): `js/binary-expressions/inline-object-array.js` — a hugged
	// object in a non-first declarator's `||` chain now sits one level past the declarator —
	// and `js/variable_declarator/multiple.js` — an arrow body inside a multi-declarator. Same
	// measurement as the svelte pin: these four plus the two svelte movers are the only six
	// files in the whole gates view whose output changes, so nothing arrived anywhere.
	//
	// 110 → 105: five files LEAVE for `match` — the numbers-fill round. Four to the comma
	// fix, whose fill content now carries its comma so the pairwise measure sees the next
	// item's (`layercake/src/_data/unemployment.js` above, and prettier's
	// `js/arrays/numbers-in-args.js`, `numbers-in-assignment.js`, `numbers3.js`, all lines the
	// old measure packed one item too far); one to the blank-line separator
	// (`js/arrays/preserve_empty_lines.js`, whose authored blanks the fill packed through).
	// Reasoning on `CORPUS_FORMAT_MATCH_MIN`; `partial` moves one the same way.
	//
	// 105 → 104: the binaryish continuation-indent batch. `js/binary-expressions/short-right.js`
	// and `…/functionImplementationErrors.ts` LEAVE for `match`, and `typescript/arrow/16067.ts`
	// ARRIVES from `partial` because the hunk a detector explained is the one that got fixed.
	// Reasoning on `CORPUS_FORMAT_MATCH_MIN`.
	//
	// 104 → 101: three files LEAVE for `match` — the cast-seed first-argument hug. Prettier's
	// `isHopefullyShortCallArgument` reads an `as` / `satisfies` seed through a cast branch of
	// its own (array element unwrapped, a lone type argument descended into, then `isSimpleType`
	// on what is left plus `isSimpleCallArgument` at depth 1 on the operand) and does not read a
	// `<T>x` assertion at all, so an angle-bracket seed is never short; tsv hugged both. The
	// three are `language-tools/…/svelte-check/src/options.ts` (above) and prettier's own tests
	// for the rule, `typescript/argument-expansion/argument_expansion.ts` and
	// `typescript/satisfies-operators/argument-expansion.ts` — whose `[] as unknown as number[]`
	// and `[] satisfies unknown satisfies number[]` seeds need the second half of the same
	// change: `isSimpleCallArgument` strips only the chain-element wrappers, so a cast OPERAND
	// is not simple either. `partial` moves one the other way in the same step (notes.ts, to
	// `known`), and the byte-diff over the gates view names those four as the only movers
	// (measurement on `CORPUS_FORMAT_MATCH_MIN`).
	//
	// 101 → 100: `language-tools/…/svelte2tsx/src/svelte2tsx/addComponentExport.ts` LEAVES for
	// `match`. A lone LITERAL argument no longer costs the call its break point: the printer
	// spelled prettier's 25-char `LONE_SHORT_ARGUMENT_THRESHOLD_RATE` at the wrong layer —
	// that threshold belongs to the ASSIGNMENT layout (`isPoorlyBreakableMemberOrCallChain`),
	// which chooses between breaking at the operator and breaking the call, never that the
	// call has no break at all — and emitted a group-free, line-free doc, so `fn('short')`
	// could not break once nothing above it could. Prettier has no such arm: every non-hug
	// argument ends at `printCallArguments`' soft-break group. Nothing arrived in `unknown`,
	// and the same one-mover byte-diff is described on `CORPUS_FORMAT_MATCH_MIN`.
	typescript: 100,
	css: 23
};

/**
 * corpus:compare:format --all — EXACT per-language `partial` divergence count over the
 * gates view (same semantics as `CORPUS_FORMAT_UNKNOWN_PIN`). The author's repos are gated
 * here like every other snapshot collection; each arrival is named below.
 */
export const CORPUS_FORMAT_PARTIAL_PIN: Record<Language, number> = {
	// 1 → 2: the author's repos join the pinned corpus; `zzz/src/lib/CapabilityWebsocket.svelte`
	// arrives (its explained hunk is `spaced_tag_travel`).
	svelte: 2,
	// 25 → 26: the author's repos join the pinned corpus; `fuz_ui/src/lib/project_stats_data.ts`
	// arrives (its explained hunk is `fill_101_boundary`).
	//
	// 26 → 27: cosmicplayground joins the `real` tier; `cosmicplayground/src/lib/notes.ts` arrives
	// with 3 of 5 hunks explained (`fill_101_boundary`, `comment_position`) and two from its
	// `chromas.reduce((result, chroma) => {…}, {} as Record<Chroma, Hue>)` calls that no
	// detector recognized — a real backlog item, closed at 24 → 23 below.
	//
	// 27 → 26: `fuz_ui/src/lib/project_stats_data.ts` leaves for `match` — its one explained
	// hunk was `fill_101_boundary`, the numbers-fill over-width the content-carried comma fixes.
	// Reasoning on `CORPUS_FORMAT_MATCH_MIN`, which moves +5 in the same step.
	//
	// 26 → 24: the binaryish continuation-indent batch — `typescript/arrow/16067.ts` leaves for
	// `unknown` (its explained hunk is fixed) and `…/functionImplementations.ts` leaves for
	// `known`. Reasoning on `CORPUS_FORMAT_MATCH_MIN`.
	//
	// 24 → 23: `cosmicplayground/src/lib/notes.ts` leaves for `known` — those two hunks are
	// FIXED, and what is left of the file is the explained pair it arrived with. `Record<K, V>`
	// carries two type arguments, so prettier's cast branch descends into nothing simple and it
	// refuses the first-argument hug; tsv now refuses it too. Reasoning on
	// `CORPUS_FORMAT_UNKNOWN_PIN`, which moves −3 in the same step.
	typescript: 23,
	css: 9
};

/**
 * bench:harvest:svelte-styles — EXACT extracted `<style>` block count over the perf
 * view, i.e. the `../corpora` snapshot's `.svelte` files. Pure input material (not a
 * tsv success count), and pinned like the suite harvests: the source is a pinned
 * snapshot, so a move is a snapshot refresh (re-pin with the new `collections/` tree
 * id), a collection joining a perf tier, or a broken extraction, and the harvest fails
 * BEFORE writing so a wrong cache never replaces a good one. Stamped on the snapshot's
 * `collections/` tree id and the perf view's entry list (`lib/harvest_stamp.ts`).
 * Measured 2026-09-05: ../corpora `collections/` at 5f40c547c, over the perf view's
 * 951 `.svelte` files.
 *
 * 278 → 401: earbetter and cosmicplayground join the `real` tier, so the perf view gains
 * their `.svelte` files (58 + 65 blocks) with the snapshot's tree id unmoved — the
 * view-composition move the stamp's `perf_entries` input exists to notice.
 */
export const SVELTE_STYLES_BLOCKS_PIN = 401;

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
 * bench:harvest:svelte-rejects — exact reject count. Measured 2026-08-24: ../svelte
 * at 5ccdfe355, ../prettier at 1dcd0b05d, ../prettier-plugin-svelte at 7809486,
 * oracle svelte@5.56.9, 145 of 4716 conformance-view Svelte files.
 * Fewer = the svelte/compiler oracle stopped rejecting (broken import/config);
 * more = it started rejecting wholesale — either way the cache would corrupt the
 * published coverage number. Re-derived by `bench:pins:suites` (see there).
 *
 * Moves with THREE checkout commits in {@link GATE_CHECKOUT_IDS}, not just the
 * one it is named after: the Svelte-language conformance corpus is the svelte
 * suite plus both prettier suites' `.html` (which the loader reads as Svelte), and
 * the split of the 145 is 98 / 40 / 7. The harvest stamps all three, so a pull of
 * any of them re-grades this pin rather than leaving it describing the previous
 * corpus.
 *
 * Three of the 145 are the suite's own fixtures for CSS parser fixes that landed
 * upstream AFTER the pinned oracle's release — namespaced type selectors
 * (`svg|*`, `*|*`) and `nth-child`'s `of` with no whitespace after it. They are
 * valid Svelte for the checkout and invalid for the oracle that defines validity
 * here, so they are excluded like any other reject; taking the oracle past them
 * returns all three to the corpus, and tsv then needs `ns|*` / `*|*`, which it
 * rejects today in parity with this oracle.
 */
export const SVELTE_REJECTS_PIN = 145;

/**
 * The conformance CSS corpus's REJECT count — files `svelte/compiler`'s `parseCss`
 * refuses — which `diagnostics/css_over_acceptance.ts` grades every `parse/css`
 * tool over. Deterministic given the inputs that build the corpus: the ../prettier,
 * ../svelte AND ../wpt checkout commits ({@link GATE_CHECKOUT_IDS} — the svelte
 * suite ships `.css` files of its own, so this pin moves with that checkout exactly
 * as {@link SVELTE_REJECTS_PIN} does), and the svelte oracle version.
 * {@link WPT_CSS_HARVEST_PIN} is one more input but NOT a substitute for ../wpt's
 * commit: it is a file COUNT, and an edit to an existing wpt test moves the content
 * this pin reads without moving it.
 *
 * Unlike {@link SVELTE_REJECTS_PIN} this list filters NOTHING — `parseCss` is not
 * a validity oracle in either direction (it accepts malformed CSS and rejects
 * valid modern CSS it doesn't implement), so excluding its rejects would drop
 * files tsv also fails and flatter tsv's own coverage. The list exists to give
 * the CSS surface the over-acceptance axis coverage can't show, and the pin is
 * what makes the reference row's grammar moving VISIBLE instead of silently
 * reshaping the published `parse/css` numbers.
 *
 * Derived LIVE from the corpus rather than from a harvest cache (nothing else
 * consumes the list), but graded and STAMPED like the harvests: `deno task
 * css:over-acceptance:pin` is a `bench:pins:suites` leg, so it is re-derived on
 * the same cadence as its siblings; the full `css:over-acceptance` profile grades it
 * too, and stamps the same three checkout commits. Measured 2026-08-24: ../prettier at 1dcd0b05d, ../svelte at 5ccdfe355,
 * ../wpt at 7437c7bc7, oracle svelte@5.56.9, 240 of 22642 conformance-view CSS files.
 *
 * One of the 240 is `css/samples/namespaced-type-selector/expected.css`, the `.css`
 * sibling of the namespaced-type-selector fixtures {@link SVELTE_REJECTS_PIN}
 * describes: a single upstream CSS-parser fix lands in both counts, so a change that
 * moves one of these pins should expect to move the other.
 */
export const CSS_REJECTS_PIN = 240;
