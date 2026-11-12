/**
 * `perf:ab`'s root (`scripts/perf_ab.ts`). Its reach:
 *
 * - the checkout, found from this module's location, read-only: the drivers' sources, and the
 *   working directory of `git` and of the drivers
 * - the corpus, and the git index a linked worktree keeps outside the checkout, each opened by
 *   its path, symlinks followed
 * - the snapshot cache, `--cache DIR` or `$HOME/.cache/tsv/perf_ab`, opened by its path (made
 *   when missing), symlinks followed, read and written beneath its handle
 * - each driver a run builds, run beneath that one handle (`spawn.program_in`), made once its
 *   binary is in place, with gro's base environment alone (`DRIVER_PASS_ENV`)
 * - `perf` and `pgrep` when on `PATH` (a run without `perf` grades the wall alone), `perf` with
 *   the drivers' environment, since it passes its own to the driver it counts, `pgrep` with gro's
 *   base environment
 * - `git` with git's local variables (`GIT_LOCAL_PASS_ENV`), and for the working-tree snapshot
 *   with `GIT_INDEX_FILE` naming a copy of the index in the cache; `tar` with gro's base
 *   environment
 * - `nice`, which runs `ionice` and cargo, with the build environment (`BUILD_ENV`,
 *   `BUILD_FIXED_ENV`)
 * - `HOME`, and no other variable
 *
 * The load average comes from `node:os`, a named exception to the host-module rule: no host
 * module reports it.
 *
 * @module
 */

import { loadavg } from 'node:os';

import { fs, type FsDir } from '@fuzdev/gro/fs.ts';
import { create_fs_paths, type FsPaths } from '@fuzdev/gro/fs_paths.ts';
import { GIT_LOCAL_PASS_ENV } from '@fuzdev/gro/git.ts';
import { process } from '@fuzdev/gro/process.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

import { BUILD_ENV } from '../git_snapshot.ts';
import {
	BUILD_FIXED_ENV,
	BUILD_PROGRAM,
	DRIVER_PASS_ENV,
	type PerfAbCache,
	type PerfAbDeps,
	run_perf_ab_command
} from '../perf_ab.ts';
import { open_checkout } from './checkout.ts';

/** The directory at `path`, absolute, made when missing, opened through the boundary that
 * follows a path's symlinks. */
const open_made_dir = async (fs_paths: FsPaths, path: string): Promise<FsDir> => {
	const opened = await fs_paths.open_path(path);
	if (opened.path === '') return opened.dir;
	await opened.dir.mkdir(opened.path, { recursive: true });
	return opened.dir.dir(opened.path);
};

const fs_paths = create_fs_paths(fs);

/** Every handle the run takes; a program missing from `PATH` throws here. */
const create_deps = async (): Promise<PerfAbDeps> => ({
	checkout: (await open_checkout()).readonly(),
	fs_paths,
	home: process.env.narrow(['HOME']).get('HOME'),
	open_cache: async (path): Promise<PerfAbCache> => {
		const cache = await open_made_dir(fs_paths, path);
		return {
			dir: cache,
			driver: (name) => spawn.program_in(cache, name, { pass_env: DRIVER_PASS_ENV })
		};
	},
	git: spawn.program('git', { pass_env: GIT_LOCAL_PASS_ENV }),
	git_with_index: (index) =>
		spawn.program('git', { pass_env: GIT_LOCAL_PASS_ENV, env: { GIT_INDEX_FILE: index } }),
	tar: spawn.program('tar'),
	build: spawn.program(BUILD_PROGRAM, { pass_env: BUILD_ENV, env: BUILD_FIXED_ENV }),
	perf: spawn.find('perf', { pass_env: DRIVER_PASS_ENV }),
	pgrep: spawn.find('pgrep'),
	pid: process.pid,
	load_1m: () => loadavg()[0]!
});

// exit 1 is a regression verdict, so a run that can't start is a harness error, exit 2
let code = 2;
try {
	code = await run_perf_ab_command(await create_deps(), process.args);
} catch (err) {
	console.error(`perf:ab: error: ${err instanceof Error ? err.message : String(err)}`);
}
process.set_exit_code(code);
