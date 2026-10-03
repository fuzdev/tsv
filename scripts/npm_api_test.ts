/**
 * The npm packages' facade (`crates/tsv_wasm/npm/api.js` + `api_parse.js`) over a FAKE
 * engine: the options reader every package shares, its refusals word for word, the
 * source-type forwarding, and the `{locations: true}` sugar.
 *
 * A fake engine rather than a built package on purpose: what this grades is the
 * facade's own logic, which needs no artifact — so it gates in `deno task check`
 * (`test:deno`) on a bare clone. The package suites (`scripts/test_npm.ts`,
 * `scripts/test_napi_npm.ts`) grade the same surface through the real engines, and
 * `deno task check:loc` grades the reconstruction itself against the Rust emitter.
 */

import { deepStrictEqual, strictEqual, throws } from 'node:assert';

import { create_format_api, read_options } from '../crates/tsv_wasm/npm/api.js';
import { create_parse_api } from '../crates/tsv_wasm/npm/api_parse.js';
import { reconstruct_locations } from '../crates/tsv_wasm/npm/locations.js';

type Call = { source: string; source_type: string | undefined };

/** A span-only wire for `source`: a root spanning it, as every engine emits. */
const span_wire = (type: string, source: string): string =>
	JSON.stringify({ type, start: 0, end: source.length, body: [] });

/** A fake engine over all three languages that records every call it gets. */
function fake_engine(with_object_parse: boolean) {
	const calls: Array<Call & { op: string; language: string }> = [];
	const roots: Record<string, string> = {
		svelte: 'Root',
		typescript: 'Program',
		css: 'StyleSheetFile'
	};
	const family = <T>(op: string, f: (language: string, source: string) => T) =>
		Object.fromEntries(
			Object.keys(roots).map((language) => [
				language,
				(source: string, source_type?: string) => {
					calls.push({ op, language, source, source_type });
					return f(language, source);
				}
			])
		);
	return {
		calls,
		engine: {
			parse_json: family('parse_json', (language, source) => span_wire(roots[language], source)),
			...(with_object_parse
				? {
						parse: family('parse', (language, source) =>
							JSON.parse(span_wire(roots[language], source))
						)
					}
				: {}),
			format: family('format', (_language, source) => `formatted:${source}`)
		}
	};
}

/** Assert `f` throws an `Error` whose message is exactly `message`. */
const throws_exactly = (f: () => unknown, message: string): void => {
	throws(f, (e: unknown) => e instanceof Error && e.message === message);
};

Deno.test('read_options: defaults — locations off, the source type unset', () => {
	for (const options of [undefined, null, {}]) {
		deepStrictEqual(read_options(options, 'parse', true, true), {
			locations: false,
			source_type: undefined
		});
	}
	// a supported key set to `undefined` is its default, sourceType included on a
	// language that refuses a set one — the forwarding idiom
	deepStrictEqual(
		read_options({ locations: undefined, sourceType: undefined }, 'parse', true, false),
		{ locations: false, source_type: undefined }
	);
	deepStrictEqual(read_options({ locations: true, sourceType: 'script' }, 'parse', true, true), {
		locations: true,
		source_type: 'script'
	});
});

Deno.test('read_options: every refusal, word for word', () => {
	throws_exactly(
		() => read_options('script', 'parse', true, true),
		'parse options must be an object'
	);
	throws_exactly(
		() => read_options(['script'], 'format', false, true),
		'format options must be an object'
	);
	throws_exactly(
		() => read_options({ locations: 'yes' }, 'parse', true, true),
		"parse option 'locations' must be a boolean"
	);
	throws_exactly(
		() => read_options({ sourceType: 'module' }, 'parse', true, false),
		"parse option 'sourceType' is only supported for TypeScript"
	);
	throws_exactly(
		() => read_options({ sourceType: true }, 'format', false, true),
		"format option 'sourceType' must be 'script' or 'module'"
	);
	throws_exactly(
		() => read_options({ sourceType: 'sloppy' }, 'parse', true, true),
		"invalid sourceType 'sloppy' (expected 'script' or 'module')"
	);
	// an unknown key errors whatever its value, `undefined` included
	throws_exactly(
		() => read_options({ locatons: undefined }, 'parse', true, true),
		"unknown parse option 'locatons' (expected 'locations' or 'sourceType')"
	);
	throws_exactly(
		() => read_options({ x: 1 }, 'parse', true, false),
		"unknown parse option 'x' (expected 'locations')"
	);
	throws_exactly(
		() => read_options({ locations: false }, 'parse', false, true),
		"unknown parse option 'locations' (expected 'sourceType')"
	);
	throws_exactly(
		() => read_options({ anything: true }, 'format', false, false),
		"unknown format option 'anything' (this export takes no options)"
	);
});

Deno.test('read_options: a throwing getter is named, its error kept as the cause', () => {
	const getter_error = new Error('getter exploded');
	const bag = (name: string) =>
		Object.defineProperty({}, name, {
			enumerable: true,
			get() {
				throw getter_error;
			}
		});
	for (const [name, noun] of [
		['locations', 'parse'],
		['sourceType', 'format']
	] as const) {
		throws(
			() => read_options(bag(name), noun, true, true),
			(e: unknown) =>
				e instanceof Error &&
				e.message === `failed to read ${noun} option '${name}'` &&
				e.cause === getter_error
		);
	}
	// an unknown key is refused before it is read, so its getter never runs
	throws_exactly(
		() => read_options(bag('locatons'), 'parse', true, true),
		"unknown parse option 'locatons' (expected 'locations' or 'sourceType')"
	);
});

Deno.test('read_options: a non-plain object reads as a keyless bag — all defaults', () => {
	for (const options of [new Date(0), new Map([['locations', true]])]) {
		deepStrictEqual(read_options(options, 'parse', true, true), {
			locations: false,
			source_type: undefined
		});
	}
});

Deno.test('parse: the span tree by default, `loc` reconstructed on request', () => {
	for (const with_object_parse of [true, false]) {
		const { engine, calls } = fake_engine(with_object_parse);
		const api = create_parse_api(engine);
		const source = 'let x = 1;\nx;';
		const ast = api.parse_typescript(source);
		deepStrictEqual(ast, JSON.parse(span_wire('Program', source)));
		strictEqual(calls.at(-1)?.op, with_object_parse ? 'parse' : 'parse_json');
		const located = api.parse_typescript(source, { locations: true });
		deepStrictEqual(
			located,
			reconstruct_locations(JSON.parse(span_wire('Program', source)), source, {
				language: 'typescript'
			})
		);
		deepStrictEqual(located.loc, { start: { line: 1, column: 0 }, end: { line: 2, column: 2 } });
	}
});

Deno.test('parse: the language reaches the reconstruction (Svelte counts LF alone)', () => {
	const { engine } = fake_engine(false);
	const api = create_parse_api(engine);
	// a lone CR: a line under ECMAScript's rule, none under Svelte's
	const source = 'a\rb';
	strictEqual(api.parse_svelte(source, { locations: true }).loc.end.line, 1);
	strictEqual(api.parse_typescript(source, { locations: true }).loc.end.line, 2);
});

Deno.test('reconstruct_locations: the language is inferred from a root, never guessed', () => {
	// each parse root names its document
	const lf_cr = 'a\rb';
	for (const [type, line] of [
		['Root', 1],
		['Program', 2],
		['StyleSheetFile', 1]
	] as const) {
		const ast = reconstruct_locations(JSON.parse(span_wire(type, lf_cr)), lf_cr);
		strictEqual(ast.loc.end.line, line, type);
	}
	// a subtree names no document: refused, rather than read under TypeScript's rule
	const fragment = { type: 'Fragment', start: 0, end: 3, nodes: [] };
	throws_exactly(
		() => reconstruct_locations(fragment, lf_cr),
		"locations: cannot infer the document's language from a 'Fragment' root — pass " +
			"{language: 'typescript' | 'svelte' | 'css'}, or the parse's own root " +
			'(Root, Program or StyleSheetFile)'
	);
	throws_exactly(
		() => reconstruct_locations({ start: 0, end: 3 }, lf_cr),
		"locations: cannot infer the document's language from a root with no type — pass " +
			"{language: 'typescript' | 'svelte' | 'css'}, or the parse's own root " +
			'(Root, Program or StyleSheetFile)'
	);
	// a Svelte `<script>`'s program is a `Program` that starts after its tag: refused,
	// rather than read as a TypeScript root (ECMAScript terminators, a counted BOM)
	const svelte_source = '<script>\na\rb\n</script>';
	const script_program = { type: 'Program', start: 8, end: 13, body: [], sourceType: 'module' };
	throws_exactly(
		() => reconstruct_locations(script_program, svelte_source),
		"locations: cannot infer the document's language from a 'Program' that does not " +
			"span the source — pass {language: 'typescript' | 'svelte' | 'css'}, or the " +
			"parse's own root (Root, Program or StyleSheetFile)"
	);
	strictEqual(
		reconstruct_locations(script_program, svelte_source, { language: 'svelte' }).loc.end.line,
		3
	);
	// named, a subtree reconstructs under that language's rule
	strictEqual(reconstruct_locations(fragment, lf_cr, { language: 'svelte' }).loc.end.line, 1);
	// a named language that is not one of the three keeps the locator's own refusal
	throws(
		() => reconstruct_locations(fragment, lf_cr, { language: 'html' as 'css' }),
		/`language` must be 'typescript', 'svelte' or 'css'.*\(got 'html'\)/
	);
});

Deno.test('reconstruct_locations: depth costs no JS stack', () => {
	// far past where a frame-per-level walk throws a `RangeError` on every runtime; each
	// level nests an array too, which a recursive walk pays a frame for as well
	const depth = 100_000;
	let element: Record<string, unknown> = { type: 'Text', start: 0, end: 1, raw: 'x', data: 'x' };
	const innermost = element;
	for (let i = 0; i < depth; i++) {
		element = {
			type: 'RegularElement',
			start: 0,
			end: 1,
			name: 'div',
			attributes: [],
			fragment: { type: 'Fragment', nodes: [[element]] }
		};
	}
	const root = { type: 'Root', start: 0, end: 1, fragment: { type: 'Fragment', nodes: [element] } };
	reconstruct_locations(root, 'x');
	deepStrictEqual(innermost.loc, { start: { line: 1, column: 0 }, end: { line: 1, column: 1 } });
});

Deno.test(
	'reconstruct_locations: an in-tag comment is told by the key order its collector wrote',
	() => {
		// Svelte's template reader writes `{type, start, end, value}` (an in-tag comment, which
		// gets `character`); acorn's `onComment` wrapper `{type, value, start, end}` (the plain
		// shape) — whatever the tree around them says
		const source = '<div /* a */ title={/* b */ x}></div>';
		const at = (needle: string) => source.indexOf(needle);
		const template_reader: Record<string, unknown> = {
			type: 'Block',
			start: at('/* a'),
			end: at('/* a') + 7,
			value: ' a '
		};
		const acorn: Record<string, unknown> = {
			type: 'Block',
			value: ' b ',
			start: at('/* b'),
			end: at('/* b') + 7
		};
		const root = { type: 'Root', start: 0, end: source.length, comments: [acorn, template_reader] };
		reconstruct_locations(root, source);
		deepStrictEqual(template_reader.loc, {
			start: { line: 1, column: 5, character: 5 },
			end: { line: 1, column: 12, character: 12 }
		});
		deepStrictEqual(acorn.loc, {
			start: { line: 1, column: at('/* b') },
			end: { line: 1, column: at('/* b') + 7 }
		});
	}
);

Deno.test('parse: the source type is forwarded, unset when not named', () => {
	const { engine, calls } = fake_engine(false);
	const api = create_parse_api(engine);
	api.parse_typescript('x', { sourceType: 'script' });
	api.parse_typescript_json('x');
	api.parse_typescript_json('x', { sourceType: 'module' });
	api.parse_svelte('x', { sourceType: undefined });
	deepStrictEqual(
		calls.map((c) => c.source_type),
		['script', undefined, 'module', undefined]
	);
});

Deno.test('parse_*_json: the wire string untouched, and no `locations` key', () => {
	const { engine } = fake_engine(true);
	const api = create_parse_api(engine);
	strictEqual(api.parse_css_json('a{}'), span_wire('StyleSheetFile', 'a{}'));
	throws_exactly(
		() => api.parse_typescript_json('x', { locations: false }),
		"unknown parse option 'locations' (expected 'sourceType')"
	);
	throws_exactly(
		() => api.parse_css_json('a{}', { locations: false }),
		"unknown parse option 'locations' (this export takes no options)"
	);
	throws_exactly(
		() => api.parse_svelte('x', { sourceType: 'module' }),
		"parse option 'sourceType' is only supported for TypeScript"
	);
});

Deno.test('format: the source type forwarded, `locations` unknown', () => {
	const { engine, calls } = fake_engine(false);
	const api = create_format_api(engine.format);
	strictEqual(api.format_typescript('x', { sourceType: 'script' }), 'formatted:x');
	strictEqual(api.format_css('a{}'), 'formatted:a{}');
	deepStrictEqual(
		calls.map((c) => [c.language, c.source_type]),
		[
			['typescript', 'script'],
			['css', undefined]
		]
	);
	throws_exactly(
		() => api.format_typescript('x', { locations: false }),
		"unknown format option 'locations' (expected 'sourceType')"
	);
	throws_exactly(
		() => api.format_svelte('x', { locations: false }),
		"unknown format option 'locations' (this export takes no options)"
	);
	throws_exactly(
		() => api.format_css('x', { sourceType: 'script' }),
		"format option 'sourceType' is only supported for TypeScript"
	);
});

Deno.test('a non-string source is refused before the engine sees it', () => {
	const { engine, calls } = fake_engine(false);
	const api = { ...create_parse_api(engine), ...create_format_api(engine.format) };
	for (const [name, noun] of [
		['parse_typescript', 'parse'],
		['parse_svelte_json', 'parse'],
		['format_css', 'format']
	] as const) {
		for (const [source, got] of [
			[42, 'number'],
			[null, 'null'],
			[undefined, 'undefined'],
			[{}, 'object']
		] as const) {
			throws(
				() => (api[name] as (source: unknown) => unknown)(source),
				(error: unknown) =>
					error instanceof TypeError &&
					error.message === `${noun} source must be a string (got ${got})`
			);
		}
	}
	strictEqual(calls.length, 0);
});

Deno.test('the facade builds exactly the engine families it is handed', () => {
	const { engine } = fake_engine(false);
	deepStrictEqual(Object.keys(create_format_api(engine.format)).sort(), [
		'format_css',
		'format_svelte',
		'format_typescript'
	]);
	deepStrictEqual(Object.keys(create_parse_api({ parse_json: engine.parse_json })).sort(), [
		'parse_css',
		'parse_css_json',
		'parse_svelte',
		'parse_svelte_json',
		'parse_typescript',
		'parse_typescript_json'
	]);
});
