/**
 * The type-strip API the strip-capable tsv npm packages export (`@fuzdev/tsv-wasm` and the
 * native `@fuzdev/tsv`) — the strip half of the facade `api.js` opens with (its module
 * doc carries the bag's rules, and its `read_options` / `read_source` the error texts).
 *
 * `strip_<lang>(source, options?)` RETURNS what the strip concludes about the source and
 * never throws for it: `{ok: true, code}` for the JS, or `{ok: false, error}` for a source
 * that does not parse (`error.kind === 'parse'`, the error `parse_<lang>(source, {sourceType})`
 * returns for the goal the error belongs to) or one holding TypeScript the strip refuses
 * (`error.kind === 'unsupported'`: the `construct`, its one-line `message`, the `frame` under
 * it, and its span as `start` / `end` / `loc: {start, end}`). The error is a plain object —
 * no stack captured per failure, and structured-clone-able, so a worker can post it. What
 * still throws is what throws on every other export: an argument refusal (a `TypeError`), and
 * an engine failure with no point (the size cap, a caught panic, a WASM trap, stack
 * exhaustion).
 *
 * The result shape is every facade export's (`api.js`'s module doc); what the strip adds is
 * its refusal. The raw engines (`tsv_wasm`, `tsv_napi`) export
 * `strip_typescript(source, source_type?)`, the flat shape every other export has, with the
 * source type the bag names forwarded as it is (unset stays `undefined`: the module grammar,
 * retried as a script, as a format reads it) — returning the code, or the `{ok: false, error}`
 * they built, which the facade checks (`read_failure`, and the `construct` against the set
 * below) and hands back as it is.
 *
 * A module of its own so the packages without the strip load none of it; it imports
 * nothing but `api.js`'s readers.
 */

import { read_failure, read_options, read_source } from './api.js';

/** The error kinds the strip returns: a parse failure, or a refusal. */
const STRIP_KINDS = new Set(['parse', 'unsupported']);

/**
 * The constructs a refusal names, as the engines spell `construct`. One outside this set
 * is an engine bug, so it is thrown rather than handed back as a result.
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
 * @param {Record<string, (source: string, source_type?: 'script' | 'module') => unknown>} strip
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
				const code = strip_language(text, source_type);
				if (typeof code === 'string') return { ok: true, code };
				return read_strip_failure(code);
			}
		});
	}
	return api;
}

/**
 * An engine's returned strip failure, checked: `read_failure`'s shape, and for a refusal a
 * `construct` the strip names.
 *
 * @param {unknown} value - what the engine returned in place of the code
 * @returns {{ok: false, error: Record<string, any>}}
 * @throws Error when `value` is no such failure — an engine bug
 */
function read_strip_failure(value) {
	const failure = read_failure(value, STRIP_KINDS);
	const { error } = failure;
	if (error.kind === 'unsupported' && !CONSTRUCTS.has(error.construct)) {
		throw new Error('internal error: the engine refused a construct the strip never names', {
			cause: value
		});
	}
	return failure;
}
