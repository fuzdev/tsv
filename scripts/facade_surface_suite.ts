/**
 * Facade rows every tsv npm package entry is held to, registered by both package suites
 * (`scripts/test_npm.ts` over each WASM variant's auto-init and lazy entries,
 * `scripts/test_napi_npm.ts` over the native loader) so one row cannot drift between them:
 *
 * - every exported function is named for its export — a caller's stack trace and a
 *   `function.name` check read an export's own name, and the facade builds its functions
 *   under computed keys, which name them (an assignment to a computed member leaves it
 *   `''`, which is how the facade's functions once all shipped);
 * - a source holding a lone surrogate is refused by every `format_*` / `parse_*` export,
 *   never converted lossily — both engines read the source as UTF-8, which would replace it
 *   with U+FFFD, so the facade refuses one before either engine sees it (and before the lazy
 *   entries' init guard, so the row holds on an uninitialized one too).
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

export interface FacadeSurfaceOptions {
	/** The exact count of `format_*` / `parse_*` exports the entry publishes, where the
	 * suite knows it; otherwise at least one per language is required. */
	facade_exports?: number;
}

/**
 * Register the shared facade rows over one package entry, given as its module namespace or
 * as a function returning it for an entry only bound once a test ahead of these has run.
 *
 * @param label - names the entry in the suite's title
 * @param api_or_getter - the entry's exports, or a function returning them
 * @param options - what the caller knows of the entry's export set
 */
export function register_facade_surface_suite(
	label: string,
	api_or_getter: Record<string, any> | (() => Record<string, any>),
	options: FacadeSurfaceOptions = {}
): void {
	const get_api = () => (typeof api_or_getter === 'function' ? api_or_getter() : api_or_getter);
	describe(`facade surface: ${label}`, () => {
		it('every exported function is named for its export', () => {
			const functions = Object.entries(get_api()).filter(
				([, value]) => typeof value === 'function'
			);
			assert.ok(
				functions.length > 5,
				`expected the published functions, found ${functions.length}`
			);
			for (const [name, value] of functions) {
				assert.equal((value as { name: string }).name, name, name);
			}
		});

		it('a source holding a lone surrogate is refused, never converted lossily', () => {
			const api = get_api();
			const exports = Object.keys(api).filter((name) => /^(format|parse)_/.test(name));
			if (options.facade_exports === undefined) {
				assert.ok(exports.length >= 3, `expected the facade's exports, found ${exports.length}`);
			} else {
				assert.equal(exports.length, options.facade_exports);
			}
			for (const name of exports) {
				const noun = name.startsWith('format_') ? 'format' : 'parse';
				assert.throws(() => api[name]('"\uD800";'), {
					name: 'TypeError',
					message: `${noun} source must be well-formed UTF-16 (a lone surrogate at offset 1)`
				});
			}
		});
	});
}
