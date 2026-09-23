/**
 * The Prettier-suite filter (`prettier_fixtures.ts`) mirrors two pieces of
 * Prettier's own test runner — the implicit verify parsers of `get-parsers.js`
 * and `shouldThrowOnFormat` of `utilities.js` — and evaluates the suites' spec
 * files to read their verdicts. These pin that mirror to the runner's documented
 * behavior with hand-written specs, so a drift in the reader shows here rather
 * than as a quietly moved corpus count.
 */

import { deepStrictEqual, strictEqual, throws } from 'node:assert';

import {
	type PrettierFormatTestCall,
	ast_has_jsx,
	evaluate_prettier_spec,
	prettier_error_test_dir,
	prettier_expects_throw,
	prettier_fixture_content_skip,
	prettier_fixture_expected_valid,
	prettier_fixture_has_front_matter,
	prettier_fixture_has_markers,
	prettier_parsers_run
} from './prettier_fixtures.ts';

Deno.test('jsx: a JSX node anywhere in an ESTree/Babel AST', () => {
	strictEqual(ast_has_jsx({ type: 'Program', body: [] }), false);
	strictEqual(
		ast_has_jsx({
			type: 'Program',
			body: [{ type: 'ExpressionStatement', expression: { type: 'JSXElement' } }]
		}),
		true
	);
	// nested in an argument list; a fragment counts; a comment array is walked harmlessly
	strictEqual(
		ast_has_jsx({
			type: 'CallExpression',
			arguments: [{ type: 'Identifier' }, { type: 'JSXFragment', children: [] }],
			comments: [{ type: 'CommentLine', value: 'x' }]
		}),
		true
	);
	// a type annotation is not JSX, and neither is a string that merely says so
	strictEqual(
		ast_has_jsx({ type: 'TypeCastExpression', typeAnnotation: { type: 'TypeAnnotation' } }),
		false
	);
	strictEqual(ast_has_jsx({ type: 'StringLiteral', value: 'JSXElement' }), false);
	// a cyclic AST (babel's parent links) terminates
	const cyclic: Record<string, unknown> = { type: 'Program', body: [] };
	(cyclic.body as unknown[]).push({ type: 'ReturnStatement', parent: cyclic });
	strictEqual(ast_has_jsx(cyclic), false);
});

Deno.test('markers: range and cursor placeholders, anywhere in the source', () => {
	strictEqual(prettier_fixture_has_markers('const a = 1;\n'), false);
	strictEqual(
		prettier_fixture_has_markers('<<<PRETTIER_RANGE_START>>>a<<<PRETTIER_RANGE_END>>>'),
		true
	);
	strictEqual(prettier_fixture_has_markers('foo(<|>a);'), true);
});

Deno.test('front matter: getFrontMatter mirrored — an opening AND a closing fence', () => {
	strictEqual(prettier_fixture_has_front_matter('---\ntitle: x\n---\na {}\n'), true);
	strictEqual(prettier_fixture_has_front_matter('\uFEFF---\r\ntitle: x\r\n---\r\n'), true);
	// CR alone ends a line too: Prettier folds CR / CRLF to LF before it looks
	strictEqual(prettier_fixture_has_front_matter('---\rtitle: x\r---\ra {}\r'), true);
	// an explicit language after the fence; toml's own fence
	strictEqual(prettier_fixture_has_front_matter('---yaml\ntitle: x\n---\n'), true);
	strictEqual(prettier_fixture_has_front_matter('+++\ntitle = "x"\n+++\n'), true);
	// pandoc's `...` closes YAML alone — not toml, not a named non-yaml language
	strictEqual(prettier_fixture_has_front_matter('---\ntitle: x\n...\n'), true);
	strictEqual(prettier_fixture_has_front_matter('+++\ntitle = "x"\n...\n'), false);
	strictEqual(prettier_fixture_has_front_matter('---toml\ntitle = "x"\n...\n'), false);
	// the closing fence need only HEAD its line, as Prettier reads it
	strictEqual(prettier_fixture_has_front_matter('---\nx\n---y\n'), true);
	// a lone opening fence is not front matter: the whole file goes to the parser
	strictEqual(prettier_fixture_has_front_matter('---\naaa\nb---\n\na {}\n'), false);
	strictEqual(prettier_fixture_has_front_matter('---\n'), false);
	strictEqual(prettier_fixture_has_front_matter('---'), false);
	// a fence that isn't the file's first three bytes is a selector or an operator
	strictEqual(prettier_fixture_has_front_matter(' ---\nx\n---\n'), false);
	strictEqual(prettier_fixture_has_front_matter('a {}\n---\nx\n---\n'), false);
	strictEqual(prettier_fixture_content_skip('a {}\n'), false);
	strictEqual(prettier_fixture_content_skip('---\nx\n---\na {}\n'), true);
});

Deno.test('parsers run: get-parsers.js implicit verify parsers', () => {
	deepStrictEqual(prettier_parsers_run('js', ['babel']), [
		'babel',
		'acorn',
		'espree',
		'meriyah',
		'oxc',
		'__babel_estree'
	]);
	// the ES verify parsers are added for JS suites only
	deepStrictEqual(prettier_parsers_run('typescript', ['babel']), ['babel', '__babel_estree']);
	deepStrictEqual(prettier_parsers_run('typescript', ['typescript']), [
		'typescript',
		'babel-ts',
		'oxc-ts'
	]);
	deepStrictEqual(prettier_parsers_run('js', ['flow']), ['flow', 'hermes']);
	deepStrictEqual(prettier_parsers_run('css', ['css']), ['css']);
	// an `_errors_/` directory runs exactly what it names
	deepStrictEqual(prettier_parsers_run('js', ['babel', 'typescript'], true), [
		'babel',
		'typescript'
	]);
});

Deno.test('error test: isErrorTest — a directory at or below one named _errors_', () => {
	strictEqual(prettier_error_test_dir('/p/tests/format/js/_errors_/x'), true);
	strictEqual(prettier_error_test_dir('/p/tests/format/js/_errors_'), true);
	strictEqual(prettier_error_test_dir('/p/tests/format/js/errors'), false);
	strictEqual(prettier_error_test_dir('/p/tests/format/js/_errors_x/y'), false);
});

Deno.test('expects throw: shouldThrowOnFormat', () => {
	strictEqual(prettier_expects_throw('a.js', 'acorn', undefined), false);
	strictEqual(prettier_expects_throw('a.js', 'acorn', true), true);
	strictEqual(prettier_expects_throw('a.js', 'acorn', { acorn: true }), true);
	strictEqual(prettier_expects_throw('a.js', 'acorn', { acorn: ['a.js'] }), true);
	strictEqual(prettier_expects_throw('b.js', 'acorn', { acorn: ['a.js'] }), false);
	strictEqual(prettier_expects_throw('a.js', 'espree', { acorn: true }), false);
});

Deno.test('verdict: kept when any spec-grammar parser is expected to accept', () => {
	// a babel-only proposal dir: every ES parser declared throwing
	const proposal: PrettierFormatTestCall[] = [
		{
			parsers: ['babel'],
			errors: { acorn: true, espree: true, meriyah: true, oxc: true, 'oxc-ts': true }
		}
	];
	strictEqual(prettier_fixture_expected_valid('do.js', 'js', proposal), false);
	// the same dir under a non-JS suite adds no verify parsers, but the declared
	// errors are still Prettier's word: rejected, not "no verdict"
	strictEqual(prettier_fixture_expected_valid('do.js', 'typescript', proposal), false);
	// per-file lists: only the named file is rejected
	const lists: PrettierFormatTestCall[] = [
		{ parsers: ['typescript'], errors: { typescript: ['3.ts'], 'oxc-ts': ['3.ts'] } }
	];
	strictEqual(prettier_fixture_expected_valid('3.ts', 'typescript', lists), false);
	strictEqual(prettier_fixture_expected_valid('4.ts', 'typescript', lists), true);
	// one spec-grammar parser without an error keeps the file
	const partial: PrettierFormatTestCall[] = [
		{ parsers: ['typescript'], errors: { typescript: ['3.ts'] } }
	];
	strictEqual(prettier_fixture_expected_valid('3.ts', 'typescript', partial), true);
	// a later call (an options variant) can be the one that accepts
	const variants: PrettierFormatTestCall[] = [
		{ parsers: ['babel'], errors: { acorn: true, espree: true, meriyah: true, oxc: true } },
		{ parsers: ['babel', 'typescript'], errors: undefined }
	];
	strictEqual(prettier_fixture_expected_valid('x.js', 'js', variants), true);
	// nothing standard is run: no verdict
	strictEqual(
		prettier_fixture_expected_valid('x.js', 'js', [{ parsers: ['flow'], errors: undefined }]),
		null
	);
	// a declared error for a spec-grammar parser the call never runs is still a
	// verdict (typescript/definite: `["babel-ts"]` with `errors.typescript`)
	const declared_unrun: PrettierFormatTestCall[] = [
		{ parsers: ['babel-ts'], errors: { typescript: ['definite.ts'] } }
	];
	strictEqual(prettier_fixture_expected_valid('definite.ts', 'typescript', declared_unrun), false);
	strictEqual(prettier_fixture_expected_valid('other.ts', 'typescript', declared_unrun), null);
	// ...but never a KEEP: an unrun parser without an error says nothing
	strictEqual(
		prettier_fixture_expected_valid('x.ts', 'typescript', [
			{ parsers: ['babel-ts'], errors: { flow: true } }
		]),
		null
	);
	// an `_errors_/` directory: every named parser throws unless the spec says otherwise
	const error_dir: PrettierFormatTestCall[] = [
		{ parsers: ['babel', 'typescript'], errors: undefined }
	];
	strictEqual(prettier_fixture_expected_valid('x.js', 'js', error_dir, true), false);
	strictEqual(prettier_fixture_expected_valid('x.js', 'js', error_dir), true);
	const error_dir_override: PrettierFormatTestCall[] = [
		{ parsers: ['babel', 'typescript'], errors: { babel: true } }
	];
	strictEqual(prettier_fixture_expected_valid('x.js', 'js', error_dir_override, true), true);
});

Deno.test('evaluate: the spec shapes the suites use', () => {
	deepStrictEqual(
		evaluate_prettier_spec('runFormatTest(import.meta, ["babel", "flow", "typescript"]);\n', '/s'),
		[{ parsers: ['babel', 'flow', 'typescript'], errors: undefined }]
	);
	// consts, spreads, and a second call with options
	const spec = `
const invalid = ["a.js", "b.js"];
const errors = { acorn: [...invalid, "c.js"], typescript: true };
runFormatTest(import.meta, ["babel"], { errors });
runFormatTest(import.meta, ["babel"], { semi: false, errors });
`;
	deepStrictEqual(evaluate_prettier_spec(spec, '/s'), [
		{ parsers: ['babel'], errors: { acorn: ['a.js', 'b.js', 'c.js'], typescript: true } },
		{ parsers: ['babel'], errors: { acorn: ['a.js', 'b.js', 'c.js'], typescript: true } }
	]);
	// imports are stubbed: a tag on a snippet, a default import, jest globals
	const with_imports = `
import { outdent } from "outdent";
import fs from "node:fs";
const snippets = fs.readdirSync(new URL("../eol/", import.meta.url)).map((name) => ({ name, code: outdent\`a\` + "\\n" }));
runFormatTest({ importMeta: import.meta, snippets }, ["babel"]);
test("x", () => { expect(1).toBe(1); });
`;
	deepStrictEqual(evaluate_prettier_spec(with_imports, '/s'), [
		{ parsers: ['babel'], errors: undefined }
	]);
	// an ARRAY `errors` names no parser — `shouldThrowOnFormat` indexes it by one
	deepStrictEqual(
		evaluate_prettier_spec('runFormatTest(import.meta, ["babel"], { errors: ["a.js"] });\n', '/s'),
		[{ parsers: ['babel'], errors: {} }]
	);
	// a `describe` body runs at collection time, so a call inside one is recorded
	deepStrictEqual(
		evaluate_prettier_spec(
			'describe("x", () => { runFormatTest(import.meta, ["typescript"]); });\n',
			'/s'
		),
		[{ parsers: ['typescript'], errors: undefined }]
	);
});

Deno.test('evaluate: an import/export spelled inside a snippet is text, not a statement', () => {
	const spec = `import { outdent } from "outdent";
runFormatTest(
	{ importMeta: import.meta, snippets: [outdent\`
		@a
		export @b class A {}
	\`] },
	["babel"],
);
`;
	deepStrictEqual(evaluate_prettier_spec(spec, '/s'), [{ parsers: ['babel'], errors: undefined }]);
});

Deno.test('evaluate: refuses what it cannot read, naming the spec', () => {
	throws(
		() => evaluate_prettier_spec('runFormatTest(import.meta, "babel");\n', '/spec/x'),
		/non-string parser list: \/spec\/x/
	);
	throws(
		() =>
			evaluate_prettier_spec('runFormatTest(import.meta, ["babel"], { errors: 1 });\n', '/spec/x'),
		/unreadable `errors`: \/spec\/x/
	);
	throws(
		() =>
			evaluate_prettier_spec(
				'export const a = 1;\nrunFormatTest(import.meta, ["css"]);',
				'/spec/x'
			),
		/failed to evaluate: \/spec\/x: SyntaxError/
	);
	throws(
		() =>
			evaluate_prettier_spec(
				'import { parsers } from "./p.js";\nrunFormatTest(import.meta, [...parsers, "babel"]);',
				'/spec/x'
			),
		/failed to evaluate: \/spec\/x: .*iterated an imported value/
	);
	throws(
		() => evaluate_prettier_spec('throw new Error("boom");', '/spec/x'),
		/evaluate: \/spec\/x/
	);
	// every way a stubbed import could stand in for a list's contents and read as
	// fewer entries — concatenated, spread into an object, or an `errors` value —
	// refuses rather than dropping the entries it stands for
	throws(
		() =>
			evaluate_prettier_spec(
				'import { parsers } from "./p.js";\nrunFormatTest(import.meta, ["babel"].concat(parsers));',
				'/spec/x'
			),
		/failed to evaluate: \/spec\/x: .*concatenated an imported value/
	);
	throws(
		() =>
			evaluate_prettier_spec(
				'import { errors } from "./e.js";\nrunFormatTest(import.meta, ["babel"], { errors: { ...errors } });',
				'/spec/x'
			),
		/failed to evaluate: \/spec\/x: .*spread an imported value/
	);
	throws(
		() =>
			evaluate_prettier_spec(
				'import { invalid } from "./i.js";\nrunFormatTest(import.meta, ["babel"], { errors: { acorn: invalid } });',
				'/spec/x'
			),
		/unreadable `errors`: \/spec\/x/
	);
	throws(
		() =>
			evaluate_prettier_spec(
				'runFormatTest(import.meta, ["babel"], { errors: { acorn: ["a.js", 1] } });',
				'/spec/x'
			),
		/unreadable `errors`: \/spec\/x/
	);
	// a spec whose calls this cannot see records none — never "no verdict"
	throws(
		() => evaluate_prettier_spec('import { run } from "./r.js";\nrun(import.meta);', '/spec/x'),
		/no `runFormatTest` call: \/spec\/x/
	);
});
