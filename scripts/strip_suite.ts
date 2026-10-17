/**
 * The type-strip contract every strip-capable tsv npm package publishes — `@fuzdev/tsv-wasm`
 * and the native `@fuzdev/tsv` — as shared tables, registered by both package suites
 * (`scripts/test_npm.ts` over the `all` variant, `scripts/test_napi_npm.ts` over the native
 * loader) and, under Bun, by `scripts/test_bun.ts`, so the two engines are held to one
 * table.
 *
 * Two surfaces:
 *
 * - **the API** (`register_strip_api_suite`): `strip_typescript(source, options?)` returns a
 *   result object and never throws for a strip outcome — `{ok: true, code}` for JS, or
 *   `{ok: false, error}` for a source that does not parse (`error.kind === 'parse'`, the
 *   point `parse_typescript` reports for it at the goal the error is attributed to) or one
 *   holding TypeScript that type blanking cannot erase (`error.kind === 'unsupported'`, the
 *   refused construct and its span). The parse goal reads as the format path reads it: a
 *   named `sourceType` is exact, and with none the source is parsed as a module and retried
 *   as a script only if that fails. Only argument misuse throws, as a `TypeError` in the
 *   facade's own words;
 * - **the CLI** (`register_strip_cli_suite`): `tsv strip <file>` / `--content` / `--stdin`
 *   print the stripped code to stdout verbatim, `--check` prints nothing, and the exit code
 *   is 0 stripped, 1 refused, 2 a parse error or a usage error (argh's own flag-parsing
 *   refusals exit 1, as for every command).
 *
 * The rows reach what the strip's position invariant turns on: a blanked range holding a
 * multi-unit character, CRLF and U+2028 kept verbatim, a leading BOM and a hashbang kept,
 * a statement-final assertion's `;` hazard, and type-free input returned unchanged.
 * Invisible characters are spelled as escapes, never written literally.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { create_locator } from '../crates/tsv_wasm/npm/locations.js';

/** The refused constructs, as `error.construct` names them. */
export type StripConstruct =
	| 'enum'
	| 'value_namespace'
	| 'parameter_property'
	| 'import_equals'
	| 'export_equals'
	| 'type_assertion'
	| 'assertion_grouping';

/** How a refusal's message names each construct: `cannot strip <description>`. */
export const STRIP_CONSTRUCT_DESCRIPTIONS: Readonly<Record<StripConstruct, string>> = {
	enum: 'an enum',
	value_namespace: 'a namespace with a value member',
	parameter_property: 'a parameter property',
	import_equals: "an 'import =' declaration",
	export_equals: "an 'export =' assignment",
	type_assertion: 'an angle-bracket type assertion',
	assertion_grouping: 'an assertion whose blanking would regroup its expression'
};

/** The options bag a row passes, when any: the parse goal, named. */
export interface StripRowOptions {
	sourceType: 'script' | 'module';
}

export interface StripOkRow {
	name: string;
	source: string;
	options?: StripRowOptions;
	code: string;
}

export interface StripRefusalRow {
	name: string;
	source: string;
	construct: StripConstruct;
	/** The refused span's UTF-16 offsets. */
	start: number;
	end: number;
}

export interface StripParseErrorRow {
	name: string;
	source: string;
	options?: StripRowOptions;
	/**
	 * The goal whose parse error the strip reports, as `parse_typescript(source, {sourceType})`
	 * reports it (default `'module'`). A named goal is that goal; with none named, the
	 * module's, unless the script retry reached further without dying on a module-only
	 * construct.
	 */
	reported?: 'script' | 'module';
}

export const STRIP_OK_ROWS: ReadonlyArray<StripOkRow> = [
	{ name: 'a type annotation', source: 'let x: number = 1;\n', code: 'let x         = 1;\n' },
	{ name: 'no trailing newline is added', source: 'let x: T', code: 'let x   ' },
	{
		name: 'type parameters, parameter and return types',
		source: 'function f<T>(a: T): T { return a; }\n',
		code: 'function f   (a   )    { return a; }\n'
	},
	{
		name: 'type-only imports and exports, an inline type specifier',
		source: "import type { A } from 'a';\nimport { type B, c } from 'b';\nexport type { A };\n",
		code: "                           \nimport {         c } from 'b';\n                  \n"
	},
	{
		name: 'a statement-final assertion before a line opening with (',
		source: 'let x = y as T\n(z)\n',
		code: 'let x = y;    \n(z)\n'
	},
	{
		name: 'CRLF and U+2028 are kept verbatim',
		source: 'const a = (b as string).length;\r\ninterface I { x: 1 }\u2028export {};\n',
		code: 'const a = (b          ).length;\r\n                    \u2028export {};\n'
	},
	{
		name: 'an astral character before a blanked range',
		source: "const s = '\u{1d49c}' as string;\n",
		code: "const s = '\u{1d49c}'          ;\n"
	},
	{
		name: 'a leading BOM is kept',
		source: '\uFEFFlet x: T = 1;\n',
		code: '\uFEFFlet x    = 1;\n'
	},
	{
		name: 'a hashbang is kept',
		source: '#!/usr/bin/env node\nlet x: T = 1;\n',
		code: '#!/usr/bin/env node\nlet x    = 1;\n'
	},
	{
		name: 'type-free input comes back unchanged',
		source: 'const x = 1;\n',
		code: 'const x = 1;\n'
	},
	// the unset goal: a module, retried as a script only if that fails
	{
		name: 'a sloppy script with no goal named: a with statement',
		source: 'with (o as any) { x; }\n',
		code: 'with (o       ) { x; }\n'
	},
	{
		name: 'a sloppy script with no goal named: a legacy octal literal',
		source: 'fs.chmodSync(p as string, 0755);\n',
		code: 'fs.chmodSync(p          , 0755);\n'
	},
	{
		name: 'a sloppy script with no goal named: await as a name',
		source: 'var await: number = 1;\n',
		code: 'var await         = 1;\n'
	},
	{
		name: 'a module-valid source is never read as a script: top-level await',
		source: 'await f() as T;\n',
		code: 'await f()     ;\n'
	},
	{
		name: "a named 'script' goal",
		source: 'var await: number = 1;\n',
		options: { sourceType: 'script' },
		code: 'var await         = 1;\n'
	},
	{
		name: "a named 'module' goal",
		source: 'let x: T = 1;\nexport { x };\n',
		options: { sourceType: 'module' },
		code: 'let x    = 1;\nexport { x };\n'
	}
];

export const STRIP_REFUSAL_ROWS: ReadonlyArray<StripRefusalRow> = [
	{ name: 'an enum', source: 'enum E { A }\n', construct: 'enum', start: 0, end: 12 },
	{
		name: 'a namespace with a value member',
		source: 'namespace N { export const a = 1; }\n',
		construct: 'value_namespace',
		start: 0,
		end: 35
	},
	{
		name: 'a parameter property, spanning its first modifier',
		source: 'class A { constructor(public x: number) {} }\n',
		construct: 'parameter_property',
		start: 22,
		end: 28
	},
	{
		name: 'import =',
		source: "import x = require('y');\n",
		construct: 'import_equals',
		start: 0,
		end: 24
	},
	{ name: 'export =', source: 'export = x;\n', construct: 'export_equals', start: 0, end: 11 },
	{
		name: 'an angle-bracket assertion on line 2',
		source: 'const a = 1;\nconst b = <T>c;\n',
		construct: 'type_assertion',
		start: 23,
		end: 27
	},
	{
		name: 'an assertion whose blanking would regroup',
		source: 'let v = a + b as T / c;\n',
		construct: 'assertion_grouping',
		start: 8,
		end: 18
	},
	{
		// UTF-16 offsets, not bytes: the astral character ahead of the enum is two units
		name: 'an enum after an astral character',
		source: 'let \u{1d49c}: T = 1; enum E {}\n',
		construct: 'enum',
		start: 15,
		end: 24
	}
];

export const STRIP_PARSE_ERROR_ROWS: ReadonlyArray<StripParseErrorRow> = [
	{ name: 'a missing binding', source: 'let = ;\n' },
	{ name: 'the end of the source', source: 'function f() {\n' },
	{ name: 'after CRLF and an astral character', source: "const a = '\u{1d49c}';\r\nconst = ;\r\n" },
	// both grammars reject it: the error is the one the format path's fallback reports
	{
		name: "a sloppy script's typo, past the with the retry admits",
		source: 'with (o) {}\nlet = ;\n',
		reported: 'script'
	},
	{
		name: 'a module whose error sits ahead of its export',
		source: 'let x: number = 0755;\nexport {};\n',
		reported: 'module'
	},
	{
		name: "a named 'module' goal takes no retry",
		source: 'with (a) {}\n',
		options: { sourceType: 'module' },
		reported: 'module'
	},
	{
		name: "a named 'script' goal takes no module attempt",
		source: "import type { A } from 'a';\n",
		options: { sourceType: 'script' },
		reported: 'script'
	}
];

/** The thrown value of `f`, failing the test when nothing is thrown. */
function thrown_by(f: () => unknown): any {
	try {
		f();
	} catch (error) {
		return error;
	}
	assert.fail('expected a throw');
}

/** The first line of `message`, and the `line:col` header of its second, as a parse error's
 * message prints them. */
function header_of(message: string): string {
	return message.split('\n')[1]?.split(' ')[0] ?? '';
}

/**
 * Register the API rows over one package entry, given as its module namespace or as a
 * function returning it for an entry only bound once a test ahead of these has run.
 *
 * @param label - names the entry in the suite's title
 * @param api_or_getter - the entry's exports, or a function returning them
 */
export function register_strip_api_suite(
	label: string,
	api_or_getter: Record<string, any> | (() => Record<string, any>)
): void {
	const get_api = () => (typeof api_or_getter === 'function' ? api_or_getter() : api_or_getter);

	describe(`strip_typescript: ${label}`, () => {
		it('is exported', () => {
			assert.equal(typeof get_api().strip_typescript, 'function');
		});

		for (const row of STRIP_OK_ROWS) {
			it(`strips: ${row.name}`, () => {
				const result = get_api().strip_typescript(row.source, row.options);
				assert.deepStrictEqual(Object.keys(result), ['ok', 'code']);
				assert.deepStrictEqual(result, { ok: true, code: row.code });
				// the position invariant, which every row is an instance of
				assert.equal(result.code.length, row.source.length, 'UTF-16 length is kept');
				// idempotence: the output strips to itself
				assert.deepStrictEqual(get_api().strip_typescript(result.code, row.options), result);
			});
		}

		for (const row of STRIP_REFUSAL_ROWS) {
			it(`refuses: ${row.name}`, () => {
				const result = get_api().strip_typescript(row.source);
				assert.deepStrictEqual(Object.keys(result), ['ok', 'error']);
				assert.equal(result.ok, false);
				const { error } = result;
				assert.deepStrictEqual(Object.keys(error), [
					'kind',
					'construct',
					'message',
					'start',
					'end',
					'loc'
				]);
				const locator = create_locator(row.source, { language: 'typescript' });
				const start = locator.position_at(row.start);
				const end = locator.position_at(row.end);
				assert.deepStrictEqual(
					{ ...error, message: undefined },
					{
						kind: 'unsupported',
						construct: row.construct,
						message: undefined,
						start: row.start,
						end: row.end,
						loc: { start, end }
					}
				);
				assert.ok(!(error instanceof Error), 'a plain object, not a thrown-shape Error');
				const [first] = error.message.split('\n');
				assert.equal(first, `cannot strip ${STRIP_CONSTRUCT_DESCRIPTIONS[row.construct]}`);
				// the header names the span's start as `line:column + 1`, as a parse error's does
				assert.equal(header_of(error.message), `${start.line}:${start.column + 1}`);
			});
		}

		for (const row of STRIP_PARSE_ERROR_ROWS) {
			it(`reports a parse error: ${row.name}`, () => {
				const api = get_api();
				const result = api.strip_typescript(row.source, row.options);
				assert.deepStrictEqual(Object.keys(result), ['ok', 'error']);
				assert.equal(result.ok, false);
				assert.deepStrictEqual(Object.keys(result.error), ['kind', 'message', 'start', 'loc']);
				// the same error, at the same point, that parsing at the reported goal reports
				const thrown = thrown_by(() =>
					api.parse_typescript(row.source, { sourceType: row.reported ?? 'module' })
				);
				assert.ok(thrown instanceof SyntaxError, String(thrown));
				const parsed = thrown as SyntaxError & { start: number; loc: unknown };
				assert.deepStrictEqual(result.error, {
					kind: 'parse',
					message: parsed.message,
					start: parsed.start,
					loc: parsed.loc
				});
			});
		}

		it('never throws for a strip outcome', () => {
			const api = get_api();
			for (const { source } of STRIP_REFUSAL_ROWS) {
				assert.doesNotThrow(() => api.strip_typescript(source), JSON.stringify(source));
			}
			for (const { source, options } of STRIP_PARSE_ERROR_ROWS) {
				assert.doesNotThrow(() => api.strip_typescript(source, options), JSON.stringify(source));
			}
		});

		it('takes a sourceType bag', () => {
			const api = get_api();
			const ok = { ok: true, code: 'let x   ;' };
			assert.deepStrictEqual(api.strip_typescript('let x: T;', undefined), ok);
			assert.deepStrictEqual(api.strip_typescript('let x: T;', null), ok);
			assert.deepStrictEqual(api.strip_typescript('let x: T;', {}), ok);
			assert.deepStrictEqual(api.strip_typescript('let x: T;', { sourceType: 'module' }), ok);
			assert.deepStrictEqual(api.strip_typescript('let x: T;', { sourceType: 'script' }), ok);
			// the forwarding idiom: a supported key set to `undefined` is its default
			assert.deepStrictEqual(api.strip_typescript('let x: T;', { sourceType: undefined }), ok);
		});

		it("throws a TypeError for argument misuse, in the facade's words", () => {
			const api = get_api();
			const refusals: Array<[() => unknown, string]> = [
				[() => api.strip_typescript(42), 'strip source must be a string (got number)'],
				[() => api.strip_typescript(null), 'strip source must be a string (got null)'],
				[
					() => api.strip_typescript('"\uD800";'),
					'strip source must be well-formed UTF-16 (a lone surrogate at offset 1)'
				],
				[() => api.strip_typescript('x', 'module'), 'strip options must be an object'],
				[() => api.strip_typescript('x', ['module']), 'strip options must be an object'],
				[
					() => api.strip_typescript('x', { locations: true }),
					"unknown strip option 'locations' (expected 'sourceType')"
				],
				[
					() => api.strip_typescript('x', { sourceType: 'sloppy' }),
					"strip option 'sourceType' must be 'script' or 'module' (got 'sloppy')"
				]
			];
			for (const [call, message] of refusals) {
				assert.throws(call, { name: 'TypeError', message });
			}
		});
	});
}

/** What a CLI runner returns — `spawnSync`'s result with utf-8 encoding. */
export interface CliResult {
	status: number | null;
	stdout: string;
	stderr: string;
}

/** Run one `tsv` bin with `args` (and `stdin`, when given), in `cwd` when given. */
export type CliRunner = (args: Array<string>, stdin?: string, cwd?: string) => CliResult;

/** One CLI row: the argv (with `FILE` standing for a temp file holding `file`, when given),
 * the stdin, and the outcome. `stderr_starts` is a prefix; `stdout` is exact. */
export interface StripCliRow {
	name: string;
	args: Array<string>;
	stdin?: string;
	/** Written to a temp file named `file_name` (default `a.ts`); `FILE` in `args` is its path. */
	file?: string;
	file_name?: string;
	status: number;
	stdout?: string;
	stderr?: string;
	stderr_starts?: string;
	stderr_includes?: string;
	/** The message ends in the OS's own words for a read failure (Rust's `io::Error`
	 * natively, Node's `ENOENT` text in `cli.js`), so the two bins agree on its prefix
	 * alone. */
	os_words?: boolean;
}

const ENUM = 'enum E { A }\n';

export const STRIP_CLI_ROWS: ReadonlyArray<StripCliRow> = [
	{
		name: '--content prints the stripped code verbatim',
		args: ['strip', '--content', 'let x: number = 1;\n'],
		status: 0,
		stdout: 'let x         = 1;\n',
		stderr: ''
	},
	{
		name: '--content adds no trailing newline',
		args: ['strip', '--content', 'let x: T'],
		status: 0,
		stdout: 'let x   ',
		stderr: ''
	},
	{
		name: '--stdin reads stdin',
		args: ['strip', '--stdin'],
		stdin: 'function f<T>(a: T): T { return a; }\n',
		status: 0,
		stdout: 'function f   (a   )    { return a; }\n',
		stderr: ''
	},
	{
		name: 'a file path prints to stdout',
		args: ['strip', 'FILE'],
		file: 'const a = (b as string).length;\r\ninterface I { x: 1 }\u2028export {};\n',
		status: 0,
		stdout: 'const a = (b          ).length;\r\n                    \u2028export {};\n',
		stderr: ''
	},
	{
		name: 'a .svelte.ts file is TypeScript',
		args: ['strip', 'FILE'],
		file: 'let count: number = $state(0);\n',
		file_name: 'counter.svelte.ts',
		status: 0,
		stdout: 'let count         = $state(0);\n',
		stderr: ''
	},
	{
		name: 'a leading BOM survives a file read',
		args: ['strip', 'FILE'],
		file: '\uFEFFlet x: T = 1;\n',
		status: 0,
		stdout: '\uFEFFlet x    = 1;\n',
		stderr: ''
	},
	{
		name: 'a refusal exits 1 with the construct and its position',
		args: ['strip', '--content', ENUM],
		status: 1,
		stdout: '',
		stderr_starts: 'Error: cannot strip an enum\n1:1 enum E { A }\n'
	},
	{
		name: 'a refusal on a file names the position in that file',
		args: ['strip', 'FILE'],
		file: 'const a = 1;\nconst b = <T>c;\n',
		status: 1,
		stdout: '',
		stderr_starts: 'Error: cannot strip an angle-bracket type assertion\n2:11 const b = <T>c;\n'
	},
	{
		name: 'a parse error exits 2',
		args: ['strip', '--content', 'let = ;\n'],
		status: 2,
		stdout: '',
		stderr_starts: "Parse error: Expected expression, found ';'\n1:7 let = ;\n"
	},
	{
		name: '--check on strippable input prints nothing and exits 0',
		args: ['strip', '--check', '--content', 'let x: T = 1;\n'],
		status: 0,
		stdout: '',
		stderr: ''
	},
	{
		name: '--check on type-free input exits 0',
		args: ['strip', '--check', '--content', 'const x = 1;\n'],
		status: 0,
		stdout: '',
		stderr: ''
	},
	{
		name: '--check on a refusal exits 1',
		args: ['strip', '--check', '--content', ENUM],
		status: 1,
		stdout: '',
		stderr_starts: 'Error: cannot strip an enum\n'
	},
	{
		name: '--check on a parse error exits 2',
		args: ['strip', '--check', 'FILE'],
		file: 'let = ;\n',
		status: 2,
		stdout: '',
		stderr_starts: 'Parse error: '
	},
	{
		name: '--content with no --source-type retries a sloppy script',
		args: ['strip', '--content', 'fs.chmodSync(p as string, 0755);\n'],
		status: 0,
		stdout: 'fs.chmodSync(p          , 0755);\n',
		stderr: ''
	},
	{
		name: "a failed retry reports the sloppy script's typo",
		args: ['strip', '--content', 'with (o) {}\nlet = ;\n'],
		status: 2,
		stdout: '',
		stderr_starts: "Parse error: Expected expression, found ';'\n2:7 let = ;\n"
	},
	{
		name: '--source-type script reads await as a name',
		args: ['strip', '--source-type', 'script', '--content', 'var await: number = 1;\n'],
		status: 0,
		stdout: 'var await         = 1;\n',
		stderr: ''
	},
	{
		name: '--source-type module takes no retry',
		args: ['strip', '--source-type', 'module', '--content', 'with (a) {}\n'],
		status: 2,
		stdout: '',
		stderr_starts: "Parse error: The 'with' statement is not allowed in strict mode\n"
	},
	{
		name: '--source-type script takes no module attempt',
		args: ['strip', '--stdin', '--source-type', 'script'],
		stdin: "import type { A } from 'a';\n",
		status: 2,
		stdout: '',
		stderr_starts: "Parse error: 'import' is only allowed in a module\n"
	},
	{
		name: 'an invalid --source-type is a usage error',
		args: ['strip', '--source-type', 'sloppy', '--content', 'let x: T;'],
		status: 2,
		stdout: '',
		stderr: "Error: invalid --source-type 'sloppy' (expected 'script' or 'module')\n"
	},
	{
		name: '--source-type beside a file path is refused',
		args: ['strip', '--source-type', 'script', 'FILE'],
		file: 'let y: T;\n',
		status: 2,
		stdout: '',
		stderr:
			"Error: --source-type applies to --content/--stdin; a file path's goal is its extension's (.mjs/.mts a module, any other JS/TS extension the module grammar retried as a script)\n"
	},
	{
		name: 'a .mts file is a module by name: no script retry',
		args: ['strip', 'FILE'],
		file: 'var await: number = 1;\n',
		file_name: 'a.mts',
		status: 2,
		stdout: '',
		stderr_starts: 'Parse error: '
	},
	{
		name: 'an .MTS file is a module by name too: the extension reads without regard to ASCII case',
		args: ['strip', 'FILE'],
		file: 'var await: number = 1;\n',
		file_name: 'A.MTS',
		status: 2,
		stdout: '',
		stderr_starts: 'Parse error: '
	},
	{
		name: 'a .cts file keeps the script retry',
		args: ['strip', 'FILE'],
		file: 'var await: number = 1;\n',
		file_name: 'a.cts',
		status: 0,
		stdout: 'var await         = 1;\n',
		stderr: ''
	},
	{
		name: 'no input is a usage error',
		args: ['strip'],
		status: 2,
		stdout: '',
		stderr: 'Error: No input provided. Use a file path, --content, or --stdin\n'
	},
	{
		name: '--content beside a file path is refused',
		args: ['strip', '--content', 'let x: T;', 'FILE'],
		file: 'let y: T;\n',
		status: 2,
		stdout: '',
		stderr: 'Error: --content/--stdin cannot be combined with a file path\n'
	},
	{
		name: 'a .svelte file is out of scope',
		args: ['strip', 'FILE'],
		file: '<script lang="ts">let x: T;</script>\n',
		file_name: 'a.svelte',
		status: 2,
		stdout: '',
		stderr_includes: 'unsupported file extension (tsv strip handles .ts, .mts, .cts)'
	},
	{
		name: 'a .js file is refused, not read as TypeScript',
		args: ['strip', 'FILE'],
		file: 'f(a < b, c > (d));\n',
		file_name: 'a.js',
		status: 2,
		stdout: '',
		stderr_includes:
			'a.js: a JavaScript file has no types to strip (tsv strip handles .ts, .mts, .cts)\n'
	},
	{
		name: 'a .mjs file is refused, not read as TypeScript',
		args: ['strip', 'FILE'],
		file: 'f(a < b, c > (d));\n',
		file_name: 'a.mjs',
		status: 2,
		stdout: '',
		stderr_includes:
			'a.mjs: a JavaScript file has no types to strip (tsv strip handles .ts, .mts, .cts)\n'
	},
	{
		name: 'a .cjs file is refused, not read as TypeScript',
		args: ['strip', 'FILE'],
		file: 'f(a < b, c > (d));\n',
		file_name: 'a.cjs',
		status: 2,
		stdout: '',
		stderr_includes:
			'a.cjs: a JavaScript file has no types to strip (tsv strip handles .ts, .mts, .cts)\n'
	},
	{
		name: 'a JavaScript extension is read without regard to ASCII case',
		args: ['strip', 'FILE'],
		file: 'f(a < b, c > (d));\n',
		file_name: 'A.MJS',
		status: 2,
		stdout: '',
		stderr_includes:
			'A.MJS: a JavaScript file has no types to strip (tsv strip handles .ts, .mts, .cts)\n'
	},
	{
		name: 'a .css file is out of scope',
		args: ['strip', 'FILE'],
		file: 'a {}\n',
		file_name: 'a.css',
		status: 2,
		stdout: '',
		stderr_includes: 'unsupported file extension (tsv strip handles'
	},
	{
		name: 'a missing file is an error',
		args: ['strip', 'missing_file_for_strip.ts'],
		os_words: true,
		status: 2,
		stdout: '',
		stderr_starts: 'Error: Error reading file missing_file_for_strip.ts: '
	},
	{
		name: 'a second positional is refused by the argument parser',
		args: ['strip', 'a.ts', 'b.ts'],
		status: 1,
		stdout: '',
		stderr_starts: 'Unrecognized argument: b.ts'
	},
	{
		name: '--parser is not a strip flag',
		args: ['strip', '--parser', 'ts', '--content', 'let x: T;'],
		status: 1,
		stdout: '',
		stderr_starts: 'Unrecognized argument: --parser'
	},
	{
		name: 'help strip prints the usage',
		args: ['help', 'strip'],
		status: 0,
		stderr: ''
	}
];

/**
 * Register the CLI rows over one `tsv` bin.
 *
 * @param label - names the bin in the suite's title
 * @param run - spawns the bin
 */
export function register_strip_cli_suite(label: string, run: CliRunner): void {
	describe(`tsv strip: ${label}`, () => {
		for (const row of STRIP_CLI_ROWS) {
			it(row.name, () => {
				const dir = mkdtempSync(join(tmpdir(), 'tsv-strip-cli-'));
				try {
					const file_path = join(dir, row.file_name ?? 'a.ts');
					if (row.file !== undefined) writeFileSync(file_path, row.file);
					const args = row.args.map((arg) => (arg === 'FILE' ? file_path : arg));
					const result = run(args, row.stdin, dir);
					assert.equal(result.status, row.status, `stderr: ${result.stderr}`);
					if (row.stdout !== undefined) assert.equal(result.stdout, row.stdout);
					if (row.stderr !== undefined) assert.equal(result.stderr, row.stderr);
					if (row.stderr_starts !== undefined) {
						assert.ok(
							result.stderr.startsWith(row.stderr_starts),
							`stderr: ${JSON.stringify(result.stderr)}`
						);
					}
					if (row.stderr_includes !== undefined) {
						assert.ok(
							result.stderr.includes(row.stderr_includes),
							`stderr: ${JSON.stringify(result.stderr)}`
						);
					}
					if (row.args[0] === 'help') {
						assert.ok(result.stdout.startsWith('Usage: tsv strip'), result.stdout);
						assert.ok(result.stdout.includes('--source-type'), result.stdout);
					}
					if (row.file !== undefined) {
						assert.equal(readFileSync(file_path, 'utf8'), row.file, 'strip never writes the file');
					}
				} finally {
					rmSync(dir, { recursive: true, force: true });
				}
			});
		}

		it('the top-level help lists strip', () => {
			const result = run(['--help']);
			assert.equal(result.status, 0, result.stderr);
			assert.match(result.stdout, /^\s+strip\s/m);
		});
	});
}
