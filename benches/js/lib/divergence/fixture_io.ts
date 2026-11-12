/**
 * The `FixtureIo` over a read-only handle on `tests/fixtures`, what `divergence:audit` and the
 * fixture-coverage suite read the committed fixtures through.
 *
 * A path that's missing reads as absent: `dir_exists` is `false`, `list` is empty, `read` is
 * `null`. Any other failure throws, a denied read, a path through a file, a directory where a
 * file is expected, so it is never mistaken for a missing fixture. A symlink is followed only
 * where the handle follows one, when its target is relative and stays beneath
 * `tests/fixtures`; `list` names regular files only, so a symlink in a fixture directory is no
 * witness, while `read` of an `input.*` that is a symlink reads its target.
 *
 * @module
 */

import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import { is_fs_error } from '@fuzdev/gro/fs_error.ts';

import type { FixtureIo } from './fixture_cases.ts';

/** The `FixtureIo` over `fixtures`, a handle on `tests/fixtures`. */
export const fixture_io = (fixtures: FsReadDir): FixtureIo => ({
	dir_exists: (fixture_path) => fixtures.stat_sync(fixture_path)?.kind === 'directory',
	list: async (fixture_path) => {
		const names: string[] = [];
		try {
			for await (const entry of fixtures.list(fixture_path)) {
				if (entry.kind === 'file') names.push(entry.name);
			}
		} catch (error) {
			if (is_fs_error(error, 'not_found')) return [];
			throw error;
		}
		return names.sort();
	},
	read: (fixture_path, name) => fixtures.read_text_if_exists(`${fixture_path}/${name}`)
});
