/**
 * The parse API every parse-capable tsv npm package exports — the parse half of the
 * facade `api.js` opens with (its module doc carries the bag's rules, and its
 * `read_options` the error texts).
 *
 * The engine emits one wire, span-only: `start`/`end` offsets, no per-node `loc`.
 * `{locations: true}` hands the engine the flag, and the tape route decodes that one wire into
 * the `loc`-bearing tree (`tape_engine.js`, the decoder's loc mode) — the tree `JSON.parse`
 * makes of the Rust `loc` emitter's JSON (`tsv parse --locations`), key order included, under
 * the definition `locations.js` reconstructs by, acorn-exact for TypeScript. So `loc` is a view,
 * computed in JS where a caller asks for it, at no second walk over the tree.
 *
 * `parse_<lang>` returns a result, as every facade export does (`api.js`'s module doc):
 * `{ok: true, ast}`, or the `{ok: false, error}` the engine returned, a `kind: 'parse'`
 * error, for a source that does not parse.
 *
 * A separate module from `api.js` so the format-only package, which ships no parse code,
 * never loads any.
 */

import { PARSE_KINDS, read_failure, read_options, read_source } from './api.js';

/**
 * Build the parse family over one engine, keyed by language — the engine's language
 * names, which are `locations.js`'s too.
 *
 * `engine.parse` (`(source, source_type?, locations?) => {ok: true, ast} | failure`) builds
 * the tree, with `loc` when `locations` is `true`: every published entry passes the tape
 * route (`tape_engine.js`'s `create_tape_parse` over the raw `parse_tape_<lang>`, decoding
 * the binary wire). A failure is the `{ok: false, error}` the engine returned
 * (`read_failure`).
 *
 * @param {{parse: Record<string, (source: string, source_type?: string, locations?: boolean) => unknown>}} engine
 * @returns {Record<string, (source: unknown, options?: unknown) => any>} `parse_<lang>`.
 */
export function create_parse_api(engine) {
	/** @type {Record<string, (source: unknown, options?: unknown) => any>} */
	const api = {};
	for (const [language, engine_parse] of Object.entries(engine.parse)) {
		// computed keys name the functions, as `create_format_api`'s do
		Object.assign(api, {
			[`parse_${language}`]: (source, options) => {
				const text = read_source(source, 'parse');
				const parsed = read_options(options, 'parse', language);
				const returned = /** @type {any} */ (
					engine_parse(text, parsed.source_type, parsed.locations)
				);
				if (returned?.ok !== true || !('ast' in returned)) {
					return read_failure(returned, PARSE_KINDS);
				}
				return /** @type {{ok: true, ast: any}} */ (returned);
			}
		});
	}
	return api;
}
