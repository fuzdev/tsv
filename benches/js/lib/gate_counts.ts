/**
 * Pinned gate counts — committed EXPECTED numbers for the diagnostic gates and
 * harvests, so a change in what gets graded (a gutted or refreshed suite
 * checkout, a discovery bug, a tsv behavior change, a systemic sidecar/FFI
 * failure eating a whole language) fails loudly instead of shifting inside a
 * green run. This is `scripts/validate_artifacts.ts`'s tight-bounds philosophy
 * applied to counts: every real move in a number is a deliberate, visible edit.
 *
 * Pin categories are chosen per surface — exact pins, minimums and failure-bucket pins
 * (exact two-sided `!==`). What each means, which surface takes which, and why (the
 * pinned-snapshot rule, SAFETY gating over EVERY file) is stated once in
 * docs/gate_counts.md §Semantics; the per-constant docstrings below carry only what is
 * specific to that constant. The two corpus gates' counts are not constants here: they
 * are per-source rows in the generated `corpus_format_pins.txt` / `corpus_parse_pins.txt`
 * beside this module (`corpus_pins.ts`), rewritten by each gate's own `--update`.
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
 * against. Never re-pin to absorb an unexplained move; a failure-bucket-pin trip on an
 * `--all` run that also reports per-file `native format nondeterminism` or
 * empty-prettier-output errors can be the environment rather than the code
 * (benches/js/CLAUDE.md §Known Issues), so re-run before treating it as real.
 *
 * The Rust-side pins (test262 discovery + graded manifest, `fixtures_validate` fixture
 * count) live as consts in their commands — grep `REGRESSION PIN`. The as-authored
 * audits' formatted-file count is one shared const, `FIXTURES_FORMATTED_MIN` in
 * `crates/tsv_debug/src/audit/vacuity.rs`: they walk one corpus under one skip policy, so
 * a per-audit pin would only let their slack drift apart.
 */

import { CORPORA_ROOT, CORPORA_TREE } from './corpora.ts';

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
 * `../corpora` is the real-code snapshot (`fuzdev/corpora`): every `benched` and
 * `gated` corpus entry reads one of its collections, and one object id pins them
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
	// The real-code snapshot: every snapshot-tier corpus entry (`benched`, `gated`), so
	// every corpus pin plus the styles harvest measured over the perf view. Pinned by its
	// `collections/` tree (see above); what each collection vendors is its manifest.
	[CORPORA_ROOT]: {
		tree: CORPORA_TREE,
		hash: 'e5847b3dd',
		pins: [
			'corpus_format_pins.txt',
			'corpus_parse_pins.txt',
			'SVELTE_STYLES_BLOCKS_PIN',
			'STRIP_CORPUS_PINS'
		]
	},
	// `../svelte` feeds the conformance view alone (its `tests` tree); its
	// `packages/svelte/src` is the snapshot's `svelte` collection.
	'../svelte': {
		hash: '707c28146',
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
	// the CSS and corpus pins: of its 144 rejects, 40 come from ../prettier and 7
	// from ../prettier-plugin-svelte. A pin lists EVERY checkout it was measured
	// over, not just the one it is named after; `gate_counts_test.ts` grades that
	// each pin names at least one, which cannot see a missing second.
	'../prettier': {
		hash: '1dcd0b05d',
		pins: [
			'SVELTE_REJECTS_PIN',
			'CSS_REJECTS_PIN',
			'PRETTIER_JSX_PIN',
			'corpus_format_pins.txt',
			'corpus_parse_pins.txt'
		]
	},
	'../prettier-plugin-svelte': {
		hash: '7809486',
		pins: ['SVELTE_REJECTS_PIN', 'corpus_format_pins.txt', 'corpus_parse_pins.txt']
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
	// version moving — the version-window this file's header describes.
	//
	// 3406 → 3445 scanned, 3308 → 3350 both-accept, 17 → 16 over-acceptance: the checkout
	// moved to the oracle's own release tag, adding 39 graded inputs, and the oracle moved
	// with it. That retired the one oracle-skew over-acceptance (`css-nth-of-minified`, a fix
	// the checkout carried ahead of the pinned oracle) and turned the two
	// namespaced-type-selector inputs (`svg|*`, `*|*`) from parity rejects into both-accepts,
	// once tsv parsed them too.
	//
	// 16 → 13 over-acceptance: the root-only meta tag rule (`svelte_meta_duplicate` /
	// `svelte_meta_invalid_placement`) is a parse error in tsv as it is in the oracle, so
	// `compiler-errors/samples/window-{duplicate,inside-block,inside-element}` are parity
	// rejects.
	//
	// 3445 → 3469 scanned, 3350 → 3372 both-accept: the checkout and the oracle moved together
	// to the next release tag, adding 24 graded inputs. Two are that release's own new parse
	// errors — a second `{:else}`, and an `{:else if}` after an `{:else}`
	// (`compiler-errors/samples/{each-block-duplicate-else,if-block-else-before-else-if}`) —
	// which tsv rejects too, so they are parity rejects; the other 22 parse in both.
	//
	// 3469 → 3470 scanned, 3372 → 3373 both-accept: the checkout moved two commits past the
	// oracle's release tag (the declared version unchanged — the version window this file's
	// header describes), adding one graded input,
	// `runtime-runes/samples/async-fork-discard-after-in-check/main.svelte`, which both parse;
	// the pull's other commit touched no suite input. Over-acceptance unchanged.
	scanned: 3470,
	both_accept: 3373,
	over_acceptance: 13
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
 *
 * 12284 → 12283 `accept_parity`: a net of one from the for-head target rule moving to
 * tsc's checker line — `referenceSatisfiesExpression.ts` (a bare cast target in a
 * for-of head) joined parity, while `propertyAccessChain.3.ts` and
 * `elementAccessChain.3.ts` left it, their optional-chain for-heads (TS2780 / TS2781)
 * now rejected and sanctioned under `grammar` — conformance_svelte.md §TypeScript
 * Corrections, the cast_target_for_head entry.
 *
 * 12283 → 12282 `accept_parity`: `parserForStatement4.ts` (`for (a = 1 in b)`) left it
 * for the `grammar` sanctions when the whole target of an `=` / for-head stopped
 * converting to an `AssignmentPattern` — a node that exists only as a pattern child,
 * so the head had no wire shape (acorn rejects it too); every other bucket unmoved.
 *
 * 20 → 19 `over_acceptance_parser`: `castOfYield.ts` (`<number> yield 0`, TS1109) left it
 * for reject parity — it got better — when a bare `yield` or arrow function stopped being
 * accepted as an operand (an `AssignmentExpression` is no `UnaryExpression`); the full-corpus
 * A/B moved no other TypeScript file, and every other bucket is unmoved.
 *
 * 12282 → 12284 `accept_parity`: `usingDeclarationsInFor.ts` and
 * `awaitUsingDeclarationsInFor.ts` joined it from the beyond-acorn known gaps when a
 * `using` / `await using` declaration became a C-style for-head init — the
 * `LexicalDeclaration` production `let` already took there (conformance_svelte.md
 * §TypeScript Corrections, the for_head_c_style entry); every other bucket unmoved.
 *
 * 467 → 468 `over_acceptance_checker`: `usingDeclarationsInForOf.4.ts` left reject
 * parity when a C-style head's using declaration could bind `of` (`for (using of =
 * null;;)`) — tsc's parser takes all three of its heads, and the baseline's one error
 * is the checker's TS1155 on `for (using of;;)`, the missing initializer tsv defers
 * like `const x;`'s (conformance_svelte.md §TypeScript Corrections, the
 * for_head_c_style_of_binding entry); every other bucket unmoved.
 *
 * 19 → 16 `over_acceptance_parser`: `incrementAndDecrement.ts` (`x++ = 4`),
 * `awaitUsingDeclarations.4.ts` (`await using [a] = null`, read as `await (using[a])`)
 * and `privateNameInInExpressionTransform.ts` (`'prop' in v = 10`) left it for reject
 * parity — each TS1005 in tsc's parser — when a bare operator expression stopped being
 * accepted as an assignment's left (no `LeftHandSideExpression` derives it; conformance_svelte.md
 * §TypeScript Corrections, the non-simple assignment target entry). A head-vs-new parse A/B
 * over all of tests/cases moved no other file, and every other bucket is unmoved.
 *
 * 468 → 467 `over_acceptance_checker`: `decoratorOnClassConstructor1.ts`
 * (`@dec constructor() {}`, TS1206) left it for reject parity when a decorated
 * constructor implementation became a parse error, as acorn's; every other bucket
 * unmoved.
 *
 * 16 → 14 `over_acceptance_parser`: `classExtendingPrimitive.ts` and
 * `classExtendingPrimitive2.ts` (`class C4a extends void {}`, TS1109) left it for reject
 * parity when `void` stopped reading as a name at a primary (it is the unary operator,
 * an ECMAScript ReservedWord); every other bucket unmoved.
 */
export const TS_REPO_PINS = {
	scanned: 13708,
	accept_parity: 12284,
	over_acceptance_parser: 14,
	over_acceptance_checker: 467,
	units_scanned: 7874,
	units_accept_parity: 7858
};

/**
 * bench:harvest:svelte-styles — EXACT extracted `<style>` block count over the perf
 * view, i.e. the `../corpora` snapshot's `.svelte` files. Pure input material (not a
 * tsv success count), and pinned like the suite harvests: the source is a pinned
 * snapshot, so a move is a snapshot refresh (re-pin with the new `collections/` tree
 * id), a collection joining the bench, or a broken extraction, and the harvest fails
 * BEFORE writing so a wrong cache never replaces a good one. Stamped on the snapshot's
 * `collections/` tree id and the perf view's entry list (`lib/harvest_stamp.ts`).
 * Measured 2026-10-17: ../corpora `collections/` at e5847b3dd.
 *
 * 401 → 619: the third-party collections join the bench, so the perf view gains their
 * `.svelte` blocks with the snapshot's tree id unmoved (layercake, layerchart,
 * svelte-maplibre, svelte-ux and flowbite-svelte's `src/lib`; language-tools carries
 * none), the `lang="postcss"` blocks skipped.
 *
 * 619 → 615: fuz_mastodon leaves the bench, taking its 4 blocks.
 */
export const SVELTE_STYLES_BLOCKS_PIN = 615;

/**
 * strip:corpus — exact counts over the strippable TypeScript of the `gates` view's snapshot
 * tiers (`benches/js/strip_corpus.ts`): `files` graded and `refusals`, the files tsv refuses
 * as ts-blank-space does. A move in `files` is a snapshot refresh, a collection joining a
 * tier, or a broken filter; `refusals` moves with the snapshot, or with a verdict change on
 * both sides at once (a ts-blank-space bump), the one move the per-file agreement cannot
 * see. Measured over ../corpora `collections/`, ts-blank-space 0.9.0.
 *
 * files 4852 → 4844: fuz_mastodon leaves the corpus, taking its 8 strippable files.
 */
export const STRIP_CORPUS_PINS = { files: 4844, refusals: 175 };

/**
 * bench:harvest:wpt — exact `<style>` blocks from the default `../wpt/css`. Measured
 * 2026-09-22: ../wpt at 7437c7bc, the same checkout as the previous 22,310 — the move is
 * the harvest's own: it now skips `<style>` tags inside `<script>` blocks — 202 of them,
 * JS string literals rather than stylesheets, 183 of which the cache had been writing
 * (the rest were duplicates or empty, skipped either way).
 */
export const WPT_CSS_HARVEST_PIN = 22_127;

/**
 * bench:harvest:test262 — exact expected-positive files in the cache list. Measured 2026-09-08: ../test262 at 7153986f (48,274 graded).
 * Mirrors the Rust `POSITIVE_PASSED_PIN` (crates/tsv_debug/src/cli/commands/test262.rs) that the
 * `conformance:test262` release gate enforces — same positive count, keep the two in lockstep on a test262 pull.
 */
export const TEST262_POSITIVES_PIN = 43_739;

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
 * bench:harvest:prettier-jsx — exact count of the conformance view's Prettier `.js`
 * fixtures that Prettier's own babel parser reads as JSX (`ast_has_jsx`), the
 * out-of-scope set the view drops. Measured 2026-10-06: ../prettier at 1dcd0b05d,
 * oracle prettier@3.9.9, 39 of the 795 JS-suite files the validity filter keeps.
 * Fewer = the oracle stopped seeing JSX (a broken import, or babel's JSX plugin off);
 * more = it started reading plain JS as JSX — either way the cache would move a
 * number every parser shares. Re-derived by `bench:pins:suites` (see there).
 */
export const PRETTIER_JSX_PIN = 39;

/**
 * bench:harvest:svelte-rejects — exact reject count. Measured 2026-10-15: ../svelte
 * at 707c28146, ../prettier at 1dcd0b05d, ../prettier-plugin-svelte at 7809486,
 * oracle svelte@5.57.2, 144 of 4788 conformance-view Svelte files (the count held
 * across the svelte checkout's two-commit move past the release tag: its one new
 * suite input parses).
 * Fewer = the svelte/compiler oracle stopped rejecting (broken import/config);
 * more = it started rejecting wholesale — either way the cache would corrupt the
 * published coverage number. Re-derived by `bench:pins:suites` (see there).
 *
 * Moves with THREE checkout commits in {@link GATE_CHECKOUT_IDS}, not just the
 * one it is named after: the Svelte-language conformance corpus is the svelte
 * suite plus both prettier suites' `.html` (which the loader reads as Svelte), and
 * the split of the 144 is 97 / 40 / 7. The harvest stamps all three, so a pull of
 * any of them re-grades this pin rather than leaving it describing the previous
 * corpus.
 *
 * 145 → 142: the oracle moved to svelte@5.57.0, whose CSS parser accepts namespaced
 * type selectors (`svg|*`, `*|*`) and `nth-child`'s `of` with no whitespace after it —
 * the suite's own fixtures for those fixes (`css/samples/namespaced-type-selector`,
 * `print/samples/css-namespaced-type-selector`, `parser-modern/samples/css-nth-of-minified`)
 * left the list. The same pull's 47 new suite inputs all parse.
 *
 * 142 → 144: the oracle moved to svelte@5.57.2, which rejects a block continuation landing
 * on a slot already taken — a second `{:else}`, an `{:else if}` after an `{:else}` — where
 * it used to overwrite the first branch. The suite's two fixtures for that error
 * (`compiler-errors/samples/each-block-duplicate-else`, `…/if-block-else-before-else-if`)
 * joined the list; the pull's other new suite inputs all parse.
 */
export const SVELTE_REJECTS_PIN = 144;

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
 * too, and stamps the same three checkout commits. Measured 2026-10-15: ../prettier at 1dcd0b05d, ../svelte at 707c28146,
 * ../wpt at 7437c7bc7, oracle svelte@5.57.2, 207 of 22450 conformance-view CSS files (the
 * count held across the oracle move to that version and across the svelte checkout's
 * later two-commit move past it, which added no CSS file).
 *
 * 229 → 207 with no checkout or oracle moving: the conformance view's own filters
 * tightened. The Prettier CSS suite lost its front-matter (`css/yaml/`, all but the
 * one file whose fence never closes — no front matter by Prettier's rule, and
 * parseCss accepts it) and range-marker fixtures (`lib/prettier_fixtures.ts`), nine
 * of them rejects, and the
 * wpt harvest stopped lifting `<style>` tags out of `<script>` strings
 * ({@link WPT_CSS_HARVEST_PIN}), thirteen of those rejects. Every file that left was
 * invalid for every parser by construction, so the remaining rejects are the
 * oracle's verdicts on stylesheets.
 *
 * 240 → 229: the oracle moved to svelte@5.57.0, the same CSS-parser fixes that moved
 * {@link SVELTE_REJECTS_PIN} (a single upstream fix lands in both counts, so a change that
 * moves one of these pins should expect to move the other). The eleven that left are
 * `css/samples/namespaced-type-selector/expected.css` and ten wpt-css files, each a
 * namespaced universal selector or an `nth-child` `of` glued to its selector
 * (`css-shadow/css-scoping-shadow-host-namespace`, `selectors/is-default-ns-*`,
 * `selectors/not-default-ns-*`, `selectors/nth-child-of-universal-selector`, …). The
 * checkout's one new CSS file parses.
 */
export const CSS_REJECTS_PIN = 207;
