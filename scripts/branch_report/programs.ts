/**
 * Every program the branch report runs, and the environment each gets, made from the spawn
 * capability its root hands in: the build (`nice`), `git`, `tar`, each tool (`TOOL_NAMES`),
 * and, bound to the cache's handle, the programs each side builds there. Every child starts
 * from gro's base environment (`PATH`, `HOME`, `TMPDIR`, `TERM`, `LC_ALL=C`, `NO_COLOR`,
 * `FORCE_COLOR`) plus the names listed here, the same for both sides.
 *
 * @module
 */

import type { FsDir, FsReadDir } from '@fuzdev/gro/fs.ts';
import { GIT_LOCAL_PASS_ENV } from '@fuzdev/gro/git.ts';
import type { Spawn, SpawnProgram } from '@fuzdev/gro/spawn.ts';

import { BUILD_ENV } from '../git_snapshot.ts';
import type { ReportCache, ReportHost, ToolName } from './side.ts';
import { TOOL_NAMES } from './side.ts';

/** What a built program run directly gets beside the base environment: nothing. The CLI and
 * `tsv_debug` read their arguments; `perf`, `heaptrack` and `time` pass theirs on to the program
 * they run, so they get the same. */
export const SIDE_PROGRAM_PASS_ENV: ReadonlyArray<string> = [];

/** What each tool gets beside the base environment. `rustc` and `cargo` report a side's
 * toolchain, found as its build finds it; the `perf:ab` child (`node`) passes what its own
 * builds and `git` read. Every other tool gets the base environment alone. */
export const TOOL_PASS_ENV: Readonly<Partial<Record<ToolName, ReadonlyArray<string>>>> = {
	rustc: BUILD_ENV,
	cargo: BUILD_ENV,
	perf_ab: [...BUILD_ENV, ...GIT_LOCAL_PASS_ENV]
};

/** The program name a tool is found by on `PATH`. */
const TOOL_PROGRAM: Readonly<Partial<Record<ToolName, string>>> = { perf_ab: 'node' };

/** What the root hands in: spawn, and `/usr/bin` (where `time` is), `null` when it is absent. */
export interface ReportProgramsOptions {
	spawn: Spawn;
	usr_bin: FsReadDir | null;
}

/** The build, `git`, `tar` and every tool, each with its environment. A tool the machine lacks
 * is `null`, and the leg that needs it reports it missing.
 * @throws `SpawnError` - when `git` or `tar` is not installed, or a program found isn't runnable */
export function create_report_programs(
	options: ReportProgramsOptions
): Pick<ReportHost, 'build' | 'git' | 'tar' | 'tools'> {
	const { spawn, usr_bin } = options;
	const tools = {} as Record<ToolName, SpawnProgram | null>;
	for (const name of TOOL_NAMES) {
		const pass_env = TOOL_PASS_ENV[name] ?? [];
		tools[name] =
			name === 'time'
				? usr_bin === null
					? null
					: spawn.find_in(usr_bin, 'time', { pass_env })
				: spawn.find(TOOL_PROGRAM[name] ?? name, { pass_env });
	}
	return {
		build: (env) => spawn.program('nice', { pass_env: BUILD_ENV, env }),
		git: spawn.program('git', { pass_env: GIT_LOCAL_PASS_ENV }),
		tar: spawn.program('tar'),
		tools
	};
}

/** The cache at `dir`, the programs built there run through `spawn.program_in` beneath it. */
export function create_report_cache(spawn: Spawn, dir: FsDir): ReportCache {
	return {
		dir,
		side_program: (name) => spawn.program_in(dir, name, { pass_env: SIDE_PROGRAM_PASS_ENV })
	};
}
