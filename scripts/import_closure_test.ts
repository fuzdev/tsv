/**
 * `scan_imports`, `resolve_import`, and `walk_import_closure`: what counts as an import and
 * what doesn't, the type-only modifier, the specifier rules, and the two walks' rules over an
 * in-memory tree.
 */

import { deepStrictEqual, strictEqual, throws } from 'node:assert';
import { describe, test } from 'vitest';

import { resolve_import, scan_imports, walk_import_closure } from './import_closure.ts';

/** `source`'s imports as `<specifier>` plus ` type` / ` dynamic` and `@<line>`. */
const imports = (source: string): string[] =>
	scan_imports(source).imports.map(
		(r) => `${r.specifier}${r.type_only ? ' type' : ''}${r.dynamic ? ' dynamic' : ''}@${r.line}`
	);

describe('scan_imports', () => {
	test('every static form', () => {
		deepStrictEqual(
			imports(
				[
					"import a from './a.ts';",
					"import * as b from './b.ts';",
					"import { c, d as e } from './c.ts';",
					"import f, { g } from './f.ts';",
					"import h, * as i from './h.ts';",
					"import './side.ts';",
					"export * from './star.ts';",
					"export * as ns from './ns.ts';",
					"export { j, k as default } from './j.ts';",
					"import { 'a-b' as l } from './str.ts';",
					"import m from './json.json' with { type: 'json' };"
				].join('\n')
			),
			[
				'./a.ts@1',
				'./b.ts@2',
				'./c.ts@3',
				'./f.ts@4',
				'./h.ts@5',
				'./side.ts@6',
				'./star.ts@7',
				'./ns.ts@8',
				'./j.ts@9',
				'./str.ts@10',
				'./json.json@11'
			]
		);
	});

	test('anywhere on a line, after code or a comment, and across lines', () => {
		deepStrictEqual(
			imports(
				[
					"export const x = 1; import { y } from './after_code.ts';",
					"/* a note */ import './after_comment.ts';",
					'/**',
					' * a doc',
					" */ import z from './after_doc.ts';",
					'import {',
					'\tw',
					"} from './multiline.ts';",
					'function f() {',
					"\treturn import('./inside.ts');",
					'}'
				].join('\n')
			),
			[
				'./after_code.ts@1',
				'./after_comment.ts@2',
				'./after_doc.ts@5',
				'./multiline.ts@6',
				'./inside.ts dynamic@10'
			]
		);
	});

	test('CRLF and CR line ends count lines as LF does', () => {
		deepStrictEqual(imports("// c\r\nexport const x = 1;\r\nimport './a.ts';\r\n"), ['./a.ts@3']);
		deepStrictEqual(imports("// c\rimport './a.ts';\r"), ['./a.ts@2']);
		deepStrictEqual(imports("﻿#!/usr/bin/env node\nimport './a.ts';"), ['./a.ts@2']);
	});

	test('nothing inside a comment, string, template, or regular expression is an import', () => {
		deepStrictEqual(
			imports(
				[
					"// import './line_comment.ts';",
					"/* import './block_comment.ts'; */",
					'const s = "import x from \'./string.ts\'";',
					"const t = 'import(\\'./escaped.ts\\')';",
					"const u = `import './template.ts'`;",
					"const v = `${\"x\"} import './substituted.ts' ${`import('./nested.ts')`}`;",
					"const r = /import '.\\/regex.ts'/;",
					"const q = x / 2; import './after_division.ts';",
					"const d = a.import('./member.ts'); const m = import.meta.url;",
					"const o = { import: './key.ts' };"
				].join('\n')
			),
			['./after_division.ts@8']
		);
	});

	test('a dynamic import of a literal is followed, of anything else reported', () => {
		const scanned = scan_imports(
			[
				"await import('./literal.ts');",
				'await import(`./template.ts`);',
				"await import('./attributes.json', { with: { type: 'json' } });",
				'await import(name);',
				'await import(`./${name}.ts`);',
				"await import('./a' + '.ts');"
			].join('\n')
		);
		deepStrictEqual(
			scanned.imports.map((r) => `${r.specifier}@${r.line}`),
			['./literal.ts@1', './template.ts@2', './attributes.json@3']
		);
		deepStrictEqual(scanned.computed, [4, 5, 6]);
	});

	test('a clause is type-only only when `type` modifies the whole of it', () => {
		deepStrictEqual(
			imports(
				[
					"import type A from './a.ts';",
					"import type { B } from './b.ts';",
					"import type * as C from './c.ts';",
					"export type { D } from './d.ts';",
					"export type * from './e.ts';",
					"import { type F } from './f.ts';",
					"import type from './default_named_type.ts';",
					"import type, { G } from './g.ts';",
					"import type from from './from.ts';"
				].join('\n')
			),
			[
				'./a.ts type@1',
				'./b.ts type@2',
				'./c.ts type@3',
				'./d.ts type@4',
				'./e.ts type@5',
				'./f.ts@6',
				'./default_named_type.ts@7',
				'./g.ts@8',
				'./from.ts type@9'
			]
		);
	});

	test('an export that is no re-export, and TypeScript forms, are no import', () => {
		deepStrictEqual(
			imports(
				[
					'export { a, b };',
					"import type T from './t.ts';",
					'export const from = 1;',
					'export type X = { from: string };',
					"export default 'x';",
					"import x = require('./required.ts');"
				].join('\n')
			),
			['./t.ts type@2']
		);
	});
});

describe('resolve_import', () => {
	test('relative specifiers resolve as URL paths do', () => {
		deepStrictEqual(resolve_import('scripts/pgo/pgo.ts', '../napi_host.ts'), {
			kind: 'file',
			path: 'scripts/napi_host.ts'
		});
		deepStrictEqual(resolve_import('a/b.ts', './c/./d/../e%20f.ts?raw#x'), {
			kind: 'file',
			path: 'a/c/e f.ts'
		});
	});

	test('builtins, packages, node_modules, and the root', () => {
		deepStrictEqual(resolve_import('a.ts', 'node:fs'), { kind: 'builtin' });
		for (const specifier of [
			'vitest',
			'@fuzdev/gro/fs.ts',
			'/abs.ts',
			'file:///x.ts',
			'jsr:@std/x'
		]) {
			deepStrictEqual(resolve_import('a.ts', specifier), { kind: 'package' }, specifier);
		}
		deepStrictEqual(resolve_import('a/b.ts', '../node_modules/x/index.js'), { kind: 'package' });
		deepStrictEqual(resolve_import('a/b.ts', '../../x.ts'), { kind: 'above_root' });
	});
});

describe('walk_import_closure', () => {
	const TREE: Record<string, string> = {
		'entry.ts': [
			"import { a } from './lib/a.ts';",
			"import type { T } from './lib/types.ts';",
			"import 'node:fs';",
			"import { b } from 'pkg';",
			"export const load = () => import('./lib/lazy.ts');",
			'export const any = (x: string) => import(x);'
		].join('\n'),
		'lib/a.ts': "import { c } from './c.ts';\nexport const a = c;",
		'lib/c.ts': "import data from '../data.json' with { type: 'json' };\nexport const c = data;",
		'lib/types.ts': "import type { P } from 'types_only_pkg';\nexport type T = P;",
		'lib/lazy.ts': "import 'node:os';",
		'data.json': '{}'
	};
	const read = (path: string): string => {
		const source = TREE[path];
		if (source === undefined) throw new Error(`no such file ${path}`);
		return source;
	};

	test('the stamp key walk follows type-only imports', () => {
		const closure = walk_import_closure(['entry.ts'], { read, follow_type_only: true });
		deepStrictEqual(closure.files, [
			'entry.ts',
			'lib/a.ts',
			'lib/c.ts',
			'data.json',
			'lib/types.ts',
			'lib/lazy.ts'
		]);
		deepStrictEqual(
			closure.packages.map((f) => `${f.from}: ${f.specifier}`),
			['lib/types.ts: types_only_pkg', 'entry.ts: pkg']
		);
		deepStrictEqual(
			closure.builtins.map((f) => `${f.from}: ${f.specifier}`),
			['entry.ts: node:fs', 'lib/lazy.ts: node:os']
		);
		deepStrictEqual(closure.computed, [{ from: 'entry.ts', line: 6 }]);
	});

	test('the load walk stops at a type-only import', () => {
		const closure = walk_import_closure(['entry.ts'], { read, follow_type_only: false });
		deepStrictEqual(closure.files, [
			'entry.ts',
			'lib/a.ts',
			'lib/c.ts',
			'data.json',
			'lib/lazy.ts'
		]);
		deepStrictEqual(
			closure.packages.map((f) => `${f.specifier}${f.type_only ? ' type' : ''}`),
			['pkg']
		);
	});

	test('a missing file and an import above the root fail the walk', () => {
		throws(
			() => walk_import_closure(['missing.ts'], { read, follow_type_only: true }),
			/no such file missing\.ts/
		);
		throws(
			() =>
				walk_import_closure(['lib/up.ts'], {
					read: () => "import '../../up.ts';",
					follow_type_only: true
				}),
			/lib\/up\.ts:1: \.\.\/\.\.\/up\.ts climbs above the root/
		);
	});

	test('each file once, entries first', () => {
		const closure = walk_import_closure(['lib/c.ts', 'entry.ts', 'lib/a.ts'], {
			read,
			follow_type_only: true
		});
		strictEqual(closure.files.length, new Set(closure.files).size);
		deepStrictEqual(closure.files.slice(0, 2), ['lib/c.ts', 'data.json']);
	});
});
