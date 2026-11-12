/**
 * The walk every mtime guard dates an artifact by (`source_mtime.ts`), over an in-memory
 * `SourceTree`: what counts, how the newest source is labeled, a path with nothing at it read
 * as absent, and every other failure thrown with the path named.
 */

import { deepStrictEqual, rejects } from 'node:assert';
import { test } from 'vitest';

import {
	newest_crate_source_mtime,
	newest_workspace_build_mtime,
	type SourceTree
} from './source_mtime.ts';

/** An entry: a file's or directory's mtime, or what reading it throws. */
type Entry = { ms: number; dir?: true } | { throws: string };

/**
 * A tree over `entries`, keyed by checkout-relative path. `list` gives the paths beneath a
 * directory in insertion order, or throws what `list_throws` names for it.
 */
const make_tree = (
	entries: Record<string, Entry>,
	list_throws: Record<string, string> = {}
): SourceTree => ({
	stat: async (path) => {
		const entry = entries[path];
		if (entry === undefined) return null;
		if ('throws' in entry) throw new Error(entry.throws);
		return { mtime_ms: entry.ms, is_file: entry.dir !== true };
	},
	list: async (path) => {
		if (path in list_throws) throw new Error(list_throws[path]);
		const prefix = `${path}/`;
		const found = Object.keys(entries).filter((p) => p.startsWith(prefix));
		if (found.length === 0 && entries[path] === undefined) return [];
		return found.map((p) => p.slice(prefix.length));
	}
});

test('the newest source wins, labeled crates/-relative', async () => {
	const tree = make_tree({
		'crates/a/Cargo.toml': { ms: 1 },
		'crates/a/src/lib.rs': { ms: 2 },
		'crates/b/Cargo.toml': { ms: 3 },
		'crates/b/build.rs': { ms: 4 },
		'crates/b/src/nested/table.txt': { ms: 6 },
		'crates/b/src/lib.rs': { ms: 5 }
	});
	deepStrictEqual(await newest_crate_source_mtime(tree, ['a', 'b']), {
		ms: 6,
		path: 'b/src/nested/table.txt'
	});
	deepStrictEqual(await newest_crate_source_mtime(tree, ['a']), { ms: 2, path: 'a/src/lib.rs' });
	const manifests = make_tree({ 'crates/b/Cargo.toml': { ms: 3 }, 'crates/b/build.rs': { ms: 4 } });
	deepStrictEqual(await newest_crate_source_mtime(manifests, ['b']), { ms: 4, path: 'b/build.rs' });
});

test('a directory under src/ is not a source', async () => {
	const tree = make_tree({
		'crates/a/src/lib.rs': { ms: 1 },
		'crates/a/src/nested': { ms: 9, dir: true }
	});
	deepStrictEqual(await newest_crate_source_mtime(tree, ['a']), { ms: 1, path: 'a/src/lib.rs' });
});

test('a path with nothing at it counts as absent, and the walk goes on', async () => {
	deepStrictEqual(await newest_crate_source_mtime(make_tree({}), ['a']), { ms: 0, path: '' });
	// listed, then gone by its stat (or a link to nothing): the entries after it still count
	const tree: SourceTree = {
		stat: async (path) => (path === 'crates/a/src/z.rs' ? { mtime_ms: 7, is_file: true } : null),
		list: async () => ['gone.rs', 'z.rs']
	};
	deepStrictEqual(await newest_crate_source_mtime(tree, ['a', 'b']), { ms: 7, path: 'a/src/z.rs' });
});

test("a source that is there but can't be read throws, naming it", async () => {
	const files = { 'crates/a/Cargo.toml': { ms: 1 }, 'crates/a/src/lib.rs': { ms: 2 } };
	for (const [path, tree] of [
		['crates/a/Cargo.toml', make_tree({ ...files, 'crates/a/Cargo.toml': { throws: 'boom' } })],
		['crates/a/build.rs', make_tree({ ...files, 'crates/a/build.rs': { throws: 'boom' } })],
		['crates/a/src/link.rs', make_tree({ ...files, 'crates/a/src/link.rs': { throws: 'boom' } })],
		['crates/a/src', make_tree(files, { 'crates/a/src': 'boom' })]
	] as const) {
		await rejects(newest_crate_source_mtime(tree, ['a']), {
			message: `can't read ${path} to date what it builds: boom`
		});
	}
});

test('the workspace files: the newer wins, labeled by name, a missing one absent', async () => {
	deepStrictEqual(
		await newest_workspace_build_mtime(
			make_tree({ 'Cargo.toml': { ms: 2 }, 'Cargo.lock': { ms: 3 } })
		),
		{ ms: 3, path: 'Cargo.lock' }
	);
	deepStrictEqual(await newest_workspace_build_mtime(make_tree({ 'Cargo.toml': { ms: 2 } })), {
		ms: 2,
		path: 'Cargo.toml'
	});
	deepStrictEqual(await newest_workspace_build_mtime(make_tree({})), { ms: 0, path: '' });
	await rejects(
		newest_workspace_build_mtime(
			make_tree({ 'Cargo.toml': { ms: 2 }, 'Cargo.lock': { throws: 'boom' } })
		),
		{ message: "can't read Cargo.lock to date what it builds: boom" }
	);
});
