/**
 * Tests for the comparison tools' walk order (`listing_order.ts`): `readdir`'s, which sorts by
 * code point (`strcmp` over UTF-8), not by UTF-16 code unit as `Array.prototype.sort` does.
 * And the TypeScript case discovery (`ts_repo.ts`) walking a tree in that order. Package-free,
 * so it gates in `npm run test:node`.
 */

import { deepStrictEqual } from 'node:assert';
import { test } from 'vitest';

import { compare_code_points, sort_listing } from './listing_order.ts';
import { discover_ts_cases, empty_ts_case_skips, type TsCaseTree } from './ts_repo.ts';

test('names sort by code point, an astral one after U+FFFF as in UTF-8', () => {
	const names = ['b', '\u{1F600}', '�', 'a', 'B', 'a.ts', 'a-b'];
	deepStrictEqual(
		sort_listing(names.map((name) => ({ name }))).map((e) => e.name),
		['B', 'a', 'a-b', 'a.ts', 'b', '�', '\u{1F600}']
	);
	deepStrictEqual(compare_code_points('a', 'a'), 0);
});

test('discovery walks each directory in readdir order, prunes node_modules, and counts skips', async () => {
	const dirs: Record<string, Array<{ name: string; kind: string }>> = {
		'': [
			{ name: 'z.ts', kind: 'file' },
			{ name: 'sub', kind: 'directory' },
			{ name: 'node_modules', kind: 'directory' },
			{ name: 'a.d.ts', kind: 'file' },
			{ name: 'b.tsx', kind: 'file' },
			{ name: 'link.ts', kind: 'symlink' }
		],
		sub: [
			{ name: 'y.ts', kind: 'file' },
			{ name: 'x.ts', kind: 'file' }
		]
	};
	const tree: TsCaseTree = {
		list: async function* (path) {
			const entries = dirs[path];
			if (entries === undefined) throw new Error(`unexpected list of ${path}`);
			yield* entries;
		}
	};
	const skips = empty_ts_case_skips();
	const found = [];
	for await (const c of discover_ts_cases(tree, '../root', skips, 'skip')) found.push(c);
	deepStrictEqual(found, [
		{ path: '../root/link.ts', rel: 'link.ts' },
		{ path: '../root/sub/x.ts', rel: 'sub/x.ts' },
		{ path: '../root/sub/y.ts', rel: 'sub/y.ts' },
		{ path: '../root/z.ts', rel: 'z.ts' }
	]);
	deepStrictEqual(skips, { ...empty_ts_case_skips(), tsx: 1, declaration: 1 });
});
