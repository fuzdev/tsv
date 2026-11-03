/**
 * The import-records contract every imports-capable tsv npm package publishes —
 * `@fuzdev/tsv-wasm` and the native `@fuzdev/tsv` — as one shared table, registered by both
 * package suites (`scripts/test_npm.ts` over the `all` variant, `scripts/test_napi_npm.ts`
 * over the native loader) and, under Bun, by `scripts/test_bun.ts`, so the two engines are
 * held to one table.
 *
 * `parse_imports_<lang>(source, options?)` returns a result object and never throws for what
 * it finds in the source: `{ok: true, hasModuleSyntax, imports}`, or `{ok: false, error}` for a
 * source that does not parse (`error.kind === 'parse'`, the error `parse_<lang>` returns at the
 * goal the error is attributed to). Only argument misuse throws, as a `TypeError` in the
 * facade's own words.
 *
 * The records are graded against every fixture under `tests/fixtures_imports/` — the
 * `expected.json` the Rust gate (`tests/imports_fixtures_tests.rs`) pins, so the package
 * surface returns exactly the records the language crates define, offsets in UTF-16 units.
 * Invisible characters in the rows below are spelled as escapes, never written literally.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

/** The fixture tree the records are graded against. */
const FIXTURES_DIR = fileURLToPath(new URL('../tests/fixtures_imports', import.meta.url));

/** One fixture: its input, the language its extension names, and its pinned records. */
export interface ImportsFixture {
	name: string;
	language: 'typescript' | 'svelte';
	source: string;
	expected: unknown;
}

/** Every fixture under `tests/fixtures_imports/`, in path order. */
export function imports_fixtures(): Array<ImportsFixture> {
	const fixtures: Array<ImportsFixture> = [];
	const walk = (dir: string): void => {
		const entries = readdirSync(dir).sort();
		const input = entries.find((name) => name === 'input.ts' || name === 'input.svelte');
		if (input !== undefined) {
			fixtures.push({
				name: dir.slice(FIXTURES_DIR.length + 1),
				language: input === 'input.ts' ? 'typescript' : 'svelte',
				source: readFileSync(join(dir, input), 'utf8'),
				expected: JSON.parse(readFileSync(join(dir, 'expected.json'), 'utf8'))
			});
			return;
		}
		for (const name of entries) {
			const path = join(dir, name);
			if (statSync(path).isDirectory()) walk(path);
		}
	};
	walk(FIXTURES_DIR);
	return fixtures;
}

/** A parse-failure row: the source, its language, the bag, and the goal whose error it
 * reports (default `'module'`), as `parse_<lang>(source, {sourceType})` reports it. */
export interface ImportsParseErrorRow {
	name: string;
	language: 'typescript' | 'svelte';
	source: string;
	options?: { sourceType: 'script' | 'module' };
	reported?: 'script' | 'module';
}

export const IMPORTS_PARSE_ERROR_ROWS: ReadonlyArray<ImportsParseErrorRow> = [
	{ name: 'a missing binding', language: 'typescript', source: 'let = ;\n' },
	{
		name: 'after CRLF and an astral character',
		language: 'typescript',
		source: "import '\u{1d49c}';\r\nconst = ;\r\n"
	},
	{
		name: "a sloppy script's typo, past the with the retry admits",
		language: 'typescript',
		source: 'with (o) {}\nlet = ;\n',
		reported: 'script'
	},
	{
		name: "a named 'module' goal takes no retry",
		language: 'typescript',
		source: 'with (a) {}\n',
		options: { sourceType: 'module' }
	},
	{
		name: "a named 'script' goal takes no module attempt",
		language: 'typescript',
		source: "import a from 'a';\n",
		options: { sourceType: 'script' },
		reported: 'script'
	},
	{ name: 'an unclosed element', language: 'svelte', source: '<div' },
	{ name: 'a script that does not parse', language: 'svelte', source: '<script>let = ;</script>' }
];

/**
 * Register the table over one package entry, given as its module namespace or as a function
 * returning it for an entry only bound once a test ahead of these has run.
 *
 * @param label - names the entry in the suite's title
 * @param api_or_getter - the entry's exports, or a function returning them
 */
export function register_imports_api_suite(
	label: string,
	api_or_getter: Record<string, any> | (() => Record<string, any>)
): void {
	const get_api = () => (typeof api_or_getter === 'function' ? api_or_getter() : api_or_getter);
	const call = (language: string, source: unknown, options?: unknown) =>
		get_api()[`parse_imports_${language}`](source, options);

	describe(`parse_imports: ${label}`, () => {
		it('is exported for TypeScript and Svelte', () => {
			assert.equal(typeof get_api().parse_imports_typescript, 'function');
			assert.equal(typeof get_api().parse_imports_svelte, 'function');
			assert.equal(get_api().parse_imports_css, undefined);
		});

		it('returns every fixture’s pinned records', () => {
			const fixtures = imports_fixtures();
			assert.ok(fixtures.length > 0, `no fixtures under ${FIXTURES_DIR}`);
			for (const { name, language, source, expected } of fixtures) {
				const result = call(language, source);
				assert.deepStrictEqual(Object.keys(result), ['ok', 'hasModuleSyntax', 'imports'], name);
				assert.deepStrictEqual(result, { ok: true, ...(expected as object) }, name);
			}
		});

		it('every record keeps its keys in one order', () => {
			const { imports } = call(
				'typescript',
				"import a from 'a' with { type: 'json' };\nexport * from 'b';\nimport(c);\n"
			);
			assert.equal(imports.length, 3);
			for (const record of imports) {
				assert.deepStrictEqual(Object.keys(record), [
					'kind',
					'start',
					'end',
					'specifier',
					'typeOnly',
					'phase',
					'attributes'
				]);
			}
			assert.deepStrictEqual(Object.keys(imports[0].specifier), ['value', 'start', 'end']);
			assert.deepStrictEqual(Object.keys(imports[0].attributes), ['start', 'end']);
			assert.equal(imports[2].specifier, null, 'a computed specifier');
		});

		it('indexes the text parse_<lang> indexes past a leading BOM', () => {
			// TypeScript counts the BOM, as acorn does; Svelte elides it, as Svelte's parse does
			const api = get_api();
			const ts_source = "\uFEFFimport a from 'x';\n";
			const ts = call('typescript', ts_source);
			const ts_statement = api.parse_typescript(ts_source).ast.body[0];
			assert.equal(ts.imports[0].start, 1);
			assert.equal(ts.imports[0].start, ts_statement.start);
			assert.equal(ts.imports[0].end, ts_statement.end);
			assert.equal(ts.imports[0].specifier.start, ts_statement.source.start + 1);
			const svelte_source = "\uFEFF<script>import 'x';</script>";
			const svelte = call('svelte', svelte_source);
			const svelte_statement = api.parse_svelte(svelte_source).ast.instance.content.body[0];
			assert.equal(svelte.imports[0].start, 8);
			assert.equal(svelte.imports[0].start, svelte_statement.start);
			assert.equal(svelte.imports[0].end, svelte_statement.end);
			assert.equal(svelte.imports[0].specifier.start, svelte_statement.source.start + 1);
		});

		it('reads the goal as a strip does', () => {
			const sloppy = 'with (o) { import(`./a`); }\n';
			// unset: the module grammar, retried as a script
			const retried = call('typescript', sloppy);
			assert.equal(retried.ok, true);
			assert.equal(retried.hasModuleSyntax, false);
			assert.equal(retried.imports[0].specifier.value, './a');
			assert.deepStrictEqual(call('typescript', sloppy, { sourceType: 'script' }), retried);
			assert.equal(call('typescript', sloppy, { sourceType: 'module' }).ok, false);
			// a module-valid source is never read as a script
			const top_level_await = call('typescript', 'await import("./a");\n');
			assert.equal(top_level_await.hasModuleSyntax, true);
		});

		for (const row of IMPORTS_PARSE_ERROR_ROWS) {
			it(`reports a parse error: ${row.name}`, () => {
				const api = get_api();
				const result = call(row.language, row.source, row.options);
				assert.deepStrictEqual(Object.keys(result), ['ok', 'error']);
				assert.equal(result.ok, false);
				assert.deepStrictEqual(Object.keys(result.error), [
					'kind',
					'message',
					'frame',
					'start',
					'end',
					'loc'
				]);
				// the same error, at the same point, that parsing at the reported goal reports
				const parsed =
					row.language === 'typescript'
						? api.parse_typescript(row.source, { sourceType: row.reported ?? 'module' })
						: api.parse_svelte(row.source);
				assert.equal(parsed.ok, false);
				assert.deepStrictEqual(result.error, parsed.error);
			});
		}

		it('takes a sourceType bag', () => {
			const ok = { ok: true, hasModuleSyntax: false, imports: [] };
			for (const bag of [undefined, null, {}, { sourceType: undefined }]) {
				assert.deepStrictEqual(call('typescript', 'let x;', bag), ok);
				assert.deepStrictEqual(call('svelte', '<p>x</p>', bag), ok);
			}
			assert.deepStrictEqual(call('typescript', 'let x;', { sourceType: 'module' }), ok);
			assert.deepStrictEqual(call('typescript', 'let x;', { sourceType: 'script' }), ok);
		});

		it("throws a TypeError for argument misuse, in the facade's words", () => {
			const refusals: Array<[() => unknown, string]> = [
				[() => call('typescript', 42), 'imports source must be a string (got number)'],
				[() => call('svelte', null), 'imports source must be a string (got null)'],
				[
					() => call('typescript', '"\uD800";'),
					'imports source must be well-formed UTF-16 (a lone surrogate at offset 1)'
				],
				[() => call('typescript', 'x', 'module'), 'imports options must be an object'],
				[
					() => call('typescript', 'x', { locations: true }),
					"unknown imports option 'locations' (expected 'sourceType')"
				],
				[
					() => call('typescript', 'x', { sourceType: 'sloppy' }),
					"imports option 'sourceType' must be 'script' or 'module' (got 'sloppy')"
				],
				[
					() => call('svelte', '<p>x</p>', { sourceType: 'module' }),
					"imports option 'sourceType' is only supported for TypeScript"
				],
				[
					() => call('svelte', '<p>x</p>', { locations: false }),
					"unknown imports option 'locations' (this export takes no options)"
				]
			];
			for (const [thunk, message] of refusals) {
				assert.throws(thunk, { name: 'TypeError', message });
			}
		});
	});
}
