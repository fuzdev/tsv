/**
 * The import-records API the imports-capable tsv npm packages export (`@fuzdev/tsv-wasm` and
 * the native `@fuzdev/tsv`) — the imports half of the facade `api.js` opens with (its module
 * doc carries the bag's rules, and its `read_options` / `read_source` the error texts).
 *
 * `parse_imports_<lang>(source, options?)` RETURNS what the parse concludes about the source
 * and never throws for it: `{ok: true, hasModuleSyntax, imports}` for a source that parses, or
 * `{ok: false, error}` for one that does not (`error.kind === 'parse'`, the error
 * `parse_<lang>(source, {sourceType})` returns for the goal the error belongs to). What still
 * throws is what throws on every other export: an argument refusal (a `TypeError`), and an
 * engine failure with no point (the size cap, a caught panic, a WASM trap, stack exhaustion).
 *
 * The raw engines (`tsv_wasm`, `tsv_napi`) export `parse_imports_<lang>(source, source_type?)`,
 * the flat shape every other export has, with the source type the bag names forwarded as it
 * is (unset stays `undefined`: the module grammar, retried as a script, as a strip reads it).
 * They return the records as one JSON string — `{hasModuleSyntax, imports}`, every offset a
 * UTF-16 code unit — which the facade `JSON.parse`s, or the `{ok: false, error}` they built,
 * which it checks (`read_failure`) and hands back as it is. A string that is not valid JSON is
 * an engine bug, thrown rather than returned.
 *
 * A module of its own so the packages without the import records load none of it; it imports
 * nothing but `api.js`'s readers.
 */

import { PARSE_KINDS, read_failure, read_options, read_source } from './api.js';

/**
 * Build the imports family over one engine's `parse_imports_<lang>(source, source_type?)`
 * functions, keyed by language. Each function is defined under a computed key, which names
 * it, as `create_format_api`'s are.
 *
 * @param {Record<string, (source: string, source_type?: 'script' | 'module') => unknown>} imports
 * @returns {Record<string, (source: unknown, options?: unknown) => any>} `parse_imports_<lang>`
 */
export function create_imports_api(imports) {
	/** @type {Record<string, (source: unknown, options?: unknown) => any>} */
	const api = {};
	for (const [language, parse_imports_language] of Object.entries(imports)) {
		Object.assign(api, {
			[`parse_imports_${language}`]: (source, options) => {
				const text = read_source(source, 'imports');
				const { source_type } = read_options(options, 'imports', language);
				const json = parse_imports_language(text, source_type);
				if (typeof json !== 'string') return read_failure(json, PARSE_KINDS);
				const { hasModuleSyntax, imports } = parse_records(json);
				return { ok: true, hasModuleSyntax, imports };
			}
		});
	}
	return api;
}

/**
 * `JSON.parse` an engine's import records. A failure is the engine's (it wrote invalid JSON),
 * so it is thrown as a plain `Error`, never reported as the source's parse failure.
 *
 * @param {string} json - the records string
 * @returns {{hasModuleSyntax: boolean, imports: unknown[]}}
 * @throws Error when `json` is not valid JSON
 */
function parse_records(json) {
	try {
		return JSON.parse(json);
	} catch (cause) {
		throw new Error('internal error: import records serialized to invalid JSON', { cause });
	}
}
