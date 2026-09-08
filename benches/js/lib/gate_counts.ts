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
 * conformance:ts-repo — provenance in `GATE_CHECKOUT_IDS` (../typescript); the buckets and what
 * each pin guards: docs/conformance_tsc.md.
 *
 * `scanned` single-file cases + `accept_parity` (tsv accepts, tsc's baselines call the file
 * valid). A rise in `accept_parity` on the pinned corpus is a parity gain, not a suite refresh; a
 * drop is USUALLY a regression — but read the other buckets before treating it as one, because it
 * counts only the agreeing-ACCEPT half. A file leaving for `parity reject` — tsv learning to refuse
 * something tsc's baseline refuses too — drops this number with agreement unchanged; only which
 * side of it moved. The two `unexpected` buckets staying 0 is the reading that settles it, since
 * those are where a real over-rejection lands.
 *
 * `scanned` includes the `.d.ts` cases, which this gate grades (`DECLARATIONS` in
 * `diagnostics/ts_repo_compare.ts` argues why, and why the bench harvest does not).
 *
 * The two `over_acceptance_*` pins (tsv accepts, tsc's baseline says invalid) guard the axis the
 * first two cannot see. `scanned` and `accept_parity` together fix how many files tsv accepts
 * *among the tsc-valid ones*; the reject / over-accept / beyond-acorn split of the remainder is
 * free. So a parser WIDENING — a fix that also starts accepting something tsc rejects — moves only
 * these numbers, and without a pin nothing anywhere reports it. That is the standing hazard of
 * every over-rejection fix: the new acceptance arrives unguarded. The split is by tsc's OWN
 * PARSER, run live over the file: `_parser` is what tsc's parser refuses and tsv takes (each one
 * a question for the grammar line — a production tsv should also refuse, or a recovery-only reject
 * tsv is right to defer); `_checker` is what tsc's parser accepts and its checker-side grammar
 * checks refuse (the deferred early-error posture by design — a rise there is policy, not a bug,
 * but still a decision). A `TS1xxx` code in a baseline is NOT proof of a parser rejection: the
 * range spans parser and checker (TS1036 ambient statements, TS1040 ambient `async`, TS1206
 * decorator placement are all checker-raised), which is why the live parser is asked rather than
 * the code range.
 *
 * `units_scanned` + `units_accept_parity`: the multi-file tests' `@filename` units, graded only
 * where the test's baselines carry no grammar error at all (a `TS1xxx` cannot be attributed to
 * one unit), so a unit is either accept-parity or an over-rejection — no over-acceptance or
 * reject-parity axis exists there. Pinned separately from the single-file counts because the
 * population is a different thing: `units_scanned` moves when the harness split or the
 * unit-language rule moves, which the single-file `scanned` cannot see.
 *
 * 7853 → 7858 `units_accept_parity`: the five `parserArrowFunctionExpression{8..12}.ts`
 * `fileTs.ts` units left `BEYOND_ACORN_KNOWN_GAPS` when tsv adopted tsc's rule for a
 * parenthesized arrow's return type in a conditional's consequent (`a ? (b) : c => d`
 * is `a ? b : (c => d)`; the annotation is kept only when a second `:` follows the
 * arrow) — conformance_svelte.md §TypeScript Corrections.
 */
export const TS_REPO_PINS = {
	scanned: 13708,
	accept_parity: 12284,
	over_acceptance_parser: 20,
	over_acceptance_checker: 467,
	units_scanned: 7874,
	units_accept_parity: 7858
};

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
	//
	// 2704 → 2705: `flowbite-svelte/.../button-toggle/ButtonToggle.svelte` arrives from
	// `unknown` — a container initializer under a breakable binding takes the fluid layout.
	// Reasoning on `CORPUS_FORMAT_UNKNOWN_PIN`.
	//
	// 2705 → 2706: `layerchart/…/components/Text/Text.html.svelte` arrives from `unknown` —
	// a whitespace-only part of a `style:` value re-indents with the printer's own break.
	// Reasoning on `CORPUS_FORMAT_UNKNOWN_PIN`. ONE mover in the whole view, measured both
	// ways: two staged trees over the 9,117 `find`-enumerated gates-view files formatted by a
	// pre-change and a tip `--profile corpus` CLI (per-file error output identical line for
	// line, 995/995, rewritten 5316/5316), and the `--all --json` bucket lists set-diffed
	// across the two corpus-profile FFI builds — nothing arrives anywhere, `partial` /
	// `safety` / `errors` / `expected_errors` identical file-for-file.
	svelte: 2706,
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
	//
	// 5140 → 5142: the two `Boolean(…)` / paren-callee fixes, which land as ONE step because
	// each was the other's last residual in `js/call/boolean/boolean.js`.
	//   `prettier/tests/format/js/binary-expressions/call.js` → **match** on the parenthesized
	//     binary CALL callee alone — the whole file is that one construct.
	//   `prettier/tests/format/js/call/boolean/boolean.js` → **match**, and it needs BOTH: its
	//     `(a || a || a)(Boolean)` callee is the paren shape and every other hunk is
	//     `isBooleanTypeCoercion`. Each fix alone leaves the file `unknown` on the other's gap,
	//     which is why neither measured it as a mover.
	// Measured on the merged tree, not derived: `--all --json` bucket lists set-diffed against
	// a build with the callee-paren hunks reverse-applied (`git diff … -- crates/ | git apply
	// -R`), 9,305 files. Those two are the only moves in any bucket — `known` is unmoved at
	// 120 so both land in `match`, and `partial` / `safety` / `errors` / `expected_errors`
	// come back identical file-for-file. Reasoning on `CORPUS_FORMAT_UNKNOWN_PIN`.
	//
	// 5142 → 5143: `language-tools/…/typescript-plugin/src/source-mapper.ts` arrives from
	// `unknown` — the lone-lookup inline clause. Reasoning on `CORPUS_FORMAT_UNKNOWN_PIN`.
	// ONE file in the whole snapshot + prettier suites changes bytes at all: two staged trees
	// over 11,749 `find`-enumerated files (so the ~800 `tsv format --list` prunes are in),
	// formatted by a HEAD and a tip `--profile corpus` binary, with the per-file error output
	// identical line for line.
	//
	// 5143 → 5144: prettier's `js/function-single-destructuring/array.js` arrives from
	// `unknown` — the sole-parameter hug declines a non-empty default. Reasoning on
	// `CORPUS_FORMAT_UNKNOWN_PIN`, whose svelte note carries the two-mover byte-diff this
	// step shares.
	//
	// 5144 → 5150: six files arrive from `unknown` — the member-chain / assignment
	// break-priority cluster, named with their causes and the seven-mover byte-diff on
	// `CORPUS_FORMAT_UNKNOWN_PIN`'s `95 → 89` step.
	typescript: 5150,
	// ⚠️ A short `svelte_styles` cache understates every css count at once and reads exactly
	// like a regression: the harvest is a CORPUS INPUT, not a measurement of tsv, and a
	// standalone `corpus:compare:format --all` is the one entry point that does not chain it
	// (`conformance` does, late, beside the legs that read it). Re-harvest before believing a
	// css shortfall.
	//
	// 133 → 138: five files arrive from `unknown`, closing the at-rule prelude ROUTING
	// TABLE and the media reader's node split. Reasoning and the bucket-list diff on
	// `CORPUS_FORMAT_UNKNOWN_PIN`'s `23 → 18` step.
	//
	// 138 → 137: prettier's `css/no-semicolon/url.css` leaves `match` for `known` — the ONE
	// mover of the `@import` prelude reader's positional fix (a value in first position, or
	// directly after the url/string, now reaches the value reader instead of the verbatim
	// fallback / a reject). The file is `@import ur⏎  l(//fonts…:400,400italic);`, a `url(`
	// split in two: prettier reads the `//` as a loose-mode line comment, throws on the paren
	// it swallowed, and freezes the prelude verbatim; tsv reads two `/` delimiters and
	// normalizes the list around them, as it already did for the same value after a media
	// type, under `@supports` and in a declaration. The old match was the raw fallback
	// matching a frozen value by accident. Cataloged (conformance_prettier_css.md §CSS: Values,
	// "Line-comment spelling in a function argument"), detected by `css_line_comment_freeze`,
	// so `unknown` is unmoved at 18. Measured by a per-file shape differential over the 3,268
	// `find`-enumerated css + svelte files of the corpus and the prettier suites (stdout,
	// stderr and exit code `cmp`'d between a HEAD-`preludes.rs` and a tip release binary):
	// this file is the only mover, and the `--all` run confirms it — `partial` / `safety` /
	// `errors` identical.
	//
	// 137 → 141: four files arrive from `unknown` — `@custom-selector` now ROUTES to the
	// selector printer, the last standard-CSS at-rule missing from the prelude routing
	// table. Reasoning and the bucket-list diff on `CORPUS_FORMAT_UNKNOWN_PIN`'s `18 → 14`
	// step.
	css: 141
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
	// Empty, and the fixture suite is what holds it there: every svelte divergence the gates
	// view still shows is cataloged, so a new file here is a new question.
	//
	// 0 → 5: the `third_party` tier arrives with five backlog items: the one above,
	// `ButtonToggle.svelte`, `BottomNavItem.svelte`, `Dialog.svelte` and `TimelineStepper.svelte`.
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
	//
	// 2 → 1: `ButtonToggle.svelte` LEAVES for `match` (`match` 2704 → 2705). Its seven-key
	// shorthand pattern is assigned an object literal, and prettier's `chooseLayout` has no
	// notion of a value that "expands on its own": the initializer takes `fluid`, whose marker
	// measures only ` {` and drops the literal after the `=` when `pattern = {` is what passes
	// the width, the pattern staying flat at 99. tsv withheld every layout from an object /
	// array / function / class initializer, leaving the pattern as the only thing that could
	// shed width; both layout twins (the declarator's cascade and `choose_layout`) now let
	// those values through to `fluid`, and an undecorated class joins prettier's never-break
	// list instead. Measured as two staged trees over 9,117 `find`-enumerated snapshot + suite
	// files formatted by a pre-change and a tip `--profile corpus` binary: this file and the
	// typescript mover below are the ONLY two whose bytes change, the per-file error output
	// is identical line for line, and the `--all --json` bucket lists set-diffed across the
	// same two builds show nothing arriving anywhere — `partial` / `safety` / `errors` /
	// `expected_errors` identical file-for-file.
	//
	// 1 → 0: `Text.html.svelte` LEAVES for `match` (`match` 2705 → 2706), the last one. A
	// `style:` directive's value is a CSS property value, and a value PART that is nothing
	// but whitespace can only be a token separator — a raw newline can sit in no CSS string
	// (Syntax 3 §4.3.5) and no escape can reach across a part boundary — so tsv re-emits such
	// a part as its own break and the continuation lands at the attribute's column, where it
	// had copied the author's space indentation into tab-indented output. Prettier reaches
	// further, into the parts that carry content, and collapses every whitespace run there
	// (`'Font   Name'` is a different font-family name; in a broken attribute list the
	// collapsed run becomes a line break inside a CSS string, which is a `<bad-string-token>`)
	// — the held half, cataloged and fixtured rather than matched. Measurement on
	// `CORPUS_FORMAT_MATCH_MIN`.
	svelte: 0,
	// No `third_party` arrival is open any more: the member-chain / assignment break-priority
	// cluster the tier brought in is closed (the `95 → 89` step below names each file).
	//
	// 103 → 114: eleven arrive with the `third_party` tier — the five the `95 → 89` step
	// names, plus
	// `source-mapper.ts` (which has since left, at `98 → 97` below), plus
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
	//
	// 100 → 98: two files LEAVE for `match`, on the pair of `Boolean(…)` / paren-callee fixes.
	// `js/binary-expressions/call.js` goes on the parenthesized binary CALL callee: prettier's
	// binaryish EARLY RETURN (`key === "callee" && isCallOrNewExpression(parent)`, taken ahead
	// of `shouldNotIndent`) expands the pair, `(⏎\ta &&⏎\tb⏎)()`, where tsv welded it to the
	// argument list; the `new` callee already took that shape and both now read it off one
	// seam. `js/call/boolean/boolean.js` needs that fix AND `isBooleanTypeCoercion` — its
	// `(a || a || a)(Boolean)` callee is the paren shape, its other hunks are the coercion
	// predicate — so each half alone left the file here on the other's gap. Nothing arrived.
	// Reasoning on `CORPUS_FORMAT_MATCH_MIN`, which carries the merged-tree measurement.
	//
	// 98 → 97: `source-mapper.ts` LEAVES for `match` — a member-chain backlog item of the
	// group above, and the ONLY one whose cause was the lone-lookup inline clause. Prettier's
	// `shouldInline` takes a lone `.prop`'s break point back only when the object and the
	// property are BOTH plain identifiers (member.js); tsv also inlined a `this` / `super`
	// base and a private-name property, so `this.mappings[i]` had no break point and the
	// for-of head shed width by breaking its destructuring pattern instead. Nothing arrived.
	// 97 → 96: prettier's `js/function-single-destructuring/array.js` LEAVES for `match`. Its
	// sole array-pattern parameter carries a NON-EMPTY default (`[…] = [1, 2, 3, 4, 5]`), and
	// prettier's `shouldHugTheOnlyFunctionParameter` hugs a defaulted pattern only when the
	// default is an identifier or an empty object / array; tsv hugged any default, so the
	// pattern broke inside `([` where prettier expands the parameter list and keeps the
	// pattern flat. Nothing arrived; the two-mover byte-diff is described on the svelte pin.
	//
	// 96 → 95: `prettier/tests/format/typescript/type-alias/conditional.ts` leaves for `known`.
	// Its `Equals<X, Y> = (<T>() => …) extends (<T>() => …) ? true : false` authors a redundant
	// paren shell around a generic extends-type; `is_generic_type` matched the shell node
	// (prettier's AST has none — its postprocess drops every `TSParenthesizedType`), so the
	// alias took `fluid` instead of break-after-`=` and the file carried an unexplained
	// layout beside its detector-known chained-conditional one. Measured by set-diffing the
	// suite's `--json` unknown lists between the tree and the same tree with the change
	// reverse-applied: that file is the only mover in any bucket.
	//
	// 95 → 89: six files LEAVE for `match` — the member-chain / assignment break-priority
	// cluster. One cause each:
	//   `language-tools/…/typescript/features/RenameProvider.ts` — the trailing tail a chain
	//     peels off its last call now holds computed lookups and `!` too (`?.definitions?.[0]`),
	//     each printed outside the chain as prettier's `printMemberExpression` does, so the
	//     call stays flat and the lookup takes the break.
	//   `language-tools/…/typescript/features/CompletionProvider.ts` — prettier's `memberChain`
	//     label is answered off the chain's own grouping against the short-chain cutoff
	//     (`this.x.y()?.…includes('s')` is three groups with no merge), where a call count
	//     had read every `this`-rooted chain as a merged factory head.
	//   `language-tools/…/typescript/features/FoldingRangeProvider.ts` — the declarator asks
	//     `shouldBreakAfterOperator` of the value UNDER its wrappers (`!!chain`, `await chain`),
	//     as the assignment twin already did.
	//   `prettier/tests/format/js/assignment/discussion-15196.js` — the same unwrap, reached
	//     through `void !!(await chain)`.
	//   `layerchart/…/components/Chart/Chart.shared.svelte.ts` — the alias's break-lhs arm
	//     (`isComplexTypeAliasParams`) is asked ahead of the intersection / conditional /
	//     internal-breaking layouts, so the `<…>` list breaks and the `=` keeps the `>` line.
	//   `layerchart/…/utils/canvas.svelte.test.ts` — a last argument that will break offers
	//     prettier's two-state ladder (hug, then all broken out), so a chain measuring the
	//     call reads the hug and stops at the parameter list's own softline; and a short
	//     chain is a plain group whatever its first call's arity, its renderer walking that
	//     ladder to the hug.
	// Measured as two staged trees over 9,117 `find`-enumerated snapshot + suite files
	// formatted by a pre-change and a tip `--profile corpus` CLI: these six are the ONLY files
	// whose bytes change, with the per-file error output identical line for line; the
	// `--all --json` bucket lists set-diffed across the same two FFI builds show nothing
	// arriving anywhere — `partial` / `safety` / `errors` / `expected_errors` identical
	// file-for-file. One more rule rides in corpus-NEUTRAL by construction: the container
	// predicate refuses an EMPTY literal under a cast the way `couldExpandArg` does, which is
	// what keeps `language-tools/…/typescript-plugin/src/language-service/find-references.ts`
	// (`fn(cb, <ts.ReferenceEntry[]>[])`) byte-identical once the short-chain ladder that had
	// been holding it broken out is gone.
	//
	// 89 → 88: `prettier/tests/format/typescript/type-parameters-arguments/10732.ts` leaves for
	// `match`. The mapped type's `[key in constraint]` binding now carries prettier's bracket
	// group (`group(["[", indent([softline, …]), softline, "]"])`, `mapped-type.js`), so a
	// binding too wide for its line breaks INSIDE the brackets with the key one level in and
	// the `]` on its own line, and a constraint union hangs after `in` inside them — where the
	// old flat `[key in …]` exploded a long constraint beside the `[`. Measured by the
	// baseline-vs-tip byte A/B over the prettier `typescript` + `js` suites (2,113 files):
	// three movers, this one and the two mapped-type comment files
	// (`typescript/comments/mapped-types.ts`, `typescript/prettier-ignore/mapped-types.ts`),
	// which take the same bracket break and stay `unknown` on their pre-existing
	// comment-position hunks alone — the union-hug seam work landing alongside moved no suite
	// file, glued-block unions being absent from an already-formatted corpus.
	typescript: 88,
	// 23 → 18: five files LEAVE for `match` (`match` 133 → 138), all of them one language
	// question — which reader prettier hands an at-rule prelude to, and what that reader
	// does with the text inside a feature expression.
	//
	//   - `atrule/custom-media.css`, `case/case.css`,
	//     `stylefmt-repo/custom-media-queries/…`, `stylefmt-repo/media-queries-ranges/…`
	//     leave because `@custom-media` now ROUTES to the media reader. `parser-postcss.js`
	//     sends `["media", "custom-media"]` to `parseMediaQuery`; tsv's table tested `media`
	//     alone, so a `@custom-media` prelude fell to the verbatim raw branch and took no
	//     whitespace collapse, feature-name lowercase, unit fold or number normalization.
	//     Printer-only: the wire prelude is `strip_css_comments(span.extract(source))` for
	//     the media arm and the raw arm alike.
	//   - `stylefmt-repo/at-media/at-media.css` leaves because the media reader now has the
	//     NODE SPLIT it was missing. `parseMediaQuery` cuts a prelude into nodes joined by
	//     one space, and `parseMediaFeature` cuts a feature expression at its first colon
	//     into a name (runs collapsed) and a value (verbatim), trimming only at the
	//     expression's OWN parens. tsv collapsed whitespace beside every paren at every
	//     depth, so a grouped condition lost the spaces prettier keeps —
	//     `(not ( screen and ( color ) ))` came back `(not (screen and (color)))`.
	//
	// The same split makes a `media-value`'s text the author's, and a comma inside a
	// feature expression its own byte, which is the media half of the separator/operator
	// rule; the value half is the mirror — `@supports` and `@import`'s `supports()` go to
	// `parseValue`, which regroups and prints its own `, `, while `@container` is on
	// neither list and stays verbatim. Both halves are corpus-neutral on their own and
	// ride this step's measurement.
	//
	// Measured by the ritual's old-binary/new-binary `--all --json` bucket-list diff over
	// the whole gates view: these five files are the ONLY ones that change bucket in any
	// language, and `partial` / `safety` / `errors` / `expected_errors` are identical
	// file-for-file, `typescript` and `svelte` unmoved in every bucket.
	//
	// 18 → 14: four files LEAVE for `match` (`match` 137 → 141), the routing table's last
	// standard-CSS gap — `atrule/custom-selector.css`, `attribute/custom-selector.css`,
	// `stylefmt-repo/custom-selectors/…`, `stylefmt-repo/cssnext-example/…`. Prettier's
	// `custom-selector` arm splits the `:--name` off a `@custom-selector` prelude and hands
	// the rest to `parseSelector`, printing name, `line`, then the selectors joined `,` +
	// `line` in one indented group; tsv's table had no arm, so the prelude fell to the
	// verbatim raw branch — no comma respacing, no combinator spacing, no attribute-quote
	// normalization, and no way to break a long list at its commas. tsv now reads the same
	// shape (`:` glued to a `--` ident, a gap, a strict complex-selector list) into its own
	// `PreludeValue::CustomSelector` and prints it through the selector printer's comma seam.
	// Printer-only: the wire prelude is `strip_css_comments(span.extract(source))` for the
	// new arm and the raw arm alike, the span the raw reader's in both.
	//
	// Measured by the ritual's old-binary/new-binary `--all --json` bucket-list diff over
	// the whole gates view: these four files are the ONLY ones that change bucket in any
	// language, and `partial` / `safety` / `errors` / `expected_errors` are identical
	// file-for-file, `typescript` and `svelte` unmoved in every bucket.
	//
	// 14 → 15: `parens/parens.css` ARRIVES from `partial` (`partial` 9 → 8) — an improving
	// move that reads as the opposite. The file's one explained hunk was `css_value_wrap` /
	// `fill_101_boundary`: tsv wrapped its 181-column `filter: progid:…Shadow(…) progid:…`
	// value where prettier's `value-unknown` arm freezes any value starting with `progid:`.
	// tsv now takes the same opaque class (`progid_opaque_value` in `declarations.rs`, a
	// verbatim source slice — no normalization, no wrap; cataloged in
	// conformance_prettier_css.md §CSS: Values, "`progid:` opaque value"), so that hunk is
	// GONE and the file is left with its five pre-existing unexplained hunks (the
	// `1 * 1 (1) * 1` operator respacing, `round(1.5) / 2`, `func(+20px, + 20px)`, the
	// `'test'+1` string-arithmetic family, `"("attr(title)")"`) — zero explained, so it
	// classifies `unknown`. Measured by the ritual's old-binary/new-binary `--all --filter
	// css --json` bucket-list diff: this file is the ONLY mover in any css bucket, `safety`
	// / `errors` / `expected_errors` identical file-for-file, `match` unmoved at 141
	// (`colon/colon.css`, the other `progid:` file, carries nothing the pass would touch),
	// and the full `--all` run has `typescript` and `svelte` unmoved in every bucket.
	css: 15
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
	//
	// 23 → 22: `prettier/tests/format/js/if/condition-break/boolean-expression.js` leaves for
	// `known` — its `Boolean?.(a || b || c)` hunk is FIXED (an optional call is not prettier's
	// `isBooleanTypeCoercion`, so the chain indents; tsv read the callee name alone), and what
	// is left is the two cataloged delimiter-line `comment_position` hunks it already carried.
	// The ONLY mover in any bucket: `--all --json` bucket lists set-diffed across a pre-change
	// and a tip `--profile corpus` build over the whole 9,305-file gates view — `unknown`,
	// `safety`, `errors` and `expected_errors` identical file-for-file, `match` unmoved at
	// 5140. The same change also fixes the chain-headed spelling (`Boolean(a || b).c()`, flat
	// where the member-chain printer never asked) and the two-argument one, neither of which
	// any gates file spells at breaking width. `js/call/boolean/boolean.js` holds all three and
	// needed one more thing to close: the callee-paren fix landing beside this one, which is
	// what carries it to `match` (see `CORPUS_FORMAT_UNKNOWN_PIN`); `partial` is unmoved by
	// that second half, and 22 is re-measured on the merged tree.
	typescript: 22,
	// 9 → 8: `prettier/tests/format/css/parens/parens.css` leaves for `unknown` — its one
	// explained hunk, the wrapped `progid:` value, is FIXED (tsv freezes the value as
	// prettier does), leaving the five unexplained hunks it always carried. Reasoning and the
	// bucket-list diff on `CORPUS_FORMAT_UNKNOWN_PIN`'s `14 → 15` step.
	css: 8
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
