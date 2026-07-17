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

/**
 * conformance:svelte-fixtures — measured 2026-07-09: ../svelte at 8fb7ceeba (svelte@5.56.4), oracle svelte@5.56.4.
 * both_accept 3277→3280: a top-level leading combinator (`> span {}`, `+ p {}`, and the `@media`-body form)
 * now parses — tsv's CSS selector parser accepts a leading combinator in every context, matching parseCss
 * (spec-invalidity deferred to diagnostics; see selectors.rs `parse_complex_selector`). The three
 * `validator/samples/css-invalid-combinator-selector-{1,2,3}` fixtures move from over-rejection to
 * both-accept (the `-4` trailing-combinator form stays parity — both reject). Last Svelte KNOWN_GAP closed.
 * both_accept 3276→3277: legacy `<!-- -->` HTML comments (CDO/CDC) in `<style>` now parse (css-cdo-cdc gap
 * fixed — parseCss swallows the marker span; tsv matches), so `css/samples/comment-html` moved from
 * over-rejection to both-accept.
 * both_accept 3274→3276: whitespace after `{` before a block/tag marker (`{ #if}`) now parses
 * (svelte-block-ws gap fixed — the `if-block-whitespace-{legacy,runes}` pair moves to both-accept).
 * both_accept 3273→3274: leading-symbol attribute names (`<p }>`) now parse (svelte-attr-name gap fixed),
 * so `validator/samples/attribute-invalid-name` moved from over-rejection to both-accept.
 */
export const SVELTE_FIXTURES_PINS: GatePins = { scanned: 3378, both_accept: 3283 };
// scanned 3375→3378, both_accept 3280→3283 (2026-07-13, ../svelte b4d1583ae, svelte@5.56.4,
// oracle svelte@5.56.4). NO tsv behavior change — `scanned` is a raw count of the suite's
// canonical `.svelte` inputs, so no code change can move it. The suite grew by exactly three
// inputs as ../svelte advanced 8fb7ceeba→b4d1583ae, all within the same declared 5.56.4
// (which is why the version-string check stayed green and the count pin is what caught it —
// pins:audit now compares the checkout COMMIT as well):
//
// - `runtime-runes/samples/async-batch-derived/main.svelte` (svelte bfbb026f2, #18525)
// - `runtime-runes/samples/each-keyed-computed-destructuring-key/main.svelte` (b4d1583ae, #18521)
// - `sourcemaps/samples/sourcemap-empty-source/input.svelte` (5edd8b060, #18518)
//
// A fourth new file, `each-keyed-computed-destructuring-key/Child.svelte`, is NOT counted —
// `input_basenames` is {input,main,index}.svelte. All three are ordinary valid Svelte 5
// components, so both parsers accept all three and both_accept moves by the same +3. Verdict
// parity is unchanged and clean: 0 unexpected over-rejections, 0 known gaps, 0 undocumented
// AST-shape groups.

/** conformance:ts-fixtures — measured 2026-07-08: ../acorn-typescript at 312d079 (v1.0.11), oracle @sveltejs/acorn-typescript@1.0.11. */
export const TS_FIXTURES_PINS: GatePins = { scanned: 207, both_accept: 186 };

/**
 * conformance:ts-repo — measured 2026-07-06: ../typescript at 637d5746b.
 * accept_parity 424→423 (2026-07-09, branch bug3): the type-member signature-param grammar fix
 * now rejects a parameter-property / default in a signature (acorn's tsParseBindingListForSignature).
 * `ParameterLists/parserParameterList13.ts` (`interface I { new (public x) }`) is flagged TS2369 — a
 * tsc *checker* error (TS2xxx), so tsc's parser accepts it; acorn rejects it at parse and tsv now
 * matches, moving it accept-parity → gap-beyond-acorn (case ii: tsv+acorn correctly reject at parse
 * what tsc flags later as a semantic error). over-acceptance unchanged (the corpus has no tsc-TS1xxx
 * form of these rules).
 * accept_parity 423→429 (2026-07-11, branch bug77): recording the over-rejection fixes landed since
 * bug3 (the triage-batch non-simple-assignment target + keyword-as-type accepts, the #406 type-args
 * follow-token disambiguation) — the only parser-accept-widening changes in that window — which moved
 * 6 tsc-valid files gap-beyond-acorn → accept-parity. Checkout UNCHANGED at 637d5746b (still the
 * 2026-07-06 measurement point), so a rise on a pinned corpus is pure parity gain, not a suite
 * refresh; gap_unexpected/gap_known both 0. gap-beyond-acorn now 7, all correctly rejected (the 3
 * param-property-in-signature cases above + the for-in-LHS `for(foo() in b)` / numeric-enum-member
 * quartet, which prettier ALSO rejects — so tsv siding with acorn is right there too).
 */
export const TS_REPO_PINS = { scanned: 768, accept_parity: 429 };

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
 *
 * css 5→3 (2026-07-10): the nested-paren unquoted-url parse fix closed TWO files.
 * An unquoted `url()` whose content contains a nested `(` (e.g.
 * `url(--var(foo-bar,#dadce0))`, `url( var(x) )`) truncates to the first unescaped
 * `)` (css-syntax §4.3.6), leaving a dangling `)` that drove the declaration-value
 * loop's paren depth negative and over-rejected with `Expected }`. Fixing the depth
 * clamp closed `inline-url/inline_url.css` and `loose/loose.css` (the latter's
 * `url( var(…) )` hit the same bug — it was mislabeled a `calc (…)` error-recovery
 * gap). Both oracles accept both files.
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
 * regression, never live-corpus growth.
 *
 * Measured 2026-07-16 over the reproducible subset at the pinned checkouts (../svelte
 * b4d1583ae, ../kit da5b08ea7, ../svelte.dev c21c2d0f0, ../prettier 1dcd0b05d,
 * ../prettier-plugin-svelte 7809486; oracle acorn-typescript 1.0.11; SAFETY 0). This re-pin
 * unblocked the RED 0.2 gate: the old pins (svelte 1103 / ts 4172 / css 125) were the
 * AGGREGATE over live+framework and drifted with dev-repo churn (re-pinned 3× in 2 days, and
 * the pin commit couldn't reproduce its own number). Split rationale: benches/js/CLAUDE.md
 * §Pinned gate counts.
 */
export const CORPUS_FORMAT_MATCH_MIN: Record<Language, number> = {
	svelte: 530,
	typescript: 2332,
	css: 90,
};
// ─── SUPERSEDED HISTORY (kept for the attribution trail, NOT current guidance) ───
// Everything below described the OLD AGGREGATE pin (over live+framework), retired by the
// 2026-07-16 reproducible-subset split. The very churn it narrates — "both rises are
// live-corpus CHURN", "the pre-change binary measures … on today's checkouts too" — is
// exactly what the split removed; the current pins can't move on dev-repo churn.
// svelte 1101→1103 + typescript 4170→4172 (2026-07-14, checkouts as in
// CORPUS_FORMAT_UNKNOWN_PIN below; css 125 unchanged, SAFETY 0). Both rises are live-corpus
// CHURN, zero behavior: the pre-change binary (5d0789b3, the commit these were last pinned
// at) measures svelte 1103 / typescript 4172 on today's checkouts too — i.e. the whole match
// bucket is byte-identical between the two binaries, so nothing in #463–#469 moved it. The
// pinned CHECKOUTS are unchanged (pins:audit confirms), but the live dev repos the corpus
// also reads are not tracked by GATE_CHECKOUT_COMMITS and moved since the pin. Re-pinned UP
// to keep the floor tight per the rule above. #463's two prettier-suite fixes did NOT touch
// match — they moved `partial`/`unknown`→`known` (see the two pins below).
// typescript 4168→4170 (2026-07-14, ../svelte b4d1583ae, ../kit da5b08ea7, ../svelte.dev
// c21c2d0f0, ../prettier 1dcd0b05d, ../prettier-plugin-svelte 7809486, oracle
// acorn-typescript 1.0.11); svelte + css unchanged, SAFETY 0. The checkouts are UNCHANGED
// from the 2026-07-13 pin (pins:audit's commit check confirms — zero corpus drift), so the
// +2 is behavior: #459 "own every glued block comment" landed after the pin. Net +2 match on
// TS because its call/chain/new layout gates now see owned comments (more expand/hug matches)
// and outweigh the new preserve-divergences (a leading block comment kept glued where
// prettier hoists it out). This branch's follow-ups have ~0 TS-corpus effect (the
// svelte-destructure comment-preservation fix is svelte-only — svelte match holds at 1101 —
// and the M4 unary-assignment leading-comment fix is exercised by no corpus file). A per-file
// worktree diff was not re-run this session; the cause is #459 and the move is small +
// SAFETY 0.
// typescript 4164→4168 (2026-07-13, ../svelte b4d1583ae, ../kit da5b08ea7, ../svelte.dev
// c21c2d0f0, ../prettier 1dcd0b05d, ../prettier-plugin-svelte 7809486, oracle
// acorn-typescript 1.0.11); svelte + css unchanged, SAFETY 0.
//
// NOT a clean +4: on today's checkouts the PRE-change binary measures 4162, i.e. the 4164
// pin was already stale by −2 from live-corpus movement — ../kit (5c38e515d→da5b08ea7) and
// ../svelte.dev (93a400d→c21c2d0f0) both moved since the pin was taken (../svelte and
// ../prettier did not). That drift is a CORPUS change, not a tsv regression: the HEAD binary
// reproduces 4162 on today's checkouts. The pins:audit commit check added alongside this
// exists so that kind of movement stops being silent.
//
// From that 4162 baseline the computed-lookup work is a clean +6, fully attributed by
// formatting the whole `gates` corpus with the pre-change binary and with this one and
// diffing the per-file output sets (only 8 files change output at all; 6 of them flip into
// `match`, −3 `known` / −3 `unknown`):
//
// - +3 (`known`→`match`), ../svelte: `src/index-client.js`, `src/legacy/legacy-client.js`,
//   `src/internal/client/dom/legacy/misc.js` — a computed lookup on a JSDoc-cast base no
//   longer breaks before `?.[`.
// - +2 (`unknown`→`match`), ../prettier suites: `js/assignment/issue-15534`,
//   `js/method-chain/assignment-lhs` — an unbreakable template RHS now breaks after the `=`
//   when the LHS can break, instead of splitting the assignment target.
// - +1 (`unknown`→`match`), ../kit: `src/runtime/server/page/data_serializer.js` — same
//   assignment rule.
//
// The other 2 files that move stay divergent and so don't touch a count:
// `js/binary-expressions/comment` and `js/ternaries/indent-after-paren` (both closer to
// prettier — the brackets now break as prettier's do — but each retains a separate,
// pre-existing binary/ternary indent divergence).

/**
 * corpus:compare:format --all — EXACT per-language `unknown` divergence count over the
 * REPRODUCIBLE subset (framework + prettier suites; see `CORPUS_FORMAT_MATCH_MIN`). Both
 * directions fail: a rise = a new unexplained divergence (fix it, catalog a detector in
 * `lib/divergence/patterns.ts`, or consciously re-pin a legitimately-unsupported new pinned
 * suite file); a drop = the backlog shrank, re-pin to record the win. Live dev-repo unknowns
 * are the non-gating WARN, not here. A single-run trip can be the FFI/sidecar heisenbug —
 * confirm on the single repo first. Measured 2026-07-17 (same checkouts + attribution as
 * `CORPUS_FORMAT_MATCH_MIN`; SAFETY 0).
 */
export const CORPUS_FORMAT_UNKNOWN_PIN: Record<Language, number> = {
	svelte: 7,
	typescript: 133,
	css: 22,
};
// typescript 136→133 (2026-07-17, ../svelte b4d1583ae, ../kit da5b08ea7, ../svelte.dev
// c21c2d0, ../prettier 1dcd0b0; svelte 7 + css 22 unchanged, SAFETY 0). The
// conditional/parenthesized-type body-indent fix (bug141 §Bug 2): two files move
// unknown→match — ../kit src/types/private.d.ts (a mapped type in a conditional true-branch)
// and ../svelte src/server/index.d.ts (a tuple in a conditional branch). Both had a
// broken branch VALUE one indent level short of prettier's `printBranch` (`indent(branch)`
// under useTabs); now converged. A whole-corpus pre/post per-file bucket diff confirmed
// exactly these two left, ZERO files entered unknown, SAFETY 0. (svelte/index.d.ts, the
// third file, was a `partial` — see CORPUS_FORMAT_PARTIAL_PIN.)
// ─── SUPERSEDED HISTORY (attribution trail; NOT current — pre the 2026-07-16 reproducible
// split, these were the AGGREGATE over live+framework and moved on dev-repo churn) ───
// typescript 139→140 (2026-07-14, checkouts unchanged; svelte 7 + css 22 unchanged, SAFETY 0).
// A rise, but not a new divergence: `js/comments/return-statement-2.js` moves `known`→`unknown`
// because its diff SHRANK. It carried the `return`/`throw` leading-comment ASI bug — tsv broke
// between the keyword and its argument, which is a restricted production, so the output returned
// `undefined` (and `throw` was a syntax error). Fixing that leaves only tsv's sequence-operand
// parens (`a, b` → `(a, b)`), which no detector claims — the same residual as the other
// sequence-paren unknowns (`js/arrows/issue-14702`, `js/sequence-expression/*`), so this file
// simply joins that existing class. Consciously re-pinned rather than pattern-matched: a detector
// for the sequence-paren class would cover all four at once and belongs with that divergence, not
// with an ASI fix.
//
// ⚠️ Why the bug hid here: `comment_position` — a deliberately broad detector ("tsv preserves a
// comment where prettier relocates it") — matched the ASI-corrupted output and classified the file
// `known`. A pattern that broad can mask a semantic-corruption bug, which is exactly what it did
// for as long as this file sat in `known`.
// typescript 140→139 (2026-07-14, ../svelte b4d1583ae, ../kit da5b08ea7, ../svelte.dev
// c21c2d0f0, ../prettier 1dcd0b05d, ../prettier-plugin-svelte 7809486, oracle
// acorn-typescript 1.0.11); svelte 7 + css 22 unchanged, SAFETY 0. Checkouts UNCHANGED from
// the 2026-07-13 pin (pins:audit's commit check confirms — zero corpus drift), so the −1 is
// behavior. Attributed by formatting the whole corpus with the pre-change binary (5d0789b3,
// the commit this file was last pinned at) and diffing per-file bucket membership: exactly
// one file leaves, none enter — `js/comments/assignment-pattern.js`, `unknown`→`known`. It
// is one of the four files #463 fixed (the line-comment-swallow class): the name/key→default
// `=` gap now breaks the comment onto its own line instead of swallowing the `=`, and the
// `destructuring/default_equals_line_comment` divergence added with it is what the detector
// now matches — so the file is a cataloged divergence rather than an unexplained diff.
// typescript 139→140 (2026-07-14, checkouts as in CORPUS_FORMAT_MATCH_MIN above; svelte 7 +
// css 22 unchanged, SAFETY 0). Checkouts UNCHANGED from the 2026-07-13 pin (zero corpus
// drift), so the +1 is behavior from #459 "own every glued block comment": a glued block
// comment now shifts fill wrapping where it fuses into a fill item — `js/arrays/numbers3.js`
// (its inline `/*21,*/` array comment re-flows the number fill by one column), verified no
// data loss, reparses, idempotent. A deliberate comment-preservation divergence, not a
// regression (see conformance_prettier.md §Comment relocation / §Comment Position Philosophy).
// typescript 141→139 (2026-07-13, checkouts as in CORPUS_FORMAT_MATCH_MIN above). The
// pre-change binary measures 142 on today's checkouts — the 141 pin was stale by +1 from
// the same ../kit + ../svelte.dev movement — and the computed-lookup work then removes 3
// (`issue-15534`, `method-chain/assignment-lhs`, kit's `data_serializer.js`, all
// `unknown`→`match`). See the attribution block on CORPUS_FORMAT_MATCH_MIN.
// typescript 142→141 (2026-07-13, ../svelte b4d1583ae, ../kit 5c38e515d, ../svelte.dev
// 93a400d, ../prettier 1dcd0b0, ../prettier-plugin-svelte 7809486, oracle
// acorn-typescript 1.0.11). Net −1, but it is NOT a clean −1: on today's checkouts the
// PRE-change binary measures 143, i.e. the 142 pin was already stale by +1 from live-corpus
// growth. The empty-paren dangling-comment fix then removes 2, landing at 141. A `//`
// comment alone inside an empty argument or parameter list was emitted INLINE
// (`fn(// c);`), so it ran to end-of-line and swallowed the `)` — output that did not
// re-parse. Every empty paren list (call, `new`, member-chain, function/method/arrow
// params, signature type params) now shares the bracket/brace dangling emitter, so a line
// comment breaks the list and a fitting block comment still hugs. (The sibling swallow in
// CALLEE position — `call // c⏎()` → `call // c();` — is a different mechanism and is NOT
// fixed here; see the TODO in tsv_ts/src/printer/comments/lists.rs.) svelte 7 and css 22
// unchanged.
// typescript 144→142 (2026-07-12, ../prettier 1dcd0b0, ../svelte 8fb7ceeba, ../kit
// 1b4adccf7, ../svelte.dev fb5a4e2, ../prettier-plugin-svelte 7809486, oracle
// acorn-typescript 1.0.11): the avoid-becoming-a-directive parens fix — Prettier's
// `needs-parentheses.js` wraps a bare non-directive string-literal statement in
// parens whenever its immediate container is a `Program` or `BlockStatement`
// (recomputed fresh, so redundant source parens in an ineligible container like a
// `StaticBlock`/`SwitchCase`/`TSModuleBlock` get stripped too); tsv only preserved
// existing source parens and never added or stripped them.
// `build_expression_statement_doc` (statements/mod.rs) now applies
// `needs_avoid_directive_parens`, threaded via a new `in_program_or_block: bool`
// parameter through `build_statement_doc` and the block-building chain. Moved two
// files unknown→match: js/directives/issue-7346.js, js/quotes/strings.js. A
// before/after --all path-diff (via a `git worktree` at the pre-fix commit)
// confirmed exactly these two moved, 0 files ENTERED unknown, SAFETY 0. svelte/css
// unmoved.
// typescript 146→144 (2026-07-12, ../prettier 1dcd0b0, ../svelte 8fb7ceeba, ../kit
// 1b4adccf7, ../svelte.dev fb5a4e2, ../prettier-plugin-svelte 7809486, oracle
// acorn-typescript 1.0.11): the catch-block-collapse-ignores-finally fix —
// Prettier's `block.js` only collapses an empty `catch {}` to one line when the
// `TryStatement` has no `finally` (`parent.type === "CatchClause" &&
// !parentParent.finalizer`); when `finally` follows, catch expands to `{\n}`
// just like `try`/`finally` always do. `build_try_statement_doc` in
// try_jump.rs now checks `stmt.finalizer.is_some()` instead of always
// collapsing. Moved two files unknown→match:
// js/optional-catch-binding/optional_catch_binding.js, js/try/empty.js. A
// before/after --all path-diff (via a `git worktree` at the pre-both-fixes
// commit) confirmed exactly seven files moved total across both fixes in this
// branch (the five from the empty-statement-list pin note above + these two),
// 0 files ENTERED unknown, SAFETY 0. svelte/css unmoved.
// typescript 151→146 (2026-07-12, ../prettier 1dcd0b0, ../svelte 8fb7ceeba, ../kit
// 1b4adccf7, ../svelte.dev fb5a4e2, ../prettier-plugin-svelte 7809486, oracle
// acorn-typescript 1.0.11): the standalone-EmptyStatement drop fix — Prettier's
// `printStatementSequence` never prints a bare `;` in ANY statement-list body
// (Program, BlockStatement, StaticBlock, SwitchCase, TSModuleBlock), dropping it
// even when it's the body's only content (which then collapses/expands like a
// genuinely empty body); tsv previously only did this at Program level
// (`build_statement_list_docs_into` in blocks.rs now skips it — shared by block
// bodies and namespace/module bodies — plus the switch-case consequent loop in
// switch.rs, and the two "is this body empty" gates widened to
// `all(EmptyStatement)` in blocks.rs + type_declarations.rs) — moved five files
// unknown→match: js/empty-statement/no-newline.js, js/switch/empty_lines.js,
// js/switch/empty_statement.js, js/switch/empty_switch.js, js/try/try.js. A
// before/after --all path-diff (via a `git worktree` at the pre-fix commit) confirmed
// exactly these five moved, 0 files ENTERED unknown, SAFETY 0. svelte/css unmoved
// (this fix is TS-parser-only; no corpus file exercises the pattern through the
// Svelte `<script>` embedding path).
// typescript 173→151 (2026-07-12, ../prettier 1dcd0b0, ../svelte 8fb7ceeba, ../kit
// 1b4adccf7, ../svelte.dev fb5a4e2, ../prettier-plugin-svelte 7809486, oracle
// acorn-typescript 1.0.11): the un-triaged divergence backlog shrank. The pinned prettier
// suite + framework checkouts are UNCHANGED, so every prettier-suite move is deterministic
// tsv behavior, in two waves. (1) 173→154 — the already-landed #414/#415/#417 numeric-literal
// strictness rejects sloppy-mode files (js/non-strict/octal-number.js,
// js/numeric-separators/number.js) unknown→error (tsv is strict-mode-only by design), and
// #418–#422 fixes moved others unknown→match. (2) 154→151 — this branch's two formatter fixes
// moved three more unknown→match: the `for` init-only spacing fix (js/for/in.js) and the
// empty-statement-position-block expand fix (js/module-blocks/non-module-blocks.js,
// typescript/nosemi/functions.ts). A before/after --all path-diff confirmed exactly these
// moved, 0 files ENTERED unknown, SAFETY 0. The remaining 151 unknowns are all
// line-break/wrap style divergences (no data loss). (Files that moved to `error` are
// tsv-correct strict-mode rejects; the format `error`/`expected_errors` buckets are unpinned.)
// typescript 177→173 (2026-07-11, ../prettier 1dcd0b0, oracle acorn-typescript 1.0.11): the
// unary/update-argument parenthesization fix (a `+`/`-` operand that would re-tokenize — `+(+x)`,
// `-(--x)` — and a type-assertion operand of an update — `(a as T)++` — now keep their parens;
// needs_parens.rs UnaryArgument same-sign rule + a new UpdateArgument context) resolved four
// prettier-suite files that previously emitted invalid, non-reparseable output (unknown→match):
// js/unary/series.js, js/unary-expression/urnary_expression.js, typescript/as/as.ts,
// typescript/update-expression/update-expressions.ts. A before/after --all diff on the identical
// corpus confirmed exactly those four moved (0 new unknowns, 0 new errors, SAFETY 0). The pinned
// svelte/css match minimums (1108<1111, 125<126) are pre-existing live-repo churn, NOT re-pinned.
// css 23→22 (2026-07-10): `prettier/tests/format/css/url/url.css` moved unknown→partial.
// The value-parser escaped-paren fix (an escaped `\(`/`\)` is content per css-syntax §4.3.7,
// not a nesting delimiter — value/parser.rs fast_scan + its ValueCursor/classify_separators
// twins) stopped mis-counting the escaped `)` in this file's `url(  …\)\).jpg  )` tokens: they
// now trim cleanly (§4.3.6) and its multi-`url()` `background:` list wraps with consistent tabs
// instead of the prior mixed-indent garble. The wrap is now a plain fill_101_boundary divergence
// (→ partial); the residual unexplained hunks are the escaped-paren url outer-whitespace
// divergence (tsv trims, prettier keeps — url_escaped_paren_ws_prettier_divergence), which is
// variant-only so it has no corpus detector. SAFETY 0. See the partial-pin note below.
// css 24→23 (2026-07-10): `prettier/tests/format/css/loose/loose.css` moved
// unknown→known. The new `css_url_opaque` divergence detector
// (lib/divergence/patterns.ts) now explains its sole hunk —
// `url(var( x ))` → prettier `url(var(x))`: tsv keeps unquoted-url content
// VERBATIM (opaque <url-token>, css-syntax §4.3.6) where prettier reformats
// inside the nested parens (conformance_prettier.md §CSS: Values, fixture
// url_nested_reformat_prettier_divergence). SAFETY 0. Pairs with the
// `inline_url.css` partial→known move below.
// typescript 181→177 (2026-07-09): the class-member modifier line-break fix (a contextual
// TS modifier / `accessor` / `async` separated from its member by a line break is the member,
// not a modifier — the `[no LineTerminator here]` guard in parser/statement/class.rs) resolved
// the two `prettier/tests/format/typescript/**/decorator-auto-accessors-new-line.ts` `accessor⏎`
// hunks (unknown→match). A before/after --all diff on the identical corpus confirmed exactly
// those two files moved (0 new unknowns, 0 new errors, SAFETY 0); the pinned framework checkouts
// (svelte 8fb7ceeba, kit 1b4adccf7, svelte.dev fb5a4e2, prettier-plugin-svelte 7809486) are
// unchanged, so the 181→179 remainder is pre-existing real-tier/prettier-suite churn since the
// 2026-07-08 pin, folded in here per the "re-pin to current" ritual.
// typescript 65→64 (2026-07-08): the single-object-type-param signature hug
// (bodyless declare/overload + method/call/construct signatures now hug `(o: {`
// like value-param functions — `build_signature_params_doc`) resolved the residual
// unexplained hunk in kit/src/types/internal.d.ts, so it moved partial→known (its
// tabs_only_alignment divergence remains, hence known not match). svelte/css unmoved.
//
// typescript 64→63 (2026-07-08, ../prettier 1dcd0b0, ../svelte 8fb7ceeba,
// ../kit 1b4adccf7, ../svelte.dev fb5a4e2, ../prettier-plugin-svelte 7809486, oracle
// acorn-typescript 312d079): the intersection continuation-indent fix (a pure
// non-object intersection breaking in a `wrap_in_group` position — type argument,
// tuple element, conditional branch — now indents continuation members one level
// deeper, `build_intersection_type_doc`) resolved the residual unexplained hunk in
// prettier/tests/format/typescript/union/consistent-with-flow/comment.ts, so it moved
// partial→known. A before/after --all diff on the identical corpus confirmed exactly
// one file moved (0 new unknowns, match/unknown unmoved, SAFETY 0). svelte/css unmoved.
/**
 * corpus:compare:format --all — EXACT per-language `partial` divergence count over the
 * REPRODUCIBLE subset (same semantics as `CORPUS_FORMAT_UNKNOWN_PIN`). Measured 2026-07-17
 * (same checkouts + attribution as `CORPUS_FORMAT_MATCH_MIN`; SAFETY 0). svelte is 0 because
 * all 5 live svelte partials — the fuz fill-family `.svelte` pages — are in the non-gating
 * WARN, not the gate.
 */
export const CORPUS_FORMAT_PARTIAL_PIN: Record<Language, number> = {
	svelte: 0,
	typescript: 55,
	css: 9,
};
// typescript 56→55 (2026-07-17, checkouts as in CORPUS_FORMAT_UNKNOWN_PIN above; svelte 0 +
// css 9 unchanged, SAFETY 0). The postfix-operator RHS `fluid` fix: ../kit exports/public.d.ts
// moves partial→known — `type RemoteFormFieldType<T> = { [K in keyof InputTypeMap]: … }[keyof
// InputTypeMap]`, an indexed-access RHS whose mapped-type OBJECT was breaking after `=` instead of
// hugging `= {` and expanding internally. Routing `TSArrayType`/`TSIndexedAccessType` through
// prettier's `fluid` (like the bare conditional/intersection/function RHS) converged that hunk; the
// file's remaining hunks are cataloged, so it moved to `known`. Whole-corpus pre/post per-file
// bucket diff confirmed exactly this file left, ZERO files entered partial (known 155→156), SAFETY 0.
// typescript 57→56 (2026-07-17, checkouts as in CORPUS_FORMAT_UNKNOWN_PIN above; svelte 0 +
// css 9 unchanged, SAFETY 0). Same conditional/parenthesized-type body-indent fix (bug141
// §Bug 2): ../svelte src/index.d.ts moves partial→match — a nested conditional + a
// parenthesized constructor type as an intersection member, where tsv over-indented the
// parenthesized member one level past prettier. The surgical intersection-member bare-paren
// change (`build_intersection_member_type_doc`) converged it. Whole-corpus pre/post per-file
// bucket diff confirmed exactly this file left, ZERO files entered partial, SAFETY 0.
// ─── SUPERSEDED HISTORY (attribution trail; NOT current — pre the 2026-07-16 reproducible
// split, these were the AGGREGATE over live+framework and moved on dev-repo churn) ───
// typescript 60→59 (2026-07-14, checkouts unchanged; svelte 5 + css 9 unchanged, SAFETY 0). One
// file leaves, none enter — `js/comments-closure-typecast/issue-8045.js`, `partial`→`known`, from
// the same `return`/`throw` leading-comment fix as the unknown pin below. Its JSDoc cast is
// written with a newline after the comment (`return (/** @type {T} */⏎(x))`), so the argument now
// takes the parenthesized form instead of being pulled onto the keyword's line; that matches
// prettier's structure and drops the file's unexplained hunks. The residual (tsv keeps the cast's
// parens, prettier-on-`.js` via babel is the oracle there) is the pre-existing cast-preservation
// divergence, untouched.
// typescript 61→60 + svelte 4→5 (2026-07-14, checkouts as in CORPUS_FORMAT_UNKNOWN_PIN
// above; css 9 unchanged, SAFETY 0). Two independent moves, each attributed by the
// pre-change-binary per-file diff described on CORPUS_FORMAT_UNKNOWN_PIN:
//
// - typescript −1 is BEHAVIOR: exactly one file leaves, none enter —
//   `typescript/class-comment/class-implements.ts`, `partial`→`known`. Like the unknown −1
//   it is a #463 line-comment-swallow fix (the heritage-list keyword→first-element gap no
//   longer swallows the next element), now matched by the `class/heritage_element_line_comment`
//   divergence that landed with it.
// - svelte +1 is live-corpus CHURN, and the 4 was already stale: the pre-change binary
//   measures svelte partial 5 on today's checkouts too (the whole svelte bucket is
//   byte-identical between the two binaries), so nothing in #463–#469 moved it. All 5 svelte
//   partials live in live dev repos that GATE_CHECKOUT_COMMITS does not track, and all 5
//   share one pre-existing signature (`fill_after_inline` + `fill_101_boundary`) — a doc page
//   in ../fuz_blog (committed 2026-07-14) joined the same known fill-wrapping divergence. No
//   new divergence class appeared.
// typescript 63→61 (2026-07-13, checkouts as in CORPUS_FORMAT_MATCH_MIN above). Pure
// live-corpus drift, NOT a behavior change: the pre-change binary already measures 61 on
// today's checkouts, and this branch's computed-lookup work moves the bucket by 0. The −2
// came in with the ../kit (5c38e515d→da5b08ea7) + ../svelte.dev (93a400d→c21c2d0f0)
// movement since the pin was taken — the drift the new pins:audit commit check now surfaces.
// typescript 63 (net unchanged 2026-07-12, composition shifted): on main the #422
// consecutive-line-comment break added two comment_position partials (63→65), and this
// branch's two formatter fixes resolved two others back to match — js/for/parentheses.js
// (the `for` init-only spacing fix) and js/switch/comments.js (the empty-statement-position
// block expand fix). Net 63 (same value, two-in/two-out); a before/after --all path-diff
// confirmed 0 files ENTERED partial, SAFETY 0.
// css 8→9 (2026-07-10): `prettier/tests/format/css/url/url.css` moved unknown→partial after the
// value-parser escaped-paren fix (see the css 23→22 unknown-pin note above) — its `background:`
// url list now formats cleanly and reads as fill_101_boundary, with the escaped-paren
// outer-whitespace hunks (url_escaped_paren_ws) unexplained. SAFETY 0. (Net for the day: 9→8→9 —
// inline_url.css left partial for known, url.css joined.)
// css 9→8 (2026-07-10): `prettier/tests/format/css/inline-url/inline_url.css` moved
// partial→known. The new `css_url_opaque` detector now explains its remaining hunk —
// `url(--var(foo-bar,#dadce0))` → prettier `url(--var(foo-bar, #dadce0))` (same
// opaque-url class as the `loose.css` unknown→known move above; its css_value_wrap /
// fill_101_boundary hunks were already detected). SAFETY 0.

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
