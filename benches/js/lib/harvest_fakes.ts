/**
 * In-process fakes the harvest suites share: `git` answering `rev-parse` from a table, and the
 * checkout's files as a map that records every write and directory made. Test support only,
 * imported by the `*_test.ts` suites beside each harvest.
 *
 * @module
 */

import type { StampGit, StampWrites } from './harvest_stamp.ts';

/** A `git` whose `-C <repo> rev-parse <rev>` answers `ids[repo]` (or `ids['<repo>:<tree>']`
 * for `HEAD:<tree>`), and fails for a repo the table doesn't hold, recording each argv. */
export const fake_stamp_git = (
	ids: Record<string, string>
): StampGit & { calls: Array<ReadonlyArray<string>> } => {
	const calls: Array<ReadonlyArray<string>> = [];
	return {
		calls,
		out: async (args) => {
			calls.push(args);
			const [, repo, , rev] = args;
			const key = rev === 'HEAD' ? repo! : `${repo}:${rev!.slice('HEAD:'.length)}`;
			const id = ids[key];
			return id === undefined
				? { kind: 'exited', code: 128, stdout: '' }
				: { kind: 'exited', code: 0, stdout: `${id}\n` };
		}
	};
};

/** The checkout's files as a map, every write and directory made recorded in order. */
export const fake_files = (
	files: Record<string, string> = {}
): StampWrites & { files: Map<string, string>; log: string[] } => {
	const map = new Map(Object.entries(files));
	const log: string[] = [];
	return {
		files: map,
		log,
		exists: async (path) => map.has(path) || [...map.keys()].some((k) => k.startsWith(`${path}/`)),
		read_text: async (path) => {
			const text = map.get(path);
			if (text === undefined) throw new Error(`not found: ${path}`);
			return text;
		},
		write: async (path, data) => {
			log.push(`write ${path}`);
			map.set(path, data);
		},
		mkdir: async (path) => {
			log.push(`mkdir ${path}`);
		}
	};
};

/** A 40-hex id made from `seed`, so a test's ids read apart. */
export const fake_id = (seed: string): string =>
	[...seed]
		.map((c) => c.charCodeAt(0).toString(16))
		.join('')
		.padEnd(40, '0')
		.slice(0, 40);

/** `values` as `JSON.stringify(values, null, '\t')` spells a list of strings, written by hand so a
 * change to the harvests' serialization fails the suites. */
export const tab_json_strings = (values: ReadonlyArray<string>): string =>
	values.length === 0 ? '[]' : `[\n${values.map((v) => `\t"${v}"`).join(',\n')}\n]`;
