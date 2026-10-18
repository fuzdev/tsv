/**
 * The npm packages' strip facade (`crates/tsv_wasm/npm/api_strip.js`) over a FAKE engine:
 * the result object a strip outcome becomes, the raw engine's error shapes it reads, the
 * options bag, and the argument refusals, word for word.
 *
 * The raw engine contract (both `tsv_wasm` and `tsv_napi`): `strip_typescript(source,
 * source_type?)` — the source type as the bag named it, `undefined` when unset — returns the
 * code, or the `{ok: false, error}` it built for a parse failure (`kind: 'parse'`) or a
 * refusal (`kind: 'unsupported'`, its `construct`), which the facade checks and hands back as
 * it is. Anything an engine throws passes through as itself. A fake engine rather than a
 * built package, so this gates in `deno task check` on a bare clone; `scripts/strip_suite.ts`
 * grades the same surface through the real engines.
 */

import { deepStrictEqual, strictEqual, throws } from 'node:assert';

import { create_strip_api } from '../crates/tsv_wasm/npm/api_strip.js';

/** A fake engine that records its calls and answers with `answer`. */
function fake_strip(answer: (source: string) => unknown) {
	const calls: Array<Array<unknown>> = [];
	const api = create_strip_api({
		typescript: (...args: Array<unknown>) => {
			calls.push(args);
			return answer(args[0] as string);
		}
	});
	return { api, calls };
}

/** A failure as both engines return one. */
const engine_failure = (error: Record<string, unknown>) => ({ ok: false, error });

const PARSE_ERROR = {
	kind: 'parse',
	message: "Expected expression, found ';'",
	frame: '1:7 let = ;\n          ^ here',
	start: 6,
	end: 6,
	loc: { start: { line: 1, column: 6 }, end: { line: 1, column: 6 } }
};

/** A refusal of `construct`, as both engines return one. */
const refusal = (construct: string) => ({
	kind: 'unsupported',
	construct,
	message: 'cannot strip an enum',
	frame: '2:1 enum E {}\n    ^ here',
	start: 4,
	end: 13,
	loc: { start: { line: 2, column: 0 }, end: { line: 2, column: 9 } }
});

Deno.test('strip: the code, as an ok result', () => {
	const { api, calls } = fake_strip((source) => source.replace(': T', '   '));
	const result = api.strip_typescript('let x: T;');
	deepStrictEqual(result, { ok: true, code: 'let x   ;' });
	deepStrictEqual(Object.keys(result), ['ok', 'code']);
	// the engine takes the source and the source type, unset here
	deepStrictEqual(calls, [['let x: T;', undefined]]);
});

Deno.test('strip: a returned parse failure is the result, as it is', () => {
	const failure = engine_failure(PARSE_ERROR);
	const { api } = fake_strip(() => failure);
	strictEqual(api.strip_typescript('let = ;'), failure);
});

Deno.test('strip: a returned refusal is the result, as it is', () => {
	const failure = engine_failure(refusal('enum'));
	const { api } = fake_strip(() => failure);
	strictEqual(api.strip_typescript('x;\n\nenum E {}'), failure);
});

Deno.test('strip: every construct the engines name is passed through by name', () => {
	for (const construct of [
		'enum',
		'value_namespace',
		'parameter_property',
		'import_equals',
		'export_equals',
		'type_assertion',
		'assertion_grouping'
	]) {
		const { api } = fake_strip(() => engine_failure(refusal(construct)));
		const result = api.strip_typescript('x');
		strictEqual(result.ok, false, construct);
		strictEqual(result.error.construct, construct);
	}
});

Deno.test('strip: a malformed returned failure is thrown as an engine bug', () => {
	const returned = [
		// a refusal naming a construct the facade does not know
		engine_failure(refusal('decorator')),
		// a kind the strip never returns
		engine_failure({ ...PARSE_ERROR, kind: 'refused' }),
		// a non-integer offset, a missing frame
		engine_failure({ ...PARSE_ERROR, start: 0.5 }),
		engine_failure({ ...PARSE_ERROR, frame: undefined }),
		{ ok: false },
		42,
		undefined
	];
	for (const value of returned) {
		const { api } = fake_strip(() => value);
		throws(
			() => api.strip_typescript('x'),
			(e: unknown) =>
				e instanceof Error && e.message.startsWith('internal error:') && e.cause === value
		);
	}
});

Deno.test('strip: anything an engine throws passes through as the same object', () => {
	class RuntimeError extends Error {}
	const thrown = [
		new Error('File too large: 5000000000 bytes (maximum: 4294967295 bytes / 4 GiB)'),
		new RuntimeError('unreachable'),
		new RangeError('Maximum call stack size exceeded'),
		// a failure is returned, never thrown: a thrown one is still a throw
		engine_failure(PARSE_ERROR),
		'a string'
	];
	for (const value of thrown) {
		const { api } = fake_strip(() => {
			throw value;
		});
		throws(
			() => api.strip_typescript('x'),
			(e: unknown) => e === value
		);
	}
});

Deno.test('strip: the options bag — the source type forwarded, unknown keys refused', () => {
	const { api, calls } = fake_strip((source) => source);
	for (const options of [
		undefined,
		null,
		{},
		{ sourceType: 'module' },
		{ sourceType: 'script' },
		{ sourceType: undefined }
	]) {
		deepStrictEqual(api.strip_typescript('x', options), { ok: true, code: 'x' });
	}
	// a named goal reaches the engine as named; an unset one stays unset, never `'module'`
	deepStrictEqual(
		calls.map((call) => call[1]),
		[undefined, undefined, undefined, 'module', 'script', undefined]
	);
	const refusals: Array<[unknown, string]> = [
		['module', 'strip options must be an object'],
		[['module'], 'strip options must be an object'],
		[42, 'strip options must be an object'],
		[{ locations: true }, "unknown strip option 'locations' (expected 'sourceType')"],
		[{ locatons: undefined }, "unknown strip option 'locatons' (expected 'sourceType')"],
		[
			{ sourceType: 'sloppy' },
			"strip option 'sourceType' must be 'script' or 'module' (got 'sloppy')"
		],
		[{ sourceType: 1 }, "strip option 'sourceType' must be 'script' or 'module' (got 1)"]
	];
	for (const [options, message] of refusals) {
		throws(
			() => api.strip_typescript('x', options),
			(e: unknown) => e instanceof TypeError && e.message === message,
			message
		);
	}
	strictEqual(calls.length, 6, 'a refused bag never reaches the engine');
});

Deno.test('strip: a bad source is a TypeError that never reaches the engine', () => {
	const { api, calls } = fake_strip((source) => source);
	const refusals: Array<[unknown, string]> = [
		[42, 'strip source must be a string (got number)'],
		[null, 'strip source must be a string (got null)'],
		[undefined, 'strip source must be a string (got undefined)'],
		['a\uD800', 'strip source must be well-formed UTF-16 (a lone surrogate at offset 1)']
	];
	for (const [source, message] of refusals) {
		throws(
			() => api.strip_typescript(source),
			(e: unknown) => e instanceof TypeError && e.message === message,
			message
		);
	}
	strictEqual(calls.length, 0);
});

Deno.test('strip: the function is named for its export', () => {
	const { api } = fake_strip((source) => source);
	strictEqual(api.strip_typescript.name, 'strip_typescript');
	deepStrictEqual(Object.keys(api), ['strip_typescript']);
});
