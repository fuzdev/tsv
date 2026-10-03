/**
 * How the parse comparisons serialize a canonical parser's AST before diffing it against
 * tsv's wire — shared by `corpus_compare_parse.ts` and the fixtures gates
 * (`lib/fixtures_gate.ts`), so the two grade one serialization.
 *
 * tsv's bindings emit one wire, span-only: `start`/`end` offsets, no per-node `loc`
 * (Svelte also no `name_loc`). The oracles emit `loc` (acorn under Svelte's
 * `locations: true`, Svelte's own reader), so the span grading strips exactly those keys
 * from the oracle's output — the definition `tests/loc_definition.rs` (`strip_locations`)
 * encodes, and the whole of what the span-only wire omits (the `character` field Svelte
 * puts on a name-shaped or in-tag-comment position lives inside one of them). Nothing
 * else differs, so the diff engine and its documented matchers grade the result
 * unchanged.
 *
 * @module
 */

/**
 * The BigInt half of the fixture sidecar's jsonReplacer
 * (`crates/tsv_debug/src/deno/sidecar.ts`), so the comparisons and `expected.json`
 * generation agree on the values neither can serialize natively.
 *
 * ⚠️ Deliberately NOT the sidecar's whole replacer: the sidecar also substitutes U+FFFD
 * for lone surrogates, because its response crosses a Rust boundary where serde_json
 * rejects the document outright. Nothing crosses a boundary here, so the canonical AST
 * keeps acorn's TRUE lone-surrogate value — which is the only reason
 * `parse_divergences.ts`'s `lone_surrogate_value` divergence matcher can still fire.
 * Substituting here would make the canonical side agree with ours by construction and
 * silently retire that matcher.
 */
export function bigint_replacer(_key: string, value: unknown): unknown {
	return typeof value === 'bigint' ? value.toString() : value;
}

/** The keys the span-only wire omits — see the module doc. */
export const LOCATION_KEYS: ReadonlySet<string> = new Set(['loc', 'name_loc']);

/** `bigint_replacer` that also drops the span-only wire's omitted keys. */
export function span_only_replacer(key: string, value: unknown): unknown {
	return LOCATION_KEYS.has(key) ? undefined : bigint_replacer(key, value);
}
