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
import { create_locator, reconstruct_locations } from '../crates/tsv_wasm/npm/locations.js';

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

/** Assert `f` throws a `TypeError` — the class of every argument refusal — whose message is
 * exactly `message`. */
const throws_type_error = (f: () => unknown, message: string): void => {
	throws(f, (e: unknown) => e instanceof TypeError && e.message === message);
};

/** Assert `f` throws a `RangeError` whose message matches `pattern`. */
const throws_range_error = (f: () => unknown, pattern: RegExp): void => {
	throws(f, (e: unknown) => e instanceof RangeError && pattern.test(e.message));
};

Deno.test('read_options: defaults — locations off, the source type unset', () => {
	for (const options of [undefined, null, {}]) {
		deepStrictEqual(read_options(options, 'parse', 'typescript'), {
			locations: false,
			source_type: undefined
		});
	}
	// a supported key set to `undefined` is its default, sourceType included on a
	// language that refuses a set one — the forwarding idiom
	for (const kind of ['parse', 'parse_json', 'format'] as const) {
		deepStrictEqual(read_options({ sourceType: undefined }, kind, 'svelte'), {
			locations: false,
			source_type: undefined
		});
	}
	deepStrictEqual(read_options({ locations: undefined, sourceType: undefined }, 'parse', 'css'), {
		locations: false,
		source_type: undefined
	});
	deepStrictEqual(read_options({ locations: true, sourceType: 'script' }, 'parse', 'typescript'), {
		locations: true,
		source_type: 'script'
	});
	deepStrictEqual(read_options({ sourceType: 'module' }, 'parse_json', 'typescript'), {
		locations: false,
		source_type: 'module'
	});
	deepStrictEqual(read_options({ sourceType: 'script' }, 'format', 'typescript'), {
		locations: false,
		source_type: 'script'
	});
});

Deno.test('read_options: every refusal, word for word', () => {
	throws_type_error(
		() => read_options('script', 'parse', 'typescript'),
		'parse options must be an object'
	);
	throws_type_error(
		() => read_options(['script'], 'format', 'typescript'),
		'format options must be an object'
	);
	throws_type_error(
		() => read_options({ locations: 'yes' }, 'parse', 'typescript'),
		"parse option 'locations' must be a boolean (got 'yes')"
	);
	throws_type_error(
		() => read_options({ locations: null }, 'parse', 'svelte'),
		"parse option 'locations' must be a boolean (got null)"
	);
	throws_type_error(
		() => read_options({ sourceType: 'module' }, 'parse', 'svelte'),
		"parse option 'sourceType' is only supported for TypeScript"
	);
	// one text for every value that is not a goal, the value described
	for (const [value, got] of [
		[true, 'true'],
		['sloppy', "'sloppy'"],
		['', "''"],
		[null, 'null'],
		[1, '1'],
		[{}, 'object']
	] as const) {
		throws_type_error(
			() => read_options({ sourceType: value }, 'format', 'typescript'),
			`format option 'sourceType' must be 'script' or 'module' (got ${got})`
		);
	}
	throws_type_error(
		() => read_options({ sourceType: 'sloppy' }, 'parse', 'typescript'),
		"parse option 'sourceType' must be 'script' or 'module' (got 'sloppy')"
	);
	// a refused string is echoed escaped — no raw line break, control character, bidi
	// control or lone surrogate reaches the message, and a quote cannot close the literal
	// early — and clipped past 40 UTF-16 units, the `…` inside the quotes, never splitting
	// a surrogate pair. `locations.js` restates the rule, so both copies run the table.
	for (const [value, got] of [
		['a\nb', String.raw`'a\nb'`],
		[`it's "x"`, String.raw`'it\'s "x"'`],
		['\u0000\\', String.raw`'\u0000\\'`],
		['\uD800', String.raw`'\ud800'`],
		['a\u2028b\u2029', String.raw`'a\u2028b\u2029'`],
		['\u0085\u009b[1m\u007f', String.raw`'\u0085\u009b[1m\u007f'`],
		['\u202ex\u2066', String.raw`'\u202ex\u2066'`],
		['\u200e\u200f\u202a\u2069\u061c', String.raw`'\u200e\u200f\u202a\u2069\u061c'`],
		['x'.repeat(40), `'${'x'.repeat(40)}'`],
		['x'.repeat(41), `'${'x'.repeat(40)}…'`],
		['x'.repeat(39) + '\u{1F600}', `'${'x'.repeat(39)}…'`],
		['y\n'.repeat(1000), `'${String.raw`y\n`.repeat(20)}…'`]
	]) {
		throws_type_error(
			() => read_options({ sourceType: value }, 'format', 'typescript'),
			`format option 'sourceType' must be 'script' or 'module' (got ${got})`
		);
		throws_type_error(
			() => create_locator('a', { language: value as 'css' }),
			`locations option 'language' must be 'typescript', 'svelte' or 'css' (got ${got})`
		);
	}
	// an unknown key errors whatever its value, `undefined` included; the detail names the
	// export's own key set
	throws_type_error(
		() => read_options({ locatons: undefined }, 'parse', 'typescript'),
		"unknown parse option 'locatons' (expected 'locations' or 'sourceType')"
	);
	throws_type_error(
		() => read_options({ x: 1 }, 'parse', 'svelte'),
		"unknown parse option 'x' (expected 'locations')"
	);
	throws_type_error(
		() => read_options({ x: 1 }, 'parse_json', 'typescript'),
		"unknown parse option 'x' (expected 'sourceType')"
	);
	// an unknown key is quoted by the same rule as a refused value, in both copies
	throws_type_error(
		() => read_options({ 'k\u202e\n': 1 }, 'parse', 'svelte'),
		String.raw`unknown parse option 'k\u202e\n' (expected 'locations')`
	);
	throws_type_error(
		() => create_locator('a', { 'k\u202e\n': 1 } as never),
		String.raw`unknown locations option 'k\u202e\n' (expected 'language')`
	);
	throws_type_error(
		() => read_options({ locations: false }, 'format', 'typescript'),
		"unknown format option 'locations' (expected 'sourceType')"
	);
	throws_type_error(
		() => read_options({ anything: true }, 'format', 'css'),
		"unknown format option 'anything' (this export takes no options)"
	);
	// a `_json` export: `locations` gets its own explanation, naming the export's language,
	// and any other unknown key the ordinary refusal
	throws_type_error(
		() => read_options({ locations: undefined }, 'parse_json', 'typescript'),
		"parse option 'locations' is not supported by parse_typescript_json — the JSON string " +
			'is the span-only wire; use parse_typescript(source, {locations: true})'
	);
	throws_type_error(
		() => read_options({ x: 1 }, 'parse_json', 'css'),
		"unknown parse option 'x' (this export takes no options)"
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
			() => read_options(bag(name), noun, 'typescript'),
			(e: unknown) =>
				e instanceof TypeError &&
				e.message === `failed to read ${noun} option '${name}'` &&
				e.cause === getter_error
		);
	}
	// an unknown key is refused before it is read, so its getter never runs
	throws_type_error(
		() => read_options(bag('locatons'), 'parse', 'typescript'),
		"unknown parse option 'locatons' (expected 'locations' or 'sourceType')"
	);
});

Deno.test('read_options: a non-plain object reads as a keyless bag — all defaults', () => {
	for (const options of [new Date(0), new Map([['locations', true]])]) {
		deepStrictEqual(read_options(options, 'parse', 'typescript'), {
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
		const tree = JSON.parse(span_wire(type, lf_cr));
		const ast = reconstruct_locations(tree, lf_cr);
		strictEqual(ast, tree, 'mutated in place and returned');
		strictEqual(ast.loc.end.line, line, type);
	}
	// a subtree names no document: refused, rather than read under TypeScript's rule
	const fragment = { type: 'Fragment', start: 0, end: 3, nodes: [] };
	throws_type_error(
		() => reconstruct_locations(fragment, lf_cr),
		"locations option 'language' is required: cannot infer the document's language from " +
			"a 'Fragment' root — pass {language: 'typescript' | 'svelte' | 'css'}, or the " +
			"parse's own root (Root, Program or StyleSheetFile)"
	);
	throws_type_error(
		() => reconstruct_locations({ start: 0, end: 3 }, lf_cr),
		"locations option 'language' is required: cannot infer the document's language from " +
			"a root with no type — pass {language: 'typescript' | 'svelte' | 'css'}, or the " +
			"parse's own root (Root, Program or StyleSheetFile)"
	);
	// a Svelte `<script>`'s program is a `Program` that starts after its tag: refused,
	// rather than read as a TypeScript root (ECMAScript terminators, a counted BOM)
	const svelte_source = '<script>\na\rb\n</script>';
	const script_program = { type: 'Program', start: 8, end: 13, body: [], sourceType: 'module' };
	throws_type_error(
		() => reconstruct_locations(script_program, svelte_source),
		"locations option 'language' is required: cannot infer the document's language from " +
			"a 'Program' that does not span the source — pass {language: 'typescript' | " +
			"'svelte' | 'css'}, or the parse's own root (Root, Program or StyleSheetFile)"
	);
	strictEqual(
		reconstruct_locations(script_program, svelte_source, { language: 'svelte' }).loc.end.line,
		3
	);
	// named, a subtree reconstructs under that language's rule
	strictEqual(reconstruct_locations(fragment, lf_cr, { language: 'svelte' }).loc.end.line, 1);
	// a named language that is not one of the three keeps the locator's own refusal
	throws_type_error(
		() => reconstruct_locations(fragment, lf_cr, { language: 'html' as 'css' }),
		"locations option 'language' must be 'typescript', 'svelte' or 'css' (got 'html')"
	);
});

Deno.test('locations: a non-string source is refused, as read_source refuses one', () => {
	const ast = JSON.parse(span_wire('Program', 'x'));
	for (const [source, got] of [
		[42, 'number'],
		[null, 'null'],
		[undefined, 'undefined'],
		[{}, 'object']
	] as const) {
		const message = `locations source must be a string (got ${got})`;
		const bad = source as unknown as string;
		throws_type_error(() => create_locator(bad, { language: 'typescript' }), message);
		throws_type_error(() => reconstruct_locations(ast, bad), message);
	}
});

Deno.test("create_locator: position_at answers one offset in the wire's coordinates", () => {
	const ts = create_locator('ab\r\ncd', { language: 'typescript' });
	deepStrictEqual(ts.position_at(0), { line: 1, column: 0 });
	deepStrictEqual(ts.position_at(4), { line: 2, column: 0 });
	// the end of the text is a position
	deepStrictEqual(ts.position_at(6), { line: 2, column: 2 });
	// a Svelte or CSS document indexes the text without its BOM, so its length is one less
	const css = create_locator('\uFEFFa\nb', { language: 'css' });
	deepStrictEqual(css.position_at(2), { line: 2, column: 0 });
	deepStrictEqual(css.position_at(3), { line: 2, column: 1 });
	throws_range_error(() => css.position_at(4), /from 0 to 3.*\(got 4\)/);
	// a number that is not a position of the text is out of range
	for (const [offset, got] of [
		[-1, '-1'],
		[1.5, '1.5'],
		[Number.NaN, 'NaN'],
		[Infinity, 'Infinity'],
		[7, '7']
	] as const) {
		throws_range_error(
			() => ts.position_at(offset),
			new RegExp(`^position_at: offset must be an integer from 0 to 6, .*\\(got ${got}\\)$`)
		);
	}
	// a value that is not a number is the wrong type, as every argument refusal is
	for (const [offset, got] of [
		['1', "'1'"],
		[undefined, 'none'],
		[null, 'null'],
		[1n, 'bigint'],
		[{ valueOf: () => 1 }, 'object']
	] as const) {
		throws_type_error(
			() => ts.position_at(offset as unknown as number),
			`position_at: offset must be a number (got ${got})`
		);
	}
});

Deno.test(
	"a locator's loc_of: null without a span, a RangeError for one the text does not hold",
	() => {
		const source = 'a\nbc';
		const locator = create_locator(source, { language: 'svelte' });
		deepStrictEqual(locator.loc_of({ start: 2, end: 4 }), {
			start: { line: 2, column: 0 },
			end: { line: 2, column: 2 }
		});
		for (const node of [null, undefined, {}, { start: 0 }, { start: '0', end: 1 }]) {
			strictEqual(locator.loc_of(node as never), null);
		}
		for (const [start, end] of [
			[0, 5],
			[5, 5],
			[3, 2],
			[-1, 2],
			[0.5, 2]
		]) {
			const pattern = new RegExp(`^loc_of: node span ${start}\\.\\.${end} is not a range`);
			throws_range_error(() => locator.loc_of({ start, end }), pattern);
		}
		// the bound is the INDEXED text's: a Svelte BOM is not in the wire's coordinates
		const bom_locator = create_locator('\uFEFFa\nbc', { language: 'svelte' });
		deepStrictEqual(bom_locator.loc_of({ start: 2, end: 4 }), {
			start: { line: 2, column: 0 },
			end: { line: 2, column: 2 }
		});
		throws_range_error(() => bom_locator.loc_of({ start: 2, end: 5 }), /^loc_of: node span 2\.\.5/);
		// a `-0` offset is offset 0, not a `column: -0`
		deepStrictEqual(locator.position_at(-0), { line: 1, column: 0 });
		deepStrictEqual(locator.loc_of({ start: -0, end: -0 }), {
			start: { line: 1, column: 0 },
			end: { line: 1, column: 0 }
		});
	}
);

Deno.test('locations: the options bag is read as the facade reads one', () => {
	const ast = { type: 'Program', start: 0, end: 1 };
	for (const bag of ['typescript', 1, ['typescript']]) {
		throws_type_error(
			() => create_locator('a', bag as never),
			'locations options must be an object'
		);
		throws_type_error(
			() => reconstruct_locations(ast, 'a', bag as never),
			'locations options must be an object'
		);
	}
	throws_type_error(
		() => create_locator('a', { langauge: 'css' } as never),
		"unknown locations option 'langauge' (expected 'language')"
	);
	const getter_error = new Error('getter exploded');
	throws(
		() =>
			create_locator('a', {
				get language(): never {
					throw getter_error;
				}
			}),
		(
			e: unknown
		) =>
			e instanceof TypeError &&
			e.message === "failed to read locations option 'language'" &&
			e.cause === getter_error
	);
	// a set `language` is never inferred around, `null` included
	throws_type_error(
		() => reconstruct_locations(ast, 'a', { language: null } as never),
		"locations option 'language' must be 'typescript', 'svelte' or 'css' (got null)"
	);
	// `create_locator` takes no inference: the language is required, the value described
	for (const [bag, got] of [
		[undefined, 'none'],
		[{}, 'none'],
		[{ language: 'js' }, "'js'"],
		[{ language: 1 }, '1']
	] as const) {
		throws_type_error(
			() => create_locator('a', bag as never),
			`locations option 'language' must be 'typescript', 'svelte' or 'css' (got ${got})`
		);
	}
	// unset — an absent bag, `null`, or the key set to `undefined` — infers
	for (const bag of [undefined, null, {}, { language: undefined }]) {
		strictEqual(reconstruct_locations({ ...ast }, 'a', bag as never).loc.end.column, 1);
	}
	// the bag is read by its own enumerable keys alone, as `read_options` reads one: an
	// inherited `language` is not a setting
	const inherited = Object.create({ language: 'css' });
	throws_type_error(
		() => create_locator('a', inherited),
		"locations option 'language' must be 'typescript', 'svelte' or 'css' (got none)"
	);
	// so the language is inferred from the `Program` root (TypeScript's rule: the lone CR
	// breaks a line), not read off the prototype (CSS's: it does not)
	strictEqual(
		reconstruct_locations({ type: 'Program', start: 0, end: 3 }, 'a\rb', inherited).loc.end.line,
		2
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

Deno.test('parse_*_json: the wire string untouched, and `locations` refused with a pointer', () => {
	const { engine, calls } = fake_engine(true);
	const api = create_parse_api(engine);
	strictEqual(api.parse_css_json('a{}'), span_wire('StyleSheetFile', 'a{}'));
	// whatever the value, `undefined` included — the key itself is the mistake
	for (const [language, value] of [
		['typescript', false],
		['css', true],
		['svelte', undefined]
	] as const) {
		throws_type_error(
			() => api[`parse_${language}_json`]('x', { locations: value }),
			`parse option 'locations' is not supported by parse_${language}_json — the JSON ` +
				`string is the span-only wire; use parse_${language}(source, {locations: true})`
		);
	}
	strictEqual(calls.length, 1, 'a refused bag never reaches the engine');
	throws_type_error(
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
	throws_type_error(
		() => api.format_typescript('x', { locations: false }),
		"unknown format option 'locations' (expected 'sourceType')"
	);
	throws_type_error(
		() => api.format_svelte('x', { locations: false }),
		"unknown format option 'locations' (this export takes no options)"
	);
	throws_type_error(
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

Deno.test('every function the facade builds is named for its export', () => {
	const { engine } = fake_engine(true);
	const api = { ...create_parse_api(engine), ...create_format_api(engine.format) };
	for (const [name, f] of Object.entries(api)) strictEqual(f.name, name);
});

Deno.test('a source that is not well-formed UTF-16 is refused, naming its offset', () => {
	const { engine, calls } = fake_engine(true);
	const api: Record<string, (source: string) => unknown> = {
		...create_parse_api(engine),
		...create_format_api(engine.format)
	};
	for (const [source, offset] of [
		['\uD800', 0],
		['"\uD800"', 1],
		['a\uDC00', 1],
		// a high surrogate whose follower is not a low one, and a pair in reverse order
		['\uD83D😀', 0],
		['😀\uDE00\uD83D', 2],
		['ok\uD83D', 2]
	] as const) {
		for (const [name, f] of Object.entries(api)) {
			const noun = name.startsWith('format_') ? 'format' : 'parse';
			throws_type_error(
				() => f(source),
				`${noun} source must be well-formed UTF-16 (a lone surrogate at offset ${offset})`
			);
		}
	}
	strictEqual(calls.length, 0, 'a refused source never reaches the engine');
	// a pair is well-formed, astral characters included
	strictEqual(api.format_typescript!('"😀"'), 'formatted:"😀"');
	// the same answers on a runtime without `String.prototype.isWellFormed` (an older
	// browser), where the scan alone decides
	const descriptor = Object.getOwnPropertyDescriptor(String.prototype, 'isWellFormed');
	delete (String.prototype as { isWellFormed?: unknown }).isWellFormed;
	try {
		strictEqual(api.format_typescript!('"😀"'), 'formatted:"😀"');
		throws_type_error(
			() => api.parse_css!('a\uD800'),
			'parse source must be well-formed UTF-16 (a lone surrogate at offset 1)'
		);
	} finally {
		if (descriptor) Object.defineProperty(String.prototype, 'isWellFormed', descriptor);
	}
});

/**
 * The whole published surface over an engine whose every call throws `make_thrown()`
 * (a fresh value per call), with or without the engine-side object parse.
 */
function throwing_api(make_thrown: () => unknown, with_object_parse: boolean) {
	const calls: Array<string> = [];
	const family = (op: string) =>
		Object.fromEntries(
			['svelte', 'typescript', 'css'].map((language) => [
				language,
				() => {
					calls.push(`${op}_${language}`);
					throw make_thrown();
				}
			])
		);
	const engine = {
		parse_json: family('parse_json'),
		...(with_object_parse ? { parse: family('parse') } : {})
	};
	const api: Record<string, (source: any, options?: unknown) => unknown> = {
		...create_parse_api(engine),
		...create_format_api(family('format'))
	};
	return { api, calls };
}

/** What `f` throws. */
const caught = (f: () => unknown): unknown => {
	try {
		f();
	} catch (error) {
		return error;
	}
	throw new Error('expected a throw');
};

const PUBLISHED = [
	'parse_svelte',
	'parse_svelte_json',
	'parse_typescript',
	'parse_typescript_json',
	'parse_css',
	'parse_css_json',
	'format_svelte',
	'format_typescript',
	'format_css'
];

/**
 * An `Error` shaped as Bun builds every one: its own NON-enumerable numeric `line` and
 * `column` (a plain assignment over those keeps them non-enumerable), plus `extra` set as
 * ordinary enumerable properties.
 */
function bun_shaped_error(extra: Record<string, unknown>): Error {
	const error = new Error('x');
	for (const [key, value] of [
		['line', 7],
		['column', 3]
	] as const) {
		Object.defineProperty(error, key, {
			value,
			writable: true,
			enumerable: false,
			configurable: true
		});
	}
	return Object.assign(error, extra);
}

Deno.test('a pointed engine error becomes a SyntaxError with exactly start and loc', () => {
	const message = "Expected ';', found 'y'\n2:3 x y\n      ^ here";
	const pointed = () => Object.assign(new Error(message), { start: 9, line: 2, column: 2 });
	for (const with_object_parse of [true, false]) {
		const { api } = throwing_api(pointed, with_object_parse);
		for (const name of PUBLISHED) {
			const error = caught(() => api[name]('x\nx y'));
			strictEqual(error instanceof SyntaxError, true, name);
			const syntax_error = error as SyntaxError & { start: number; loc: unknown };
			strictEqual(syntax_error.message, message, name);
			deepStrictEqual(Object.keys(syntax_error), ['start', 'loc'], name);
			strictEqual(syntax_error.start, 9, name);
			deepStrictEqual(syntax_error.loc, { line: 2, column: 2 }, name);
			// minimal by design: no cause, nothing else from the engine's object
			strictEqual(syntax_error.cause, undefined, name);
		}
	}
});

Deno.test('anything else an engine throws passes through as the same object', () => {
	class RuntimeError extends Error {}
	const values: Array<() => unknown> = [
		// the size cap (or a raw engine's own refusal, reached past the facade): a plain
		// Error with no point
		() => new Error('File too large: 5 bytes (maximum: 4 bytes / 4GB)'),
		// a WASM trap, and V8's stack exhaustion
		() => new RuntimeError('unreachable'),
		() => new RangeError('Maximum call stack size exceeded'),
		// a partial point is no point
		() => Object.assign(new Error('x'), { start: 1, line: 1 }),
		() => Object.assign(new Error('x'), { start: '1', line: 1, column: 0 }),
		// Bun gives every Error its own non-enumerable numeric `line` and `column`, so an
		// error carrying a numeric `start` alone must not read as a point there
		() => bun_shaped_error({ start: 4 }),
		// a member that is not an integer is no point
		() => Object.assign(new Error('x'), { start: Number.NaN, line: 1, column: 0 }),
		() => Object.assign(new Error('x'), { start: 0, line: Infinity, column: 0 }),
		() => Object.assign(new Error('x'), { start: 0, line: 1, column: 1.5 }),
		// nor is one that is not an own enumerable property
		() =>
			Object.defineProperty(Object.assign(new Error('x'), { line: 1, column: 0 }), 'start', {
				value: 0,
				enumerable: false
			}),
		() => Object.assign(Object.create({ start: 0 }), { line: 1, column: 0 }),
		// not an object at all
		() => 'a string',
		() => undefined
	];
	for (const make of values) {
		for (const with_object_parse of [true, false]) {
			let last: unknown;
			const { api } = throwing_api(() => (last = make()), with_object_parse);
			for (const name of PUBLISHED) {
				const error = caught(() => api[name]('x'));
				strictEqual(error, last, name);
			}
		}
	}
});

Deno.test('argument refusals stay TypeErrors and never reach the engine', () => {
	const { api, calls } = throwing_api(
		() => Object.assign(new Error('bad'), { start: 0, line: 1, column: 0 }),
		true
	);
	for (const name of PUBLISHED) {
		strictEqual(caught(() => api[name](42)) instanceof TypeError, true, name);
		strictEqual(caught(() => api[name]('x', { nope: true })) instanceof TypeError, true, name);
	}
	strictEqual(calls.length, 0);
});

Deno.test("the facade's own JSON.parse failing is an internal Error, not a SyntaxError", () => {
	const api = create_parse_api({
		parse_json: { typescript: () => '{"type":"Program",' }
	});
	const error = caught(() => api.parse_typescript('x'));
	strictEqual(error instanceof Error, true);
	strictEqual(error instanceof SyntaxError, false);
	strictEqual((error as Error).message, 'internal error: AST serialized to invalid JSON');
	strictEqual((error as Error).cause instanceof SyntaxError, true);
	// the string export hands the wire back untouched — it parses nothing
	strictEqual(api.parse_typescript_json('x'), '{"type":"Program",');
});

Deno.test(
	'an options read that throws reaches the caller as itself, before any engine call',
	() => {
		// the bag is read ahead of `call_engine`, so even a throw shaped exactly like an
		// engine's pointed failure is never converted, and no engine runs
		const pointed = Object.assign(new Error('from the bag'), { start: 0, line: 1, column: 0 });
		const bag = new Proxy(
			{},
			{
				ownKeys() {
					throw pointed;
				}
			}
		);
		const { api, calls } = throwing_api(() => new Error('engine'), true);
		for (const name of PUBLISHED) {
			strictEqual(
				caught(() => api[name]('x', bag)),
				pointed,
				name
			);
		}
		strictEqual(calls.length, 0);
	}
);
