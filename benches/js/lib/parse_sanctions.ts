/**
 * Shared parse-parity SANCTIONS — over-rejections (tsv rejects input the
 * canonical parser accepts) where tsv is *correctly* stricter: the input is
 * invalid Svelte the parser is merely lenient about (its own validator/compiler
 * rejects it), so tsv rejecting one stage earlier is right. Matched as a path
 * substring; each carries a reason so the list stays a reviewed catalogue, never
 * a silent bug suppressor. A genuine gap gets FIXED (or, for a tracked drop-in
 * gap, listed in the consuming tool's own KNOWN_GAPS) — never sanctioned here.
 *
 * One home shared by `diagnostics/skip_triage.ts` (the general parse-parity gate)
 * and `diagnostics/svelte_fixtures_compare.ts` (the dedicated Svelte-fixtures
 * conformance gate), so the Svelte-fixture sanctions don't drift between them.
 */

export interface Sanction {
	pattern: string;
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
		pattern: 'css/samples/supports-import/',
		reason: '`@import` inside `@supports` prelude — Svelte lenient, tsv grammar-stricter',
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

/** First matching sanction reason for `path`, or null. */
export function sanction_for(sanctions: Sanction[], path: string): string | null {
	return sanctions.find((s) => path.includes(s.pattern))?.reason ?? null;
}
