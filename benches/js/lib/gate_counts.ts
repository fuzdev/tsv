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
export const SVELTE_FIXTURES_PINS: GatePins = { scanned: 3375, both_accept: 3280 };

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
 * corpus:compare:format --all — MINIMUM per-language exact `match` count (same
 * live-corpus semantics as `CORPUS_PARSE_COMPARED_MIN`): a shrink fails (a regression
 * or a gutted corpus), a rise passes (re-pin to keep the floor tight). Measured
 * 2026-07-08 against ../svelte 8fb7ceeba, ../kit 1b4adccf7, ../svelte.dev fb5a4e2,
 * oracle @sveltejs/acorn-typescript 1.0.11; SAFETY 0. When a fix moves a file or the
 * live corpus grows, re-pin deliberately and put the why in the commit.
 */
export const CORPUS_FORMAT_MATCH_MIN: Record<Language, number> = {
	svelte: 1111,
	typescript: 4085,
	css: 126,
};

/**
 * corpus:compare:format --all — EXACT per-language `unknown` + `partial`
 * divergence counts (the un-triaged surface the WARN line reports). Both directions
 * fail: a rise = a new unexplained divergence (fix it, catalog a detector in
 * `lib/divergence/patterns.ts`, or consciously re-pin a legitimately-unsupported new
 * corpus file); a drop = the backlog shrank, re-pin to record the win. A single-run
 * trip can be the FFI/sidecar heisenbug — confirm on the single repo first. Measured
 * 2026-07-08 against ../svelte 8fb7ceeba, ../kit 1b4adccf7, ../svelte.dev fb5a4e2,
 * ../prettier-plugin-svelte 7809486, oracle acorn-typescript 1.0.11; SAFETY 0. Put the
 * why for each move in the commit.
 */
export const CORPUS_FORMAT_UNKNOWN_PIN: Record<Language, number> = {
	svelte: 7,
	typescript: 173,
	css: 22,
};
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
export const CORPUS_FORMAT_PARTIAL_PIN: Record<Language, number> = {
	svelte: 4,
	typescript: 63,
	css: 9,
};
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
