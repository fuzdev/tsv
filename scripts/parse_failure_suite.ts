/**
 * The parse-failure contract every tsv npm package's facade publishes, as one table run
 * by both package suites (`scripts/test_npm.ts` over each WASM variant's auto-init and
 * lazy entries, `scripts/test_napi_npm.ts` over the native loader) and, under Bun, by
 * `scripts/test_bun.ts` over the staged packages, so the two engines are held to one
 * table.
 *
 * Every `parse_*`, `parse_*_json` and `format_*` call on a source that does not parse
 * returns `{ok: false, error}` (those two keys, in that order), whose `error` is a plain
 * object with exactly `kind` (`'parse'`), `message`, `start` and `loc`, where `loc` is
 * `create_locator(source, {language}).position_at(start)` and the message's `line:col`
 * header is `loc.line:loc.column + 1`, with no leading BOM echoed in the excerpt after it.
 * The rows reach what the definition turns on: an astral character ahead of the error (two
 * UTF-16 units), a leading BOM (counted by TypeScript, elided by Svelte and CSS), a lone CR,
 * U+2028 and U+2029 (a line for TypeScript, a character for Svelte and CSS), CRLF on the
 * format path (whose parse reads the CR-folded text, yet reports the caller's own
 * coordinates), errors inside a Svelte `<script>` (plain and `lang="ts"`), `<style>`,
 * template expression, attribute expression and `{#snippet}` head, errors at the end of the
 * source, the format path's Module→Script fallback beside a source type named outright
 * (exact on every export), and malformed string and template escapes, reported at their
 * own backslash. Invisible characters — a BOM, U+2028, U+2029, a CR — are spelled as
 * escapes, never written literally.
 *
 * Beside the table, the one format **refusal** of a source that parses: a lone CR inside a
 * Svelte in-tag `//` comment, which the format path's CR fold would end early
 * (`tsv_svelte::parse_folded`). The source is valid Svelte, so the refusal is an error of
 * its own kind, `'refused'`, at the CR's point — never the `'parse'` a parse failure is.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { create_locator as source_create_locator } from '../crates/tsv_wasm/npm/locations.js';

export type Language = 'typescript' | 'svelte' | 'css';

export interface ParseFailureRow {
	name: string;
	language: Language;
	source: string;
	/**
	 * The text the error sits at — its first occurrence, or its last with `last` — or `''`
	 * for the end of the source. Omitted where the row pins only the definition.
	 */
	at?: string;
	last?: boolean;
	/** Where the FORMAT's error sits, when its fallback parse reports another than the parse's. */
	format_at?: string;
	/** The options bag every export is called with (TypeScript's `sourceType`). */
	options?: { sourceType: 'script' | 'module' };
}

export const PARSE_FAILURE_ROWS: ReadonlyArray<ParseFailureRow> = [
	{
		name: 'an astral character ahead of the error',
		language: 'typescript',
		source: "const a = '𝒜';\nconst = ;",
		at: '= ;'
	},
	{ name: 'a leading BOM, counted', language: 'typescript', source: '\uFEFFconst = ;', at: '= ;' },
	{ name: 'a lone CR is a line', language: 'typescript', source: 'a;\rconst = ;', at: '= ;' },
	{ name: 'U+2028 is a line', language: 'typescript', source: 'a;\u2028const = ;', at: '= ;' },
	{ name: 'U+2029 is a line', language: 'typescript', source: 'a;\u2029const = ;', at: '= ;' },
	{ name: 'CRLF', language: 'typescript', source: "a = '𝒜';\r\nconst = ;\r\n", at: '= ;' },
	{ name: 'CR, CRLF', language: 'typescript', source: 'a;\r\r\nconst = ;', at: '= ;' },
	{ name: 'the end of the source', language: 'typescript', source: 'function f() {\n' },
	{
		// the module grammar fails at `with`; the format's script retry gets further, and
		// the further error is the file's own
		name: 'the format path falls back to the script grammar',
		language: 'typescript',
		source: 'with (a) {}\nconst = ;',
		at: 'with',
		format_at: '= ;'
	},
	{
		// a named goal is exact on every export: no retry
		name: 'sourceType module, named',
		language: 'typescript',
		source: 'with (a) {}\nconst = ;',
		options: { sourceType: 'module' },
		at: 'with'
	},
	{
		name: 'sourceType script, named',
		language: 'typescript',
		source: 'with (a) {}\r\nconst = ;',
		options: { sourceType: 'script' },
		at: '= ;'
	},
	{
		name: 'a malformed string escape, at its backslash',
		language: 'typescript',
		source: "let a = '𝒜';\nlet s = '\\u{zz}';",
		at: '\\'
	},
	{
		name: 'a malformed template escape, at its backslash',
		language: 'typescript',
		source: 'let a;\r\nlet t = `x${a}\\x4`;',
		at: '\\'
	},
	{
		name: 'inside <script>, past an astral character',
		language: 'svelte',
		source: "<script>\nlet a = '𝒜';\nconst = ;\n</script>\n",
		at: '= ;'
	},
	{
		name: 'inside <script lang="ts">',
		language: 'svelte',
		source: '<script lang="ts">\n\tlet a: 𝒜 = 1;\n\tconst = ;\n</script>\n',
		at: '= ;'
	},
	{
		name: 'a malformed string escape in <script>, past a BOM, at its backslash',
		language: 'svelte',
		source: "\uFEFF<p>x</p>\n<script>\n\tlet s = '\\x4';\n</script>\n",
		at: '\\'
	},
	{
		name: 'a malformed string escape in a template expression, at its backslash',
		language: 'svelte',
		source: "<p>𝒜</p>\r\n{'\\u{zz}'}",
		at: '\\'
	},
	{
		name: 'a leading BOM, elided, before a template expression',
		language: 'svelte',
		source: '\uFEFF<p>x</p>\n{a +}',
		at: '}',
		last: true
	},
	{
		name: 'a lone CR in the markup is not a line',
		language: 'svelte',
		source: '<p>a\rb</p>\n{a +}',
		at: '}',
		last: true
	},
	{
		name: 'CR, CRLF in the markup is one line',
		language: 'svelte',
		source: '<p>x</p>\r\r\n{a +}',
		at: '}',
		last: true
	},
	{
		name: 'U+2028 in the markup is not a line',
		language: 'svelte',
		source: '<p>a\u2028b</p>{a +}',
		at: '}',
		last: true
	},
	{
		name: 'inside <style>, CRLF',
		language: 'svelte',
		source: '<style>\r\na { color: red; }\r\n𝒜 {\r\n</style>\r\n',
		at: '</style>'
	},
	{
		name: 'inside a template expression',
		language: 'svelte',
		source: '<div>{a b}</div>',
		at: 'b}'
	},
	{
		name: 'inside an attribute expression',
		language: 'svelte',
		source: '<p>𝒜</p>\n<div title={a +}></div>',
		at: '}>'
	},
	{
		name: 'inside a {#snippet} head',
		language: 'svelte',
		source: '<p>x</p>\r\n{#snippet f(a b)}{/snippet}',
		at: 'b)'
	},
	{ name: 'the end of the source', language: 'svelte', source: '<p>𝒜</p>\n<div', at: '' },
	{ name: 'a leading BOM, elided, at the end', language: 'css', source: '\uFEFFa {', at: '' },
	{ name: 'a lone CR is not a line', language: 'css', source: 'a { color: red; }\rb {', at: '' },
	{
		name: 'U+2028, U+2029 and an astral character in a comment',
		language: 'css',
		source: '/* \u2028 \u2029 𝒜 */\n𝒜 } b {}',
		at: '} b'
	},
	{ name: 'CRLF', language: 'css', source: 'a {\r\n\tcolor: red;\r\n}\r\n𝒜 } b {}', at: '} b' }
];

/** The `start` a row's `at` names: a UTF-16 index into the text the wire indexes. */
const expected_start = (row: ParseFailureRow, at: string): number => {
	const index =
		at === '' ? row.source.length : row.last ? row.source.lastIndexOf(at) : row.source.indexOf(at);
	assert.ok(index >= 0, `${row.name}: \`${at}\` is in the source`);
	const elided = row.language !== 'typescript' && row.source.charCodeAt(0) === 0xfeff;
	return elided ? index - 1 : index;
};

/** The error every parse failure returns, whatever export returned it. */
export interface ReturnedParseError {
	kind: 'parse';
	message: string;
	start: number;
	loc: { line: number; column: number };
}

/**
 * Assert `result` is the published parse failure for `row` — and return its error, for a
 * caller comparing two engines' errors.
 */
export function assert_parse_failure(
	result: unknown,
	row: ParseFailureRow,
	at: string | undefined,
	label: string
): ReturnedParseError {
	assert.ok(
		typeof result === 'object' && result !== null,
		`${label}: a result, got ${String(result)}`
	);
	assert.deepEqual(Object.keys(result), ['ok', 'error'], label);
	const { ok, error } = result as { ok: unknown; error: ReturnedParseError };
	assert.equal(ok, false, label);
	assert.ok(!(error instanceof Error), `${label}: a plain object, not an Error`);
	assert.deepEqual(Object.keys(error), ['kind', 'message', 'start', 'loc'], label);
	assert.deepEqual(structuredClone(error), error, `${label}: structured-clone-able`);
	const { kind, message, start, loc } = error;
	assert.equal(kind, 'parse', label);
	assert.equal(typeof message, 'string', label);
	assert.equal(typeof start, 'number', label);
	assert.deepEqual(
		loc,
		source_create_locator(row.source, { language: row.language }).position_at(start),
		`${label}: loc is the locator's point at start`
	);
	if (at !== undefined) assert.equal(start, expected_start(row, at), `${label}: start`);
	const located = message.split('\n')[1] ?? '';
	const header = located.split(' ')[0];
	assert.equal(header, `${loc.line}:${loc.column + 1}`, `${label}: the message header`);
	assert.ok(
		!located.slice(header.length + 1).startsWith('\uFEFF'),
		`${label}: the excerpt echoes no leading BOM`
	);
	return error;
}

/** The exports a row runs through, and where each one's error sits. */
const calls = (api: Record<string, any>, row: ParseFailureRow) =>
	(
		[
			[`parse_${row.language}`, row.at],
			[`parse_${row.language}_json`, row.at],
			[`format_${row.language}`, row.format_at ?? row.at]
		] as const
	).filter(([name]) => typeof api[name] === 'function');

/**
 * Every error `api` returns over the table, keyed `<row> <export>` — for a suite comparing
 * two packages' errors.
 */
export function parse_failures(api: Record<string, any>): Map<string, ReturnedParseError> {
	const out = new Map<string, ReturnedParseError>();
	for (const row of PARSE_FAILURE_ROWS) {
		for (const [name, at] of calls(api, row)) {
			const label = `${row.language}: ${row.name} — ${name}`;
			out.set(label, assert_parse_failure(api[name](row.source, row.options), row, at, label));
		}
	}
	return out;
}

/** A `div` with no attributes: Svelte's reader runs the `//` comment past the CR to the `\n`. */
export const LONE_CR_TAG_COMMENT = '<div // c\rclass="x"\n>hi</div>\n';

/** The first line of the refusal's message. */
export const LONE_CR_REFUSAL =
	"Lone carriage return inside a '//' comment in a tag: formatting folds it to a line feed, which would end the comment early";

/**
 * Register the table against one package's published API. `api` is the module, or a
 * function returning it for an entry only usable once a test ahead of these has run (the
 * lazy entry, after its `init_sync`); the exports a variant lacks (`parse_*` in the
 * format-only package) are skipped. Where the package exports `create_locator`, its own
 * copy must agree with the source tree's.
 */
export function register_parse_failure_suite(
	label: string,
	api_or_getter: Record<string, any> | (() => Record<string, any>)
): void {
	describe(`parse failures are results with kind 'parse', start and loc: ${label}`, () => {
		for (const row of PARSE_FAILURE_ROWS) {
			it(`${row.language}: ${row.name}`, () => {
				const api = typeof api_or_getter === 'function' ? api_or_getter() : api_or_getter;
				const found = calls(api, row).map(([name, at]) =>
					assert_parse_failure(
						api[name](row.source, row.options),
						row,
						at,
						`${row.language}: ${row.name} — ${name}`
					)
				);
				assert.ok(found.length > 0, 'the package exports this language');
				if (typeof api.create_locator === 'function') {
					for (const { start, loc } of found) {
						assert.deepEqual(
							api.create_locator(row.source, { language: row.language }).position_at(start),
							loc
						);
					}
				}
				// parse and format report one error for one source — the format path's own
				// CR fold and its fallback grammar excepted, where the row says so
				if (row.format_at === undefined && found.length > 1) {
					for (const error of found) assert.deepEqual(error, found[0]);
				}
			});
		}
	});
	describe(`a format refusal of a source that parses is kind 'refused': ${label}`, () => {
		it('svelte: a lone CR inside an in-tag // comment', () => {
			const api = typeof api_or_getter === 'function' ? api_or_getter() : api_or_getter;
			if (typeof api.format_svelte === 'function') {
				const result = api.format_svelte(LONE_CR_TAG_COMMENT);
				assert.deepEqual(Object.keys(result), ['ok', 'error']);
				assert.equal(result.ok, false);
				// the CR's point, the header naming it
				const { message, ...rest } = result.error;
				assert.deepEqual(Object.keys(result.error), ['kind', 'message', 'start', 'loc']);
				assert.deepEqual(rest, { kind: 'refused', start: 9, loc: { line: 1, column: 9 } });
				const [first, located = ''] = message.split('\n');
				assert.equal(first, LONE_CR_REFUSAL);
				assert.equal(located.split(' ')[0], '1:10', 'the CR is named');
				assert.deepEqual(structuredClone(result.error), result.error);
			}
			if (typeof api.parse_svelte === 'function') {
				const result = api.parse_svelte(LONE_CR_TAG_COMMENT);
				assert.equal(result.ok, true, 'the bytes parse');
				assert.deepEqual(result.ast.fragment.nodes[0].attributes, []);
			}
		});
		it('svelte: the refusal point past a leading BOM, astral and non-ASCII text and a CRLF', () => {
			const api = typeof api_or_getter === 'function' ? api_or_getter() : api_or_getter;
			if (typeof api.format_svelte !== 'function') return;
			const source = '\uFEFF<p>é😀</p>\r\n<div // c\rclass="x"\n>hi</div>\n';
			const result = api.format_svelte(source);
			assert.equal(result.ok, false);
			const { message, ...rest } = result.error;
			// the wire's coordinates: the BOM elided, UTF-16 units, LF lines
			assert.deepEqual(rest, { kind: 'refused', start: 21, loc: { line: 2, column: 9 } });
			assert.equal(source.slice(1)[rest.start], '\r', 'start is the CR, the BOM elided');
			assert.equal(message.split('\n')[1].split(' ')[0], '2:10', 'the header prints loc');
			if (typeof api.create_locator === 'function') {
				assert.deepEqual(
					api.create_locator(source, { language: 'svelte' }).position_at(rest.start),
					rest.loc
				);
			}
			if (typeof api.parse_svelte === 'function') {
				assert.equal(api.parse_svelte(source).ok, true, 'the bytes parse');
			}
		});
	});
}

/** The product under `key` of an `{ok: true}` result, failing the test on anything else. */
function ok_value(result: unknown, key: 'code' | 'ast' | 'json'): any {
	const ok = result as { ok?: unknown; error?: unknown } | null | undefined;
	assert.ok(ok?.ok === true, `expected {ok: true}, got ${JSON.stringify(result)}`);
	return (result as Record<string, unknown>)[key];
}

/** A format result's `code`, failing the test on a failure. */
export const ok_code = (result: unknown): string => ok_value(result, 'code');

/** A `parse_<lang>` result's `ast`, failing the test on a failure. */
export const ok_ast = (result: unknown): any => ok_value(result, 'ast');

/** A `parse_<lang>_json` result's `json`, failing the test on a failure. */
export const ok_json = (result: unknown): string => ok_value(result, 'json');
