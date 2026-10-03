/**
 * The parse API every parse-capable tsv npm package exports — the parse half of the
 * facade `api.js` opens with (its module doc carries the bag's rules and the error
 * texts' single source).
 *
 * The engine emits one wire, span-only: `start`/`end` offsets, no per-node `loc`.
 * `{locations: true}` runs `locations.js`'s `reconstruct_locations` over the parsed
 * object — the same definition the Rust `loc` emitter (`tsv parse --locations`)
 * implements, acorn-exact for TypeScript — so `loc` is a view, computed in JS where a
 * caller asks for it. `parse_<lang>_json` returns the wire string untouched: it refuses
 * `locations` (pointing at `parse_<lang>`), since `loc` is a view over objects.
 *
 * A separate module from `api.js` so the format-only package, which ships no
 * `locations.js`, never loads one.
 */

import { call_engine, has_source_type, read_options, read_source } from './api.js';
import { reconstruct_locations } from './locations.js';

/**
 * Build the parse family over one engine, keyed by language — the engine's language
 * names, which are `locations.js`'s too.
 *
 * `engine.parse_json` (`(source, source_type?) => string`) is required;
 * `engine.parse` (`(source, source_type?) => object`) is optional — an engine that
 * parses its wire itself (the WASM bindings run `JSON.parse` engine-side) supplies it,
 * and otherwise the facade `JSON.parse`s the string. A `JSON.parse` failure there is an
 * engine bug, never the caller's source: it throws a plain `Error` (the WASM engine's own
 * text for the same failure, the `SyntaxError` as its `cause`), so it cannot read as the
 * parse failure every other `SyntaxError` from here is.
 *
 * @param {{
 * 	parse_json: Record<string, (source: string, source_type?: string) => string>,
 * 	parse?: Record<string, (source: string, source_type?: string) => any>
 * }} engine
 * @returns {Record<string, (source: string, options?: unknown) => any>}
 * 	`parse_<lang>` and `parse_<lang>_json`.
 */
export function create_parse_api(engine) {
	/** @type {Record<string, (source: string, options?: unknown) => any>} */
	const api = {};
	for (const [language, parse_json] of Object.entries(engine.parse_json)) {
		const goal = has_source_type(language);
		const engine_parse = engine.parse?.[language];
		api[`parse_${language}`] = (source, options) => {
			const text = read_source(source, 'parse');
			const parsed = read_options(options, 'parse', true, goal);
			const ast = engine_parse
				? call_engine(engine_parse, text, parsed.source_type)
				: parse_wire(call_engine(parse_json, text, parsed.source_type));
			return parsed.locations ? reconstruct_locations(ast, text, { language }) : ast;
		};
		api[`parse_${language}_json`] = (source, options) =>
			call_engine(
				parse_json,
				read_source(source, 'parse'),
				read_options(options, 'parse', false, goal, language).source_type
			);
	}
	return api;
}

/**
 * `JSON.parse` an engine's wire string. A failure is the engine's (it serialized an AST to
 * invalid JSON), so it is thrown as a plain `Error` carrying the WASM engine's text for the
 * same case, never as the bare `SyntaxError` that would read as a parse failure.
 *
 * @param {string} json - the wire string
 * @returns {any}
 * @throws {Error} when `json` is not valid JSON
 */
function parse_wire(json) {
	try {
		return JSON.parse(json);
	} catch (cause) {
		throw new Error('internal error: AST serialized to invalid JSON', { cause });
	}
}
