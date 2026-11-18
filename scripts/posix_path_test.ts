/**
 * `posix_path`'s functions against `node:path`'s POSIX ones, whose readings they reproduce:
 * every input below, each a shape a script hands them or an edge of the lexical rules.
 */

import { strictEqual, throws } from 'node:assert';
import { posix } from 'node:path';
import { test } from 'vitest';

import { join_path, normalize_path, resolve_path } from './posix_path.ts';

const PATHS: ReadonlyArray<string> = [
	'',
	'.',
	'./',
	'..',
	'../',
	'/',
	'//',
	'///a//b',
	'/..',
	'/../a',
	'/a/../..',
	'a',
	'a/',
	'a/.',
	'a/..',
	'a/../',
	'a/../..',
	'a/../../b/',
	'./a/./b/',
	'../../a',
	'a/b/../../../c',
	'doc/tsv_ts/index.html/../../tsv_lang/struct.Span.html',
	'doc/tsv_ts/fn.f.html/../',
	'/r/tsv/crates/tsv_lang/Cargo.toml/..',
	'/r/tsv/./crates//x/',
	'..foo/bar',
	'a/..foo/...'
];

test('normalize_path reads every path as posix.normalize does', () => {
	for (const path of PATHS) strictEqual(normalize_path(path), posix.normalize(path), path);
});

test('join_path joins as posix.join does, an empty part dropped', () => {
	const parts: ReadonlyArray<ReadonlyArray<string>> = [
		[],
		[''],
		['', ''],
		['doc', 'x'],
		['doc/x', 'index.html'],
		['.', 'NOTICE'],
		['.', ''],
		['', 'a'],
		['a', '', 'b'],
		['crates/tsv_lang', 'README.md'],
		['doc/x/y/page.html', '..', '../z/other.html'],
		['doc/x/page.html', '..', 'sub/'],
		['doc/x/page.html', '..', '../../../out.html'],
		['/', '..'],
		['a/', '/b/'],
		...PATHS.map((path) => ['doc/x', path])
	];
	for (const list of parts) strictEqual(join_path(...list), posix.join(...list), list.join(' , '));
});

test('resolve_path resolves against an absolute base as posix.resolve does', () => {
	for (const base of ['/', '/r/tsv', '/r/tsv/', '/r//tsv/.', '/r/tsv/..']) {
		for (const path of PATHS) {
			strictEqual(resolve_path(base, path), posix.resolve(base, path), `${base} ${path}`);
		}
	}
	strictEqual(resolve_path('/home/u/dev/tsv', '../corpora'), '/home/u/dev/corpora');
	strictEqual(resolve_path('/home/u/dev/tsv', '/elsewhere/corpora/'), '/elsewhere/corpora');
});

test('resolve_path refuses a relative base, which posix.resolve reads against the cwd', () => {
	throws(() => resolve_path('r/tsv', 'a'), TypeError);
	throws(() => resolve_path('', 'a'), TypeError);
});
