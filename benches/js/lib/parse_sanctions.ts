/**
 * Shared parse-parity SANCTIONS — over-rejections (tsv rejects input the
 * canonical parser accepts) that tsv *keeps deliberately*: either the input is
 * invalid the canonical parser is merely lenient about (its own
 * validator/compiler rejects it), or it is valid-but-deprecated syntax tsv
 * declines in favour of its successor. Either way rejecting is intended and must
 * NOT be "fixed". Matched as a path substring; each carries a reason so the list
 * stays a reviewed catalogue, never a silent bug suppressor. A genuine gap
 * (tsv wrong, intent-to-fix) goes in the consuming tool's own KNOWN_GAPS, never
 * here.
 *
 * One home shared by `diagnostics/skip_triage.ts` (the general parse-parity gate)
 * and the dedicated per-language fixtures gates (`svelte_fixtures_compare.ts`,
 * `ts_fixtures_compare.ts`), so a language's sanctions don't drift between them.
 * Also the single home for the sibling {@link KnownGap} type (below) that every
 * gate's `KNOWN_GAPS` list uses.
 */

export interface Sanction {
	pattern: string;
	reason: string;
}

/**
 * The sibling of {@link Sanction}: an over-rejection where tsv is WRONG — a genuine
 * drop-in parse gap, tracked (in the consuming gate's own `KNOWN_GAPS`) so the gate
 * is green at baseline and only a NEW untracked over-rejection fails it. Matched as
 * a path substring. This set must only SHRINK: delete an entry once its gap is fixed
 * (the input then parses → parity). `Sanction` = keep deliberately; `KnownGap` =
 * fix eventually — the two never overlap.
 */
export interface KnownGap {
	pattern: string;
	category: string;
	reason: string;
}

/**
 * Svelte's own `tests/` fixtures where tsv is correctly stricter. Real source
 * needs none of these — each is invalid Svelte the parser merely tolerates.
 */
export const SVELTE_FIXTURE_SANCTIONS: Sanction[] = [
	// CSS grammar-strictness — Svelte's parseCss is lenient where tsv follows the
	// CSS grammar. See docs/conformance_svelte.md §CSS Parser Scope & Error Model.
	{
		pattern: 'css/samples/comment-html/',
		reason: 'HTML comment (`<!-- -->`) in a CSS selector — Svelte lenient, tsv follows the CSS grammar',
	},
	{
		pattern: 'validator/samples/css-invalid-combinator-selector',
		reason: 'invalid leading combinator (`>`/`+`) — Svelte parser accepts, its validator rejects',
	},
	// Invalid Svelte markup — Svelte's PARSER accepts, its VALIDATOR rejects; the
	// input is invalid either way, tsv just rejects one stage earlier.
	{
		pattern: 'validator/samples/attribute-invalid-name',
		reason: 'invalid attribute-name character — Svelte parser lenient, validator rejects',
	},
	{
		pattern: 'validator/samples/if-block-whitespace',
		reason: 'whitespace after `{#` (`{ #if}`) — Svelte parser lenient, validator rejects',
	},
];

/**
 * acorn-typescript's own `test/` fixtures where tsv over-rejects deliberately.
 * Real source needs none of these. A genuine gap (tsv wrong) goes in
 * `ts_fixtures_compare.ts` KNOWN_GAPS, never here.
 */
export const TS_FIXTURE_SANCTIONS: Sanction[] = [
	// Deprecated import assertions (`assert { type: 'json' }`) — superseded by
	// import attributes (`with { … }`, which tsv parses). Deliberate non-support,
	// not a gap.
	{
		pattern: 'assert_import_assert/',
		reason: "deprecated import assertions (`assert {…}`) — tsv supports the successor `with {…}` only",
	},
];

/** First matching sanction reason for `path`, or null. */
export function sanction_for(sanctions: Sanction[], path: string): string | null {
	return sanctions.find((s) => path.includes(s.pattern))?.reason ?? null;
}
