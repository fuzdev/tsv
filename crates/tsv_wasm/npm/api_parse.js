/**
 * The parse API every parse-capable tsv npm package exports — the parse half of the
 * facade `api.js` opens with (its module doc carries the bag's rules, and its
 * `read_options` the error texts).
 *
 * The engine emits one wire, span-only: `start`/`end` offsets, no per-node `loc`.
 * `{locations: true}` runs `locations.js`'s `reconstruct_locations` over the parsed
 * object — the same definition the Rust `loc` emitter (`tsv parse --locations`)
 * implements, acorn-exact for TypeScript — so `loc` is a view, computed in JS where a
 * caller asks for it.
 *
 * `parse_<lang>` returns a result, as every facade export does (`api.js`'s module doc):
 * `{ok: true, ast}`, or the `{ok: false, error}` the engine returned, a `kind: 'parse'`
 * error, for a source that does not parse.
 *
 * A separate module from `api.js` so the format-only package, which ships no
 * `locations.js`, never loads one.
 */

import { PARSE_KINDS, read_failure, read_options, read_source } from './api.js';
import { reconstruct_locations } from './locations.js';

/**
 * Build the parse family over one engine, keyed by language — the engine's language
 * names, which are `locations.js`'s too.
 *
 * `engine.parse` (`(source, source_type?) => {ok: true, ast} | failure`) builds the tree:
 * every published entry passes the tape route (`tape_engine.js`'s `create_tape_parse` over
 * the raw `parse_tape_<lang>`, decoding the binary wire). A failure is the `{ok: false,
 * error}` the engine returned (`read_failure`).
 *
 * @param {{parse: Record<string, (source: string, source_type?: string) => unknown>}} engine
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
				const returned = /** @type {any} */ (engine_parse(text, parsed.source_type));
				if (returned?.ok !== true || !('ast' in returned)) {
					return read_failure(returned, PARSE_KINDS);
				}
				/** @type {{ok: true, ast: any}} */
				const result = returned;
				if (parsed.locations) result.ast = reconstruct_locations(result.ast, text, { language });
				return result;
			}
		});
	}
	return api;
}
