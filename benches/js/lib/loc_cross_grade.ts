/**
 * The `loc` cross-grade: tsv's loc wire against the shipped reconstruction of its span-only
 * wire.
 *
 * `loc` has one definition and two implementations — the Rust writers, and
 * `crates/tsv_wasm/npm/locations.js` — so the reconstruction of the span-only wire must
 * deep-equal the loc wire of the same parse, key order aside (the writer places `loc` after
 * `end`, the reconstruction appends it). Neither side carries a hand-written expectation;
 * each is the other's drift check. Two callers ask it: `scripts/check_loc.ts` over the
 * fixture tree (the `deno task check` leg) and `corpus_compare_parse.ts` over every corpus
 * file, ahead of grading the loc wire against the oracle — so a tolerance row there can
 * only ever excuse the oracle's departure from the definition, never tsv's.
 *
 * Node-modules-free: `scripts/` imports it.
 *
 * @module
 */

// typed by the module's own JSDoc, not its `.d.ts`: that file imports `./tsv_ast.js`, which
// resolves only where a package stages the two side by side
import { reconstruct_locations } from '../../../crates/tsv_wasm/npm/locations.js';
import type { Language } from './types.ts';

/**
 * The first difference between two JSON values, as `path: a vs b`, or `null` when they are
 * deep-equal. Object key order is ignored; array order is not.
 */
export function first_difference(a: unknown, b: unknown, path = '$'): string | null {
	if (a === b) return null;
	if (typeof a !== 'object' || typeof b !== 'object' || a === null || b === null) {
		return `${path}: ${JSON.stringify(a)} vs ${JSON.stringify(b)}`;
	}
	if (Array.isArray(a) !== Array.isArray(b)) return `${path}: array vs non-array`;
	if (Array.isArray(a) && Array.isArray(b)) {
		if (a.length !== b.length) return `${path}: length ${a.length} vs ${b.length}`;
		for (let i = 0; i < a.length; i++) {
			const d = first_difference(a[i], b[i], `${path}[${i}]`);
			if (d !== null) return d;
		}
		return null;
	}
	const ao = a as Record<string, unknown>;
	const bo = b as Record<string, unknown>;
	for (const key of Object.keys(ao)) {
		if (!(key in bo)) return `${path}.${key}: present vs absent`;
	}
	for (const key of Object.keys(bo)) {
		if (!(key in ao)) return `${path}.${key}: absent vs present`;
	}
	for (const key of Object.keys(ao)) {
		const d = first_difference(ao[key], bo[key], `${path}.${key}`);
		if (d !== null) return d;
	}
	return null;
}

/**
 * Where `loc_wire` departs from the reconstruction of `span_wire`, as `path: wire vs
 * reconstruct`, or `null` when the two agree.
 *
 * @param loc_wire - tsv's loc-bearing wire of `source`.
 * @param span_wire - tsv's span-only wire of the same parse; left untouched — the
 *   reconstruction runs on a clone, since it adds `loc` (and the Svelte `name_loc`) in place.
 * @param source - the exact source parsed, a leading BOM included.
 * @param language - the document's language.
 */
export function loc_definition_violation(
	loc_wire: unknown,
	span_wire: unknown,
	source: string,
	language: Language
): string | null {
	const reconstructed = reconstruct_locations(structuredClone(span_wire), source, { language });
	return first_difference(loc_wire, reconstructed);
}
