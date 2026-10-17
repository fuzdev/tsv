/**
 * The npm packages' strip facade (`crates/tsv_wasm/npm/api_strip.js`) over a FAKE engine:
 * the result object a strip outcome becomes, the raw engine's error shapes it reads, the
 * options bag, and the argument refusals, word for word.
 *
 * The raw engine contract (both `tsv_wasm` and `tsv_napi`): `strip_typescript(source)`
 * returns the code, or throws an `Error` whose own ENUMERABLE integer properties carry the
 * failure — `start`, `line`, `column` for a parse failure (the shape every parse export
 * throws), and those plus a string `kind` (the refused construct) and `end`, `end_line`,
 * `end_column` for a refusal. Anything else an engine throws passes through as itself. A
 * fake engine rather than a built package, so this gates in `deno task check` on a bare
 * clone; `scripts/strip_suite.ts` grades the same surface through the real engines.
 */

import { deepStrictEqual, strictEqual, throws } from 'node:assert';

import { create_strip_api } from '../crates/tsv_wasm/npm/api_strip.js';

/** A fake engine that records its calls and answers with `answer`. */
function fake_strip(answer: (source: string) => string) {
	const calls: Array<Array<unknown>> = [];
	const api = create_strip_api({
		typescript: (...args: Array<unknown>) => {
			calls.push(args);
			return answer(args[0] as string);
		}
	});
	return { api, calls };
}

/** An engine failure as both engines throw one: an `Error` with own enumerable `extra`. */
const engine_error = (message: string, extra: Record<string, unknown>): Error =>
	Object.assign(new Error(message), extra);

const PARSE_MESSAGE = "Expected expression, found ';'\n1:7 let = ;\n          ^ here";
const REFUSAL_MESSAGE = 'cannot strip an enum\n2:1 enum E {}\n    ^ here';

Deno.test('strip: the code, as an ok result', () => {
	const { api, calls } = fake_strip((source) => source.replace(': T', '   '));
	const result = api.strip_typescript('let x: T;');
	deepStrictEqual(result, { ok: true, code: 'let x   ;' });
	deepStrictEqual(Object.keys(result), ['ok', 'code']);
	// the engine takes the source alone: the strip has one goal
	deepStrictEqual(calls, [['let x: T;']]);
});

Deno.test('strip: a pointed engine error is a parse error result, never a throw', () => {
	const { api } = fake_strip(() => {
		throw engine_error(PARSE_MESSAGE, { start: 6, line: 1, column: 6 });
	});
	const result = api.strip_typescript('let = ;');
	deepStrictEqual(result, {
		ok: false,
		error: { kind: 'parse', message: PARSE_MESSAGE, start: 6, loc: { line: 1, column: 6 } }
	});
	deepStrictEqual(Object.keys(result.error), ['kind', 'message', 'start', 'loc']);
	strictEqual(result.error instanceof Error, false);
});

Deno.test('strip: a refusal is an unsupported result with the construct and its span', () => {
	const { api } = fake_strip(() => {
		throw engine_error(REFUSAL_MESSAGE, {
			kind: 'enum',
			start: 4,
			line: 2,
			column: 0,
			end: 13,
			end_line: 2,
			end_column: 9
		});
	});
	const result = api.strip_typescript('x;\n\nenum E {}');
	deepStrictEqual(result, {
		ok: false,
		error: {
			kind: 'unsupported',
			construct: 'enum',
			message: REFUSAL_MESSAGE,
			start: 4,
			end: 13,
			loc: { start: { line: 2, column: 0 }, end: { line: 2, column: 9 } }
		}
	});
	deepStrictEqual(Object.keys(result.error), [
		'kind',
		'construct',
		'message',
		'start',
		'end',
		'loc'
	]);
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
		const { api } = fake_strip(() => {
			throw engine_error('m', {
				kind: construct,
				start: 0,
				line: 1,
				column: 0,
				end: 1,
				end_line: 1,
				end_column: 1
			});
		});
		const result = api.strip_typescript('x');
		strictEqual(result.ok, false, construct);
		strictEqual(result.error.construct, construct);
	}
});

Deno.test('strip: anything else an engine throws passes through as the same object', () => {
	class RuntimeError extends Error {}
	const bun_shaped = new Error('x');
	// Bun's own non-enumerable numeric `line` / `column` on every `Error`: not a point
	for (const key of ['line', 'column']) {
		Object.defineProperty(bun_shaped, key, { value: 1, enumerable: false });
	}
	const thrown = [
		new Error('File too large: 5000000000 bytes (maximum: 4294967295 bytes / 4 GiB)'),
		new RuntimeError('unreachable'),
		new RangeError('Maximum call stack size exceeded'),
		Object.assign(bun_shaped, { start: 0 }),
		// a refusal naming a construct the facade does not know is an engine bug
		engine_error('m', {
			kind: 'decorator',
			start: 0,
			line: 1,
			column: 0,
			end: 1,
			end_line: 1,
			end_column: 1
		}),
		// a point with a non-integer property is no point
		engine_error('m', { start: 0.5, line: 1, column: 0 }),
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

Deno.test('strip: the options bag — module only, unknown keys refused', () => {
	const { api, calls } = fake_strip((source) => source);
	for (const options of [
		undefined,
		null,
		{},
		{ sourceType: 'module' },
		{ sourceType: undefined }
	]) {
		deepStrictEqual(api.strip_typescript('x', options), { ok: true, code: 'x' });
	}
	strictEqual(calls.length, 5);
	const refusals: Array<[unknown, string]> = [
		['module', 'strip options must be an object'],
		[['module'], 'strip options must be an object'],
		[42, 'strip options must be an object'],
		[{ locations: true }, "unknown strip option 'locations' (expected 'sourceType')"],
		[{ locatons: undefined }, "unknown strip option 'locatons' (expected 'sourceType')"],
		[{ sourceType: 'script' }, "strip option 'sourceType' must be 'module' (got 'script')"],
		[{ sourceType: 1 }, "strip option 'sourceType' must be 'module' (got 1)"]
	];
	for (const [options, message] of refusals) {
		throws(
			() => api.strip_typescript('x', options),
			(e: unknown) => e instanceof TypeError && e.message === message,
			message
		);
	}
	strictEqual(calls.length, 5, 'a refused bag never reaches the engine');
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
