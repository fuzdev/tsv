/**
 * The type-strip API the strip-capable tsv npm packages export (`@fuzdev/tsv-wasm` and the
 * native `@fuzdev/tsv`) — the strip half of the facade `api.js` opens with (its module
 * doc carries the bag's rules, and its `read_options` / `read_source` the error texts).
 *
 * `strip_<lang>(source, options?)` RETURNS what the strip concludes about the source and
 * never throws for it: `{ok: true, code}` for the JS, or `{ok: false, error}` for a source
 * that does not parse (`error.kind === 'parse'`, the error `parse_<lang>(source, {sourceType})`
 * returns for the goal the error belongs to) or one holding TypeScript the strip
 * refuses (`error.kind === 'unsupported'`: the `construct`, its `message`, and its span as
 * `start` / `end` / `loc: {start, end}`). The error is a plain object — no stack captured
 * per failure, and structured-clone-able, so a worker can post it. What still throws is
 * what throws on every other export: an argument refusal (a `TypeError`), and an engine
 * failure with no point (the size cap, a caught panic, a WASM trap, stack exhaustion).
 *
 * The result shape is every facade export's (`api.js`'s module doc); what the strip adds is
 * its refusal. The raw engines (`tsv_wasm`, `tsv_napi`) export
 * `strip_typescript(source, source_type?)`, the flat shape every other export has, with the
 * source type the bag names forwarded as it is (unset stays `undefined`: the module grammar,
 * retried as a script, as a format reads it) — returning the code, or throwing an `Error`
 * whose own ENUMERABLE integer properties carry the failure: `start`, `line` and `column`
 * for a parse failure (the shape every parse export throws), and those plus a string `kind`
 * (the refused construct) and `end`, `end_line`, `end_column` for a refusal. Anything else
 * an engine throws passes through as itself.
 *
 * A module of its own so the packages without the strip load none of it; it imports
 * nothing but `api.js`'s readers.
 */

import {
	is_point_property,
	read_options,
	read_pointed_failure,
	read_source,
	to_parse_error
} from './api.js';

/**
 * The constructs a refusal names, as the engines spell `kind`. A `kind` outside this set
 * is an engine bug, so it is rethrown rather than handed back as a result.
 */
const CONSTRUCTS = new Set([
	'enum',
	'value_namespace',
	'parameter_property',
	'import_equals',
	'export_equals',
	'type_assertion',
	'assertion_grouping'
]);

/**
 * Build the strip family over one engine's `strip_<lang>(source, source_type?)` functions,
 * keyed by language. Each function is defined under a computed key, which names it, as
 * `create_format_api`'s are.
 *
 * @param {Record<string, (source: string, source_type?: 'script' | 'module') => string>} strip
 * @returns {Record<string, (source: unknown, options?: unknown) => any>} `strip_<lang>`
 */
export function create_strip_api(strip) {
	/** @type {Record<string, (source: unknown, options?: unknown) => any>} */
	const api = {};
	for (const [language, strip_language] of Object.entries(strip)) {
		Object.assign(api, {
			[`strip_${language}`]: (source, options) => {
				const text = read_source(source, 'strip');
				const { source_type } = read_options(options, 'strip', language);
				let code;
				try {
					code = strip_language(text, source_type);
				} catch (thrown) {
					const error = to_strip_error(thrown);
					if (error === undefined) throw thrown;
					return { ok: false, error };
				}
				return { ok: true, code };
			}
		});
	}
	return api;
}

/**
 * The result's error for an engine's pointed failure, or `undefined` for anything else
 * (rethrown by the caller as itself).
 *
 * A point is read by `api.js`'s `read_pointed_failure`. With no own enumerable `kind` it is a
 * parse failure (`to_parse_error`, the shape every export's parse failure takes); with a
 * `kind` naming a known construct and an integer `end`, `end_line` and `end_column`, a
 * refusal.
 *
 * @param {unknown} thrown - what the engine threw
 * @returns {object | undefined}
 */
function to_strip_error(thrown) {
	const pointed = read_pointed_failure(thrown);
	if (pointed === undefined) return undefined;
	if (pointed.kind === undefined) return to_parse_error(pointed);
	const { kind, message, start, loc, failure } = pointed;
	if (!CONSTRUCTS.has(kind)) return undefined;
	if (!is_point_property(failure, 'end')) return undefined;
	if (!is_point_property(failure, 'end_line')) return undefined;
	if (!is_point_property(failure, 'end_column')) return undefined;
	return {
		kind: 'unsupported',
		construct: kind,
		message,
		start,
		end: failure.end,
		loc: { start: loc, end: { line: failure.end_line, column: failure.end_column } }
	};
}
