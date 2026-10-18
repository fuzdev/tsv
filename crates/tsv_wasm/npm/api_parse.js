/**
 * The parse API every parse-capable tsv npm package exports — the parse half of the
 * facade `api.js` opens with (its module doc carries the bag's rules, and its
 * `read_options` the error texts).
 *
 * The engine emits one wire, span-only: `start`/`end` offsets, no per-node `loc`.
 * `{locations: true}` runs `locations.js`'s `reconstruct_locations` over the parsed
 * object — the same definition the Rust `loc` emitter (`tsv parse --locations`)
 * implements, acorn-exact for TypeScript — so `loc` is a view, computed in JS where a
 * caller asks for it. `parse_<lang>_json` returns the wire string untouched: it refuses
 * `locations` (pointing at `parse_<lang>`), since `loc` is a view over objects.
 *
 * Both return a result, as every facade export does (`api.js`'s module doc): `{ok: true, ast}`
 * / `{ok: true, json}`, or the `{ok: false, error}` the engine returned, a `kind: 'parse'`
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
 * `engine.parse_json` (`(source, source_type?) => string | failure`) is required;
 * `engine.parse` (`(source, source_type?) => {ok: true, ast} | failure`) is optional — an
 * engine that parses its wire itself (the WASM bindings run `JSON.parse` engine-side, and
 * build the whole result there) supplies it, and otherwise the facade `JSON.parse`s the
 * string. A `JSON.parse` failure there is an engine bug, never the caller's source: it
 * throws a plain `Error` (the WASM engine's own text for the same failure, the
 * `SyntaxError` as its `cause`), never a result. A failure is the `{ok: false, error}` the
 * engine returned (`read_failure`).
 *
 * @param {{
 * 	parse_json: Record<string, (source: string, source_type?: string) => unknown>,
 * 	parse?: Record<string, (source: string, source_type?: string) => unknown>
 * }} engine
 * @returns {Record<string, (source: unknown, options?: unknown) => any>}
 * 	`parse_<lang>` and `parse_<lang>_json`.
 */
export function create_parse_api(engine) {
	/** @type {Record<string, (source: unknown, options?: unknown) => any>} */
	const api = {};
	for (const [language, parse_json] of Object.entries(engine.parse_json)) {
		const engine_parse = engine.parse?.[language];
		// computed keys name the functions, as `create_format_api`'s do
		Object.assign(api, {
			[`parse_${language}`]: (source, options) => {
				const text = read_source(source, 'parse');
				const parsed = read_options(options, 'parse', language);
				/** @type {{ok: true, ast: any}} */
				let result;
				if (engine_parse) {
					const returned = /** @type {any} */ (engine_parse(text, parsed.source_type));
					if (returned?.ok !== true || !('ast' in returned)) {
						return read_failure(returned, PARSE_KINDS);
					}
					result = returned;
				} else {
					const json = parse_json(text, parsed.source_type);
					if (typeof json !== 'string') return read_failure(json, PARSE_KINDS);
					result = { ok: true, ast: parse_wire(json) };
				}
				if (parsed.locations) result.ast = reconstruct_locations(result.ast, text, { language });
				return result;
			},
			[`parse_${language}_json`]: (source, options) => {
				const text = read_source(source, 'parse');
				const { source_type } = read_options(options, 'parse_json', language);
				const json = parse_json(text, source_type);
				if (typeof json === 'string') return { ok: true, json };
				return read_failure(json, PARSE_KINDS);
			}
		});
	}
	return api;
}

/**
 * `JSON.parse` an engine's wire string. A failure is the engine's (it serialized an AST to
 * invalid JSON), so it is thrown as a plain `Error` carrying the WASM engine's text for the
 * same case, never reported as the source's parse failure.
 *
 * @param {string} json - the wire string
 * @returns {any}
 * @throws Error when `json` is not valid JSON
 */
function parse_wire(json) {
	try {
		return JSON.parse(json);
	} catch (cause) {
		throw new Error('internal error: AST serialized to invalid JSON', { cause });
	}
}
