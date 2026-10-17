/**
 * The type-strip API the strip-capable tsv npm packages export (`@fuzdev/tsv-wasm` and the
 * native `@fuzdev/tsv`) — the strip half of the facade `api.js` opens with (its module
 * doc carries the bag's rules, and its `read_options` / `read_source` the error texts).
 *
 * `strip_<lang>(source, options?)` RETURNS what the strip concludes about the source and
 * never throws for it: `{ok: true, code}` for the JS, or `{ok: false, error}` for a source
 * that does not parse (`error.kind === 'parse'`, the `message` / `start` / `loc` the
 * `SyntaxError` from `parse_<lang>(source, {sourceType})` carries for the goal the error
 * belongs to) or one holding TypeScript the strip
 * refuses (`error.kind === 'unsupported'`: the `construct`, its `message`, and its span as
 * `start` / `end` / `loc: {start, end}`). The error is a plain object — no stack captured
 * per failure, and structured-clone-able, so a worker can post it. What still throws is
 * what throws on every other export: an argument refusal (a `TypeError`), and an engine
 * failure with no point (the size cap, a caught panic, a WASM trap, stack exhaustion).
 *
 * The raw engines (`tsv_wasm`, `tsv_napi`) export `strip_typescript(source, source_type?)`,
 * the flat shape every other export has, with the source type the bag names forwarded as
 * it is (unset stays `undefined`: the module grammar, retried as a script, as a format reads
 * it) — returning the code, or throwing an `Error` whose own
 * ENUMERABLE integer properties carry the failure: `start`, `line` and `column` for a parse
 * failure (the shape every parse export throws), and those plus a string `kind` (the
 * refused construct) and `end`, `end_line`, `end_column` for a refusal. Anything else an
 * engine throws passes through as itself.
 *
 * A module of its own so the packages without the strip load none of it; it imports
 * nothing but `api.js`'s readers.
 */

import { read_options, read_source } from './api.js';

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
 * A point is three OWN, ENUMERABLE, integer properties, read as `api.js`'s
 * `to_syntax_error` reads one (Bun gives every `Error` its own non-enumerable numeric
 * `line` and `column`, so the values alone would not do). With no own enumerable `kind` it
 * is a parse failure; with a `kind` naming a known construct and an integer `end`,
 * `end_line` and `end_column`, a refusal.
 *
 * @param {unknown} thrown - what the engine threw
 * @returns {object | undefined}
 */
function to_strip_error(thrown) {
	if (typeof thrown !== 'object' || thrown === null) return undefined;
	if (!is_point_property(thrown, 'start')) return undefined;
	if (!is_point_property(thrown, 'line')) return undefined;
	if (!is_point_property(thrown, 'column')) return undefined;
	const failure = /** @type {Record<string, any>} */ (thrown);
	const message = typeof failure.message === 'string' ? failure.message : String(thrown);
	const loc = { line: failure.line, column: failure.column };
	if (!Object.prototype.propertyIsEnumerable.call(thrown, 'kind')) {
		return { kind: 'parse', message, start: failure.start, loc };
	}
	if (!CONSTRUCTS.has(failure.kind)) return undefined;
	if (!is_point_property(thrown, 'end')) return undefined;
	if (!is_point_property(thrown, 'end_line')) return undefined;
	if (!is_point_property(thrown, 'end_column')) return undefined;
	return {
		kind: 'unsupported',
		construct: failure.kind,
		message,
		start: failure.start,
		end: failure.end,
		loc: { start: loc, end: { line: failure.end_line, column: failure.end_column } }
	};
}

/**
 * Whether `key` is an own enumerable property of `object` holding an integer.
 *
 * @param {object} object
 * @param {string} key
 * @returns {boolean}
 */
function is_point_property(object, key) {
	return (
		Object.prototype.propertyIsEnumerable.call(object, key) &&
		Number.isInteger(/** @type {Record<string, unknown>} */ (object)[key])
	);
}
