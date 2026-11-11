/**
 * The npm packages' imports facade (`crates/tsv_wasm/npm/api_imports.js`) over a FAKE engine:
 * the result object the records become, the raw engine's error shapes it reads, the options
 * bag, and the argument refusals, word for word.
 *
 * The raw engine contract (both `tsv_wasm` and `tsv_napi`): `parse_imports_<lang>(source,
 * source_type?)` — the source type as the bag named it, `undefined` when unset — returns the
 * records as a JSON string, or the `{ok: false, error}` it built for a parse failure
 * (`kind: 'parse'`), which the facade checks and hands back as it is. Anything an engine
 * throws passes through as itself. A fake engine rather than a built package, so this gates
 * in `deno task check` on a bare clone; `scripts/imports_suite.ts` grades the same surface
 * through the real engines.
 */

import { deepStrictEqual, strictEqual, throws } from 'node:assert';
import { test } from 'vitest';

import { create_imports_api } from '../crates/tsv_wasm/npm/api_imports.js';

/** A fake engine over both languages that records its calls and answers with `answer`. */
function fake_imports(answer: (source: string) => unknown) {
	const calls: Array<Array<unknown>> = [];
	const engine =
		(language: string) =>
		(...args: Array<unknown>) => {
			calls.push([language, ...args]);
			return answer(args[0] as string);
		};
	const api = create_imports_api({ typescript: engine('typescript'), svelte: engine('svelte') });
	return { api, calls };
}

const RECORDS = {
	hasModuleSyntax: true,
	imports: [
		{
			kind: 'static',
			start: 0,
			end: 12,
			specifier: { value: 'a', start: 8, end: 9 },
			typeOnly: null,
			phase: null,
			attributes: null
		}
	]
};

const PARSE_ERROR = {
	kind: 'parse',
	message: "Expected expression, found ';'",
	frame: '1:7 let = ;\n          ^ here',
	start: 6,
	end: 6,
	loc: { start: { line: 1, column: 6 }, end: { line: 1, column: 6 } }
};

test('imports: the records, as an ok result', () => {
	const { api, calls } = fake_imports(() => JSON.stringify(RECORDS));
	const result = api.parse_imports_typescript("import 'a';");
	deepStrictEqual(result, { ok: true, ...RECORDS });
	deepStrictEqual(Object.keys(result), ['ok', 'hasModuleSyntax', 'imports']);
	deepStrictEqual(api.parse_imports_svelte('<script>import "a";</script>'), result);
	// each engine takes the source and the source type, unset here
	deepStrictEqual(calls, [
		['typescript', "import 'a';", undefined],
		['svelte', '<script>import "a";</script>', undefined]
	]);
});

test('imports: a returned parse failure is the result, as it is', () => {
	const failure = { ok: false, error: PARSE_ERROR };
	const { api } = fake_imports(() => failure);
	strictEqual(api.parse_imports_typescript('let = ;'), failure);
	strictEqual(api.parse_imports_svelte('<div'), failure);
});

test('imports: invalid JSON from the engine is thrown as an engine bug', () => {
	const { api } = fake_imports(() => '{"hasModuleSyntax":');
	throws(
		() => api.parse_imports_typescript('x'),
		(e: unknown) =>
			e instanceof Error &&
			e.message === 'internal error: import records serialized to invalid JSON' &&
			e.cause instanceof SyntaxError
	);
});

test('imports: a malformed returned failure is thrown as an engine bug', () => {
	const returned = [
		// a kind the imports parse never returns
		{ ok: false, error: { ...PARSE_ERROR, kind: 'refused' } },
		{ ok: false, error: { ...PARSE_ERROR, kind: 'unsupported', construct: 'enum' } },
		{ ok: false, error: { ...PARSE_ERROR, start: 0.5 } },
		{ ok: false },
		42,
		undefined
	];
	for (const value of returned) {
		const { api } = fake_imports(() => value);
		throws(
			() => api.parse_imports_typescript('x'),
			(e: unknown) =>
				e instanceof Error && e.message.startsWith('internal error:') && e.cause === value
		);
	}
});

test('imports: anything an engine throws passes through as the same object', () => {
	class RuntimeError extends Error {}
	const thrown = [
		new Error('File too large: 5000000000 bytes (maximum: 4294967295 bytes / 4 GiB)'),
		new RuntimeError('unreachable'),
		new RangeError('Maximum call stack size exceeded'),
		{ ok: false, error: PARSE_ERROR }
	];
	for (const value of thrown) {
		const { api } = fake_imports(() => {
			throw value;
		});
		throws(
			() => api.parse_imports_typescript('x'),
			(e: unknown) => e === value
		);
	}
});

test('imports: the options bag — the source type forwarded, unknown keys refused', () => {
	const { api, calls } = fake_imports(() => JSON.stringify(RECORDS));
	for (const options of [
		undefined,
		null,
		{},
		{ sourceType: 'module' },
		{ sourceType: 'script' },
		{ sourceType: undefined }
	]) {
		strictEqual(api.parse_imports_typescript('x', options).ok, true);
	}
	// a named goal reaches the engine as named; an unset one stays unset, never `'module'`
	deepStrictEqual(
		calls.map((call) => call[2]),
		[undefined, undefined, undefined, 'module', 'script', undefined]
	);
	// Svelte takes the forwarded `undefined` and refuses a set one
	strictEqual(api.parse_imports_svelte('x', { sourceType: undefined }).ok, true);
	const refusals: Array<[string, unknown, string]> = [
		['typescript', 'module', 'imports options must be an object'],
		['typescript', ['module'], 'imports options must be an object'],
		[
			'typescript',
			{ locations: true },
			"unknown imports option 'locations' (expected 'sourceType')"
		],
		[
			'typescript',
			{ sourceType: 'sloppy' },
			"imports option 'sourceType' must be 'script' or 'module' (got 'sloppy')"
		],
		[
			'svelte',
			{ sourceType: 'module' },
			"imports option 'sourceType' is only supported for TypeScript"
		],
		[
			'svelte',
			{ locations: false },
			"unknown imports option 'locations' (this export takes no options)"
		]
	];
	const before = calls.length;
	for (const [language, options, message] of refusals) {
		throws(
			() => api[`parse_imports_${language}`]('x', options),
			(e: unknown) => e instanceof TypeError && e.message === message,
			message
		);
	}
	strictEqual(calls.length, before, 'a refused bag never reaches the engine');
});

test('imports: a bad source is a TypeError that never reaches the engine', () => {
	const { api, calls } = fake_imports(() => JSON.stringify(RECORDS));
	const refusals: Array<[unknown, string]> = [
		[42, 'imports source must be a string (got number)'],
		[null, 'imports source must be a string (got null)'],
		['a\uD800', 'imports source must be well-formed UTF-16 (a lone surrogate at offset 1)']
	];
	for (const [source, message] of refusals) {
		throws(
			() => api.parse_imports_svelte(source),
			(e: unknown) => e instanceof TypeError && e.message === message,
			message
		);
	}
	strictEqual(calls.length, 0);
});

test('imports: each function is named for its export', () => {
	const { api } = fake_imports(() => JSON.stringify(RECORDS));
	strictEqual(api.parse_imports_typescript.name, 'parse_imports_typescript');
	strictEqual(api.parse_imports_svelte.name, 'parse_imports_svelte');
	deepStrictEqual(Object.keys(api), ['parse_imports_typescript', 'parse_imports_svelte']);
});
