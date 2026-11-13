/**
 * `branch:report`'s root (`scripts/branch_report/branch_report.ts`). Its reach:
 *
 * - the checkout, found from this module's location, read-only: the working directory of `git`
 *   and of every tool, the source of each side's tree, and the probe scripts
 * - the corpus and its files, `--aa` reports and `--out`, each opened by its path, symlinks
 *   followed (`--out` made when missing)
 * - the build cache, `--cache DIR` or `$HOME/.cache/tsv/branch_report`, opened by its path (made
 *   when missing), symlinks followed, read and written beneath its handle: each side's tree and
 *   builds, the scratch files, the reports, and the `perf:ab` child's own cache
 * - each program a side builds there, run beneath that one handle (`spawn.program_in`), made
 *   once it is built, with gro's base environment alone (`SIDE_PROGRAM_PASS_ENV`)
 * - `nice` with the build environment (`BUILD_ENV` and a build's own variables), which runs
 *   cargo, wasm-pack and each side's own tasks; `git` with git's local variables; `tar`, and the
 *   tools on `PATH` (`time` at `/usr/bin/time`), each with the environment `TOOL_PASS_ENV` gives
 * - `HOME`, `CARGO_BUILD_JOBS` and the bench narrowing variables (`BENCH_NARROWING_ENV`), and no
 *   other variable
 *
 * The load average comes from `node:os`, a named exception to the host-module rule: no host
 * module reports it.
 *
 * @module
 */

import { loadavg } from 'node:os';

import { fs, type FsDir, type FsReadDir } from '@fuzdev/gro/fs.ts';
import { create_fs_paths, type FsPaths } from '@fuzdev/gro/fs_paths.ts';
import { process } from '@fuzdev/gro/process.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

import {
	type BranchReportDeps,
	run_branch_report_command
} from '../branch_report/branch_report.ts';
import { create_report_cache, create_report_programs } from '../branch_report/programs.ts';
import { BENCH_NARROWING_ENV } from '../branch_report/timing.ts';
import { open_checkout } from './checkout.ts';

/** The directory at `path`, absolute, made when missing, opened through the boundary that
 * follows a path's symlinks. */
const open_made_dir = async (fs_paths: FsPaths, path: string): Promise<FsDir> => {
	const opened = await fs_paths.open_path(path);
	if (opened.path === '') return opened.dir;
	await opened.dir.mkdir(opened.path, { recursive: true });
	return opened.dir.dir(opened.path);
};

/** `/usr/bin`, where the report's `time` is, or `null` when it can't be opened. */
const open_usr_bin = async (): Promise<FsReadDir | null> => {
	try {
		return (await fs.open_dir('/usr/bin')).readonly();
	} catch {
		return null;
	}
};

const fs_paths = create_fs_paths(fs);

/** Every handle the run takes; `git` or `tar` missing from `PATH` throws here. */
const create_deps = async (): Promise<BranchReportDeps> => {
	const env = process.env.narrow(['HOME', 'CARGO_BUILD_JOBS', ...BENCH_NARROWING_ENV]);
	const bench_env: Record<string, string> = {};
	for (const name of BENCH_NARROWING_ENV) {
		const value = env.get(name);
		if (value !== undefined) bench_env[name] = value;
	}
	return {
		checkout: (await open_checkout()).readonly(),
		fs_paths,
		home: env.get('HOME'),
		cargo_build_jobs: env.get('CARGO_BUILD_JOBS'),
		bench_env,
		open_cache: async (path) => create_report_cache(spawn, await open_made_dir(fs_paths, path)),
		open_made_dir: (path) => open_made_dir(fs_paths, path),
		...create_report_programs({ spawn, usr_bin: await open_usr_bin() }),
		pid: process.pid,
		load_1m: () => loadavg()[0]!
	};
};

// a run that can't start is a harness error, exit 2, as every failure but a deterministic diff
let code = 2;
try {
	code = await run_branch_report_command(await create_deps(), process.args);
} catch (err) {
	console.error(`branch:report: error: ${err instanceof Error ? err.message : String(err)}`);
}
process.set_exit_code(code);
