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
 * Re-record a commit in the same change that re-pins the counts it explains. The
 * harvest-derived pins named beside each checkout ({@link SVELTE_REJECTS_PIN},
 * {@link CSS_REJECTS_PIN}, {@link TS_REPO_CORPUS_PIN}, {@link TS_REPO_REJECTS_PIN},
 * {@link WPT_CSS_HARVEST_PIN}, {@link TEST262_POSITIVES_PIN}) are re-derived by
 * `deno task bench:pins:suites` — a `deno task conformance` preflight, and nothing
 * in `deno task check` — so run it in that same change rather than leaving the move
 * for the conformance cadence to find (docs/gate_counts.md §Where the numbers live).
 *
 * `pins` is graded by `gate_counts_test.ts`: every pin exported here must be named
 * (or glob-matched) by some checkout, and every name here must exist — so a new pin
 * cannot land without saying which checkout it was measured against, and a rename
 * cannot leave a ghost. The one export outside the map is
 * {@link SVELTE_STYLES_BLOCKS_MIN}, measured over the live dev repos, which have no
 * commit to record (`UNTRACKED_PINS` in the test carries that reason).
 */
export const GATE_CHECKOUT_COMMITS: Record<string, { commit: string; pins: readonly string[] }> = {
	'../svelte': {
		commit: '5ccdfe355',
		pins: [
			'SVELTE_FIXTURES_PINS',
			'SVELTE_REJECTS_PIN',
			'CSS_REJECTS_PIN',
			'CORPUS_FORMAT_*',
			'CORPUS_PARSE_*'
		]
	},
	'../acorn-typescript': { commit: '923b213', pins: ['TS_FIXTURES_PINS'] },
	'../typescript': {
		commit: '637d5746b',
		pins: ['TS_REPO_PINS', 'TS_REPO_CORPUS_PIN', 'TS_REPO_REJECTS_PIN']
	},
	'../kit': { commit: 'c0c936124', pins: ['CORPUS_FORMAT_*', 'CORPUS_PARSE_*'] },
	'../svelte.dev': { commit: '996bd63e4', pins: ['CORPUS_FORMAT_*', 'CORPUS_PARSE_*'] },
	// Both prettier suites are Svelte-language inputs in the conformance view —
	// prettier's `tests/format/html` and the plugin's `test` are `.html` files the
	// loader reads as Svelte — so both feed {@link SVELTE_REJECTS_PIN} as well as
	// the CSS and corpus pins: of its 145 rejects, 40 come from ../prettier and 7
	// from ../prettier-plugin-svelte. A pin lists EVERY checkout it was measured
	// over, not just the one it is named after; `gate_counts_test.ts` grades that
	// each pin names at least one, which cannot see a missing second.
	'../prettier': {
		commit: '1dcd0b05d',
		pins: ['SVELTE_REJECTS_PIN', 'CSS_REJECTS_PIN', 'CORPUS_FORMAT_*', 'CORPUS_PARSE_*']
	},
	'../prettier-plugin-svelte': {
		commit: '7809486',
		pins: ['SVELTE_REJECTS_PIN', 'CORPUS_FORMAT_*', 'CORPUS_PARSE_*']
	},
	// The two suite-only checkouts: no version file to align, so their harvest
	// stamps are the only other place the commit is recorded — listed here so
	// `pins:audit:checkouts` names them when they move, like every other input.
	'../wpt': { commit: '7437c7bc7', pins: ['WPT_CSS_HARVEST_PIN', 'CSS_REJECTS_PIN'] },
	'../test262': { commit: '7153986fc', pins: ['TEST262_POSITIVES_PIN'] }
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

/** conformance:svelte-fixtures — `scanned` suite inputs + `both_accept`; provenance in `GATE_CHECKOUT_COMMITS`. */
export const SVELTE_FIXTURES_PINS: GatePins = {
	// 3392 → 3406 / 3297 → 3308 / 16 → 17: a `../svelte` pull (20b341f10 → 5ccdfe355) added 14
	// graded `.svelte` inputs — the version-window this file's header describes, since the
	// checkout still declares 5.56.9 while carrying commits published after that release. No
	// tsv change is in it: an accept-verdict diff over 11318 corpus files across the change
	// that landed beside this re-pin moved ZERO of them, and `unexpected` (over-REJECTIONS,
	// the gated direction) stays 0.
	//
	// The `over_acceptance` step is an ORACLE-SKEW artifact rather than frontier growth, and
	// is expected to fall back to 16 on its own. Its one new entry is
	// `parser-modern/samples/css-nth-of-minified`, added by the upstream fix that parses
	// `:nth-child(2n of.important)` with no whitespace after `of`. The checkout carries that
	// fix; the pinned npm oracle (svelte@5.56.9) predates it and rejects the file, so tsv —
	// which accepts it, agreeing with CURRENT Svelte — grades as over-accepting. Lower this
	// deliberately when the canonical pin next moves past the fix.
	scanned: 3406,
	both_accept: 3308,
	over_acceptance: 17
};

/** conformance:ts-fixtures — provenance in `GATE_CHECKOUT_COMMITS` (../acorn-typescript, oracle @sveltejs/acorn-typescript). */
export const TS_FIXTURES_PINS: GatePins = { scanned: 226, both_accept: 202, over_acceptance: 8 };

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
export const TS_REPO_PINS = { scanned: 13708, accept_parity: 12284, over_acceptance: 487 };

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
	//
	// 502 → 499: the embedded-body freeze
	// (conformance_prettier_svelte.md §Svelte: Foreign-language embedded bodies) takes three
	// reproducible files out of `match`, each of them an ACCIDENT of the heuristic it replaced
	// rather than a regression. `prettier-plugin-svelte/.../style-lang-less.html` and
	// `style-type-less.html`: the old `<style>` path re-indented a foreign body by inferring
	// its indent unit, which on an already-well-formed less body reproduced prettier's less
	// printer exactly — tsv now emits the author's bytes, so a 4-space body stays 4-space.
	// `prettier/tests/format/html/multiparser/unknown/unknown-lang.html`: `lang="unknown"` on
	// both tags, which tsv used to format with its TS and CSS printers on the strength of
	// nothing. Sanctioned by `svelte/style/foreign_lang_frozen_prettier_divergence` and
	// `svelte/script/foreign_lang_frozen_prettier_divergence`; all three land in `known` via
	// the `foreign_body_freeze` detector, so `unknown` and `partial` are unmoved. Those three
	// plus `no-tag-snippings.html` (divergent before and after) are the ONLY reproducible
	// svelte files whose tsv output changes at all — an A/B of every file under the framework
	// roots and both `.html` suites against a pre-change binary. The floor it replaces was
	// four below the tree's real 502, which is why a three-file drop cleared it; 499 is tight.
	svelte: 499,
	// 2332 → 2334: two reproducible files, `prettier/tests/format/js/comments/11273.js` and
	// `.../trailing-jsdocs.js`, whose divergence was a container-end trailing comment RUN the
	// author glued onto one line and tsv split onto two. That run's separator now asks the
	// source (docs/comments.md §Trailing and dangling runs), so a glued pair keeps its line as
	// it does in prettier. Both files leave `known` — the count that moves against this one.
	//
	// 2334 → 2335: `prettier/tests/format/js/sequence-break/break.js`, which leaves `unknown`
	// by matching once a sequence breaks on width. Reasoning on `CORPUS_FORMAT_UNKNOWN_PIN`,
	// which moves the other way in the same step.
	typescript: 2335,
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
	//
	// 109 → 110: `js/unary-expression/comments.js` arrives from `partial`, and — like the two
	// arrivals above — it arrives by getting BETTER (9/11 differing lines → 6/6). The unary
	// comment-holder parens are now prettier's own `group(["(", indent([softline, arg]),
	// softline, ")"])` instead of a gate-selected hard-broken arm, so a `function`-expression
	// operand's body break reaches them (`!(⏎\tfunction () {…} /* foo */⏎)` — the hunk
	// `comment_position` was claiming) and a run the author glued onto its own line no longer
	// pre-empts the width decision. What is left is six hunks of ONE cataloged divergence:
	// prettier hoists a leading comment out of an operand's required parens
	// (`!(/* foo */ (x = y))`) where tsv keeps it inside the single pair — the assignment,
	// conditional, sequence, arrow, `yield` and `await` operands, all of
	// `conformance_prettier_ts_comments.md` §Comment relocation. No detector reaches it, so
	// the file drops out of `partial`; widening one is a follow-up, not a precondition.
	// A/B'd against a reverse-patched HEAD binary over the full corpus: this file is the ONLY
	// move in any bucket — `safety` is 0 both sides and `errors` is byte-identical.
	//
	// 110 → 109: `js/while/indent.js` LEAVES the bucket by MATCHING (`match` 4353 → 4354) —
	// prettier's own adversarial test for this behavior, whose every case is a long `if` /
	// `while` / do-while head. A do-while's condition now takes the same condition group
	// `if` and `while` take, so its parens open for width instead of wrapping the operands at
	// the statement's own indent, where they read as statements. tsv had kept the plain
	// self-grouping expression doc on the reading that the do-while has no enclosing group
	// for the ungrouped binary chain to break with; it has one — the condition group itself,
	// which is what prettier's `printDoWhileStatementCondition` (its
	// `printIfStatementCondition` under another name) builds too. A/B'd against a
	// HEAD-source rebuild over the full corpus: this file is the ONLY move in any bucket —
	// nothing arrived in `unknown`, and `partial` / `safety` / `errors` / `expected_errors`
	// are byte-identical.
	//
	// 109 → 108: `js/sequence-break/break.js` LEAVES the bucket by MATCHING (`match` 4356 →
	// 4357) — prettier's own adversarial test for sequence breaking, whose every case is a
	// sequence too wide for its line. A `SequenceExpression` now joins its operands with
	// `,` + `line` under prettier's three parent-keyed layouts rather than a flat `", "` that
	// could never break (`printSequenceExpression`; `SeqLayout` in
	// `printer/expressions/operators.rs`). The file's own residue was the `Indented` arm's
	// indent SCOPE: wrapping the whole run also indents the lines a first operand breaks
	// ITSELF (`((a = b ? c : fn()), …)`), where prettier indents only the continuations —
	// which is why the arm splits the run rather than wrapping it. Measured over the full
	// corpus: this file is the only move in any bucket, and `partial` / `safety` / `errors` /
	// `expected_errors` are unchanged.
	//
	// 108 → 104: FOUR files LEAVE the bucket by MATCHING (`match` 2394 → 2398 over the
	// reproducible subset), from three independent rules that happened to be measured
	// together. `js/binary-expressions/in_instanceof.js` — a UNARY left operand of
	// `in`/`instanceof` now keeps prettier's clarity parens (`(!a) in b`), the rule its
	// `**` sibling already had in `needs_parens_binary_operand`; an `UpdateExpression` is
	// still bare, which is prettier's own `node.type` term and the file's own control.
	// `js/arrays/numbers-with-holes.js` — the blank-line scan across an ELISION stops at the
	// first comment BELOW the element's line instead of running the whole span, so a blank
	// the author left in front of a hole survives the comment's slide past the hole's comma.
	// `js/arrows/chain-as-arg.js` + `js/arrows/chain-in-logical-expression.js` — prettier's
	// `shouldBreakChain` is now a curried chain's group `shouldBreak` in call-argument and
	// binaryish position rather than a refusal of the chain layout, so those heads take the
	// progressive indent they were one level short of. Measured by diffing the `unknown`
	// lists against a reverse-patched build over the whole corpus: these four are the only
	// moves in any bucket, nothing arrived in `unknown`, and `partial` / `safety` / `errors`
	// / `expected_errors` are byte-identical.
	// ⚠️ That same measurement puts the reproducible-subset `match` at 2394 BEFORE this
	// change, well above `CORPUS_FORMAT_MATCH_MIN`'s 2335 — a pre-existing slack in that
	// floor, unrelated to this entry and deliberately not folded into it.
	//
	// 104 → 111: SEVEN files ARRIVE from `partial` in one step — a reclassification, not a
	// loss. The clause-body statement tail now defers its own-line pre-`;` comment run
	// through `line_suffix` (dedented to the flushing construct's level), so a collapsed
	// head hoists the comment past the whole statement exactly as prettier does
	// (`if (1) foo;⏎// c`) — the six `js/no-semi/*-statement.js` files and
	// `js/for/continue-and-break-comment-without-blocks.js` lose the clause hunks the
	// detector used to explain, leaving only a pre-existing residue no detector matches:
	// for the no-semi six, a `// prettier-ignore` freeze's `;` binding; for the for/continue
	// file, prettier reordering the hoisted comment past the author's blank line (tsv keeps
	// the authored order: comment, then blank — cataloged, pinned by
	// `for/clause_terminator_comment_then_blank_prettier_divergence`). `js/comments/break-continue-statements-2.js`
	// leaves `partial` for **match** outright in the same step. Measured by byte-diffing the
	// formatted prettier suites against a pre-change binary: these eight are the only moves
	// in any bucket, and `safety` / `errors` / `expected_errors` are unchanged.
	//
	// −1: `js/function/issue-12967.js` leaves for **match** — a fix, not a
	// reclassification. A JSDoc-annotated arrow as an IIFE callee had its leading run
	// hoisted out of the pair the callee is required to carry; the run now stays inside
	// it, which is prettier's own answer at that position. Measured by diffing the
	// `unknown` lists against a reverse-patched build over the whole corpus: this file
	// and `js/function/iife.js` (see `CORPUS_FORMAT_PARTIAL_PIN`) are the only moves in
	// any bucket, and `safety` / `errors` / `expected_errors` are byte-identical.
	//
	// ⚠️ The two entries above landed on SEPARATE branches, each measured against 104 in its
	// own tree, so their deltas are NOT composable in general. **110 is the re-measurement on
	// the merged tip**, not a sum — it happens to equal 111 − 1 because the two movers are
	// disjoint (the clause-tail seven are `js/no-semi/*` and `js/for/*`, the IIFE mover is
	// `js/function/*`), and that was verified per file rather than assumed.
	//
	// 110 → 109 (`main`, the value-head freeze): `js/sequence-expression/ignored.js` LEAVES
	// the bucket by MATCHING — a file whose whole content is a `// prettier-ignore` in an
	// arrow's `=>`→body gap. The `=>`→body head now resolves the value-head freeze
	// (`Printer::value_head_frozen_span`), along with the enum member's and the `for`
	// header's init declarator `=`→value gaps, the last three hosts of the assignment
	// family that were absent from the rule. A/B'd against a HEAD-source rebuild over the
	// whole corpus: this file is the ONLY move in any bucket — nothing arrived in
	// `unknown`, and `partial` / `safety` / `errors` / `expected_errors` are byte-identical.
	//
	// 110 → 108 (`bug456`, the multiline-template hug): `js/dynamic-import/template-literal.js`
	// and `js/dynamic-import/import-phase.js` leave for **match** — a fix, not a
	// reclassification. A dynamic `import()` never asked the sole-multiline-template hug, so
	// it expanded where prettier keeps the specifier on the `(` line; `import()` shares
	// `printCallExpression` with a call, so the rule reaches it too. Measured by a
	// whole-corpus byte-diff against a reverse-patched build (~23k files): these two are the
	// ONLY non-fixture movers in any bucket, and both land byte-identical to prettier's own
	// committed snapshot.
	//
	// ⚠️ **107 is the re-measurement on the merged tip, not 110 − 1 − 2.** The two entries
	// above landed on separate branches, each measured against 110 in its own tree; the sum
	// is only *predictive*, and this file's earlier merge (111 − 1 = 110) is the standing
	// reminder that it has to be verified per file rather than assumed. It was: the three
	// movers are disjoint (`js/sequence-expression/*` vs `js/dynamic-import/*`) and
	// `corpus:compare:format --all` on the merged tree reports 107.
	//
	// 107 → 106 (`bug461`, the unary/`${`/computed-key/spread value-head freeze):
	// `js/sequence-expression/ignore.js` leaves for **match** — a `+` whose operand carries an
	// own-line `// prettier-ignore`, the unary→operand head this cluster added. A/B'd against a
	// HEAD worktree over the whole corpus: this is the ONLY mover in any bucket (the unknown
	// lists are otherwise identical file-for-file, no new entry), SAFETY 0 both sides.
	//
	// 106 → 105 (`bug467`, the assignment-target member chain): `js/assignment/issue-1966.js`
	// leaves for **match** — prettier's own regression test for this behavior, whose three
	// cases are all a dotted target assigned an over-width value. An assignment target's
	// `.prop` lookups now carry no break point (prettier's `printMemberExpression`
	// `shouldInline`), so the target prints as one unbreakable unit and the assignment sheds
	// width after the operator instead of splitting the thing being assigned to. A/B'd
	// against the same tree with the mark disabled, over the whole corpus: this is the ONLY
	// mover in any bucket (`match` 4416 → 4417, the unknown lists otherwise identical
	// file-for-file, nothing arrived), and `partial` / `safety` / `errors` /
	// `expected_errors` are byte-identical.
	//
	// 105 → 104 (`bug469`, the post-arrow glued line comment): `js/arrows/issue-17421.js`
	// leaves for **`partial`**, not for `match` — a reclassification, and the file is
	// prettier's own regression test for this gap. A `//` the author glued to `=>` now keeps
	// that line (§Uniform Forced-Continuation Indent), so the file's arrow hunks become
	// `comment_position` and the detector recognizes them; what keeps it out of `known` is a
	// *different*, still-uncataloged divergence the same file carries — a **block** body
	// behind a parameter list (`(id) => // c⏎{}`), where prettier hugs `=> {` and relocates
	// the comment INSIDE the block. That residue is deliberately left unexplained rather
	// than given a detector pattern: classifying it would assert a sanction nothing has
	// decided. A/B'd against a worktree holding this branch's code minus the arrow change,
	// over the whole corpus: this file is the ONLY mover in any bucket, and `safety` /
	// `errors` / `expected_errors` are byte-identical file-for-file.
	//
	// 104 → 103 (the comment trim's whitespace class): `js/comments/trailing_space.js` leaves
	// for **match**. A comment's trailing trim is prettier's `String.prototype.trim*` — JS
	// `\s` — where tsv spelled it `str::trim_end`, whose Unicode `White_Space` disagrees at
	// exactly two code points and gets both wrong: it deletes a `<NEL>` prettier keeps and
	// keeps a `<ZWNBSP>` prettier deletes. Measured by diffing the `unknown` lists across the
	// change rather than by the count: this file is the ONLY mover in any bucket, nothing
	// arrived in `unknown`, and `partial` / `safety` / `errors` / `expected_errors` are
	// byte-identical file-for-file. (Same class as the CSS-side trims that moved in the same
	// round — the wire's `trim_wire*` and the printer's `trim_property_part` — which move no
	// count here because no corpus file carries one.)
	typescript: 103,
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
	//
	// 33 → 32: `js/unary-expression/comments.js` leaves for `unknown` the same way — the
	// hunk `comment_position` matched is FIXED, and its residue is one cataloged divergence
	// no detector recognizes. Reasoning on `CORPUS_FORMAT_UNKNOWN_PIN`, which moves the other
	// way in the same step.
	//
	// 32 → 31: `js/assignment-comments/call.js` leaves for **match** — a fix, not a
	// reclassification, so nothing arrives anywhere. The assignment expression was the lone
	// dissenter among its own siblings at the operator→value gap: a line comment the author
	// glued to the operator (`a = // c⏎expr`) hung on its own line where the declarator and
	// the class field both trail it, and prettier trails at all three. Routing the
	// assignment's line arm through the shared `build_eq_comment_break_rhs` partition made
	// the family uniform. `CORPUS_FORMAT_UNKNOWN_PIN` does not move: this file left `partial`
	// by matching outright.
	//
	// 31 → 23: the eight clause-tail movers above — seven reclassify to `unknown` (their
	// explained hunks became matches; only an unexplained residue remains) and
	// `js/comments/break-continue-statements-2.js` leaves by matching outright. Reasoning on
	// `CORPUS_FORMAT_UNKNOWN_PIN`, which moves the other way in the same step.
	//
	// +1: `js/function/iife.js` arrives from `known` — prettier's own IIFE-comment
	// test file, and a reclassification rather than a loss. The same fix that moved
	// `issue-12967.js` to `match` (see `CORPUS_FORMAT_UNKNOWN_PIN`) re-cuts this file's
	// hunks: the leading and trailing runs now stay inside the callee's pair, which
	// leaves two long-standing behaviours in hunks of their own instead of folded into
	// neighbouring `comment_position` ones — the UNHONORED `prettier-ignore` at a callee
	// (pinned in `ignore_audit_known.txt`) and the own-line block prettier pulls up to
	// the `(` line. 16 of the file's 17 hunks are still explained.
	//
	// ⚠️ **24 is the re-measurement on the merged tip**, not 23 + 1: the two deltas above are
	// off different bases. It agrees with the sum only because the movers are disjoint, which
	// was checked per file (`js/function/iife.js` is in `partial`, `issue-12967.js` in
	// neither bucket) rather than inferred from the arithmetic.
	//
	// 24 → 25 (`bug469`, the post-arrow glued line comment): `js/arrows/issue-17421.js`
	// ARRIVES from `unknown` — the same single mover, counted here on the other side.
	// Reasoning on `CORPUS_FORMAT_UNKNOWN_PIN`, which moves the other way in the same step;
	// the arrival is a gain (the file's arrow hunks are now explained) held short of `known`
	// by an uncataloged block-body residue in the same file.
	typescript: 25,
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
 * bench:harvest:svelte-rejects — exact reject count. Measured 2026-08-24: ../svelte
 * at 5ccdfe355, ../prettier at 1dcd0b05d, ../prettier-plugin-svelte at 7809486,
 * oracle svelte@5.56.9, 145 of 4716 conformance-view Svelte files.
 * Fewer = the svelte/compiler oracle stopped rejecting (broken import/config);
 * more = it started rejecting wholesale — either way the cache would corrupt the
 * published coverage number. Re-derived by `bench:pins:suites` (see there).
 *
 * Moves with THREE checkout commits in {@link GATE_CHECKOUT_COMMITS}, not just the
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
 * ../svelte AND ../wpt checkout commits ({@link GATE_CHECKOUT_COMMITS} — the svelte
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
