/**
 * Shared parse-parity SANCTIONS — over-rejections (tsv rejects input the
 * canonical parser accepts) that tsv *keeps deliberately*. The bar is high:
 * matching the canonical PARSER is the drop-in contract, and tsv already defers
 * static-semantic "invalid" verdicts to a diagnostics layer (as it does for TS
 * early-errors). So "the canonical parser is merely lenient; its
 * validator/compiler rejects it later" is NOT a sanction — that input should
 * parse (accept → AST → format round-trip) and its invalidity is a `KnownGap` in
 * the consuming gate, not a strictness win. Only two shapes qualify: (1)
 * valid-but-**deprecated** syntax tsv declines for its successor (e.g. import
 * assertions `assert {…}` → the `with {…}` tsv parses), and (2) an explicit
 * **taste** divergence cataloged with a spec/prettier reason. Matched as a path
 * substring; each carries a reason so the list stays a reviewed catalogue, never
 * a silent bug suppressor. A genuine gap (tsv wrong, intent-to-fix) goes in the
 * consuming gate's own KNOWN_GAPS, never here.
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
 * Svelte's own `tests/` fixtures: no sanctions. Every Svelte over-rejection is a
 * drop-in gap to fix, not correct-strictness — svelte's PARSER accepts each (its
 * VALIDATOR, a stage tsv doesn't run, is what rejects), so they're tracked as
 * `KnownGap`s in `svelte_fixtures_compare.ts` and get fixtures-first fixes. See
 * the sanction bar in the module doc above.
 */
export const SVELTE_FIXTURE_SANCTIONS: Sanction[] = [];

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
