/**
 * The branch report's programs over the real spawn host module: each tool found on a scratch
 * `PATH` of fake programs that print the names (never the values) of the variables they were
 * given, so each child's environment is read from the child itself; the build's; a missing
 * tool; and the programs a side builds, run only from beneath the cache's handle.
 *
 * gro's spawn walks `PATH` and reads the variables when a program is made, so each test sets
 * `process.env` first and makes the programs after, restoring it when done.
 */

import { deepStrictEqual, ok, strictEqual, throws } from 'node:assert';

import { fs, type FsDir } from '@fuzdev/gro/fs.ts';
import { is_spawn_error } from '@fuzdev/gro/spawn_error.ts';
import { spawn, type SpawnProgram } from '@fuzdev/gro/spawn.ts';
import { afterEach, test } from 'vitest';

import { BUILD_ENV } from '../git_snapshot.ts';
import {
	create_report_cache,
	create_report_programs,
	SIDE_PROGRAM_PASS_ENV,
	TOOL_PASS_ENV
} from './programs.ts';
import { TOOL_NAMES } from './side.ts';

const saved = { ...process.env };
afterEach(() => {
	for (const name of Object.keys(process.env)) if (!(name in saved)) delete process.env[name];
	Object.assign(process.env, saved);
});

/** A program that prints the sorted names of its environment, one per line. */
const ENV_NAMES_SCRIPT = '#!/bin/sh\nenv | cut -d= -f1 | sort\n';

/** A scratch `bin/` holding `names` as env-printing programs. */
const fake_bin = async (dir: FsDir, names: ReadonlyArray<string>): Promise<FsDir> => {
	await dir.mkdir('bin');
	const bin = dir.dir('bin');
	for (const name of names) {
		await bin.write(name, ENV_NAMES_SCRIPT);
		await bin.set_mode(name, 0o755);
	}
	return bin;
};

const env_names = async (program: SpawnProgram, args: ReadonlyArray<string> = []) => {
	const result = await program.out(args);
	strictEqual(result.kind === 'exited' && result.code, 0);
	// what the shell adds itself
	return result.stdout.split('\n').filter((line) => line !== '' && !SHELL_NAMES.has(line));
};

const SHELL_NAMES = new Set(['PWD', 'OLDPWD', 'SHLVL', '_']);

/** gro's base set, as this host has it. */
const BASE = ['HOME', 'LC_ALL', 'PATH', 'TERM', 'TMPDIR', 'NO_COLOR', 'FORCE_COLOR'];
const base_names = (): Array<string> =>
	BASE.filter((name) => name === 'LC_ALL' || process.env[name] !== undefined);

/** A caller environment no side should see: build flags and a stray secret. */
const set_caller_env = (bin: FsDir): void => {
	for (const name of Object.keys(process.env))
		if (name.startsWith('GIT_')) delete process.env[name];
	process.env.PATH = `${bin.path}:/usr/bin:/bin`;
	process.env.RUSTFLAGS = '-C target-cpu=native';
	process.env.CARGO_PROFILE_RELEASE_LTO = 'false';
	process.env.SOME_TOKEN = 'secret';
	process.env.CARGO_HOME = '/cargo/home';
	process.env.GIT_AUTHOR_NAME = 'someone';
};

test('every tool gets the base environment and only the names TOOL_PASS_ENV lists', async () => {
	await using dir = await fs.temp_dir('branch_report_programs-');
	const names = TOOL_NAMES.filter((n) => n !== 'perf_ab' && n !== 'time');
	const bin = await fake_bin(dir, [...names, 'node', 'git', 'tar', 'nice']);
	await using usr_bin = await fs.temp_dir('branch_report_usr_bin-');
	await fake_bin(usr_bin, ['time']);
	set_caller_env(bin);
	const programs = create_report_programs({ spawn, usr_bin: usr_bin.dir('bin') });

	for (const name of TOOL_NAMES) {
		const program = programs.tools[name];
		ok(program !== null, name);
		const expected = [
			...base_names(),
			...(TOOL_PASS_ENV[name] === undefined ? [] : ['CARGO_HOME'])
		];
		if (name === 'perf_ab') expected.push('GIT_AUTHOR_NAME');
		deepStrictEqual((await env_names(program)).sort(), expected.sort(), name);
	}
	// what each tool is found by
	strictEqual(programs.tools.perf_ab!.path, `${bin.path}/node`);
	strictEqual(programs.tools.time!.path, `${usr_bin.path}/bin/time`);
	deepStrictEqual(TOOL_PASS_ENV.rustc, BUILD_ENV);
	deepStrictEqual(
		(await env_names(programs.git)).sort(),
		[...base_names(), 'GIT_AUTHOR_NAME'].sort()
	);
	deepStrictEqual((await env_names(programs.tar)).sort(), base_names().sort());
});

test('the build gets the build environment and its own variables, never a caller flag', async () => {
	await using dir = await fs.temp_dir('branch_report_programs-');
	const bin = await fake_bin(dir, ['nice', 'git', 'tar']);
	set_caller_env(bin);
	const { build } = create_report_programs({ spawn, usr_bin: null });
	const names = await env_names(build({ CARGO_TARGET_DIR: '/t', TSV_DEPTH_SHAPES: '/s' }));
	const build_names = BUILD_ENV.filter((n) => process.env[n] !== undefined);
	deepStrictEqual(
		names.sort(),
		[...new Set([...base_names(), ...build_names, 'CARGO_TARGET_DIR', 'TSV_DEPTH_SHAPES'])].sort()
	);
	ok(!names.includes('RUSTFLAGS') && !names.includes('CARGO_PROFILE_RELEASE_LTO'));
	ok(!names.includes('SOME_TOKEN'));
});

test('a tool missing from PATH is null, and /usr/bin missing leaves time null', async () => {
	await using dir = await fs.temp_dir('branch_report_programs-');
	const bin = await fake_bin(dir, ['git', 'tar']);
	process.env.PATH = bin.path;
	const programs = create_report_programs({ spawn, usr_bin: null });
	for (const name of TOOL_NAMES) strictEqual(programs.tools[name], null, name);
});

test('a side program runs only from beneath the cache, once built, with the base environment', async () => {
	await using cache = await fs.temp_dir('branch_report_cache-');
	await using dir = await fs.temp_dir('branch_report_programs-');
	set_caller_env(await fake_bin(dir, []));
	const { side_program } = create_report_cache(spawn, cache);
	const name = `${'a'.repeat(40)}-base/target/corpus/tsv_debug`;
	// not built yet: nothing to pin
	throws(
		() => side_program(name),
		(err) => is_spawn_error(err, 'not_found')
	);
	await cache.mkdir(name.slice(0, name.lastIndexOf('/')), { recursive: true });
	await cache.write(name, ENV_NAMES_SCRIPT);
	await cache.set_mode(name, 0o755);
	const program = side_program(name);
	strictEqual(program.path, `${cache.path}/${name}`);
	deepStrictEqual(SIDE_PROGRAM_PASS_ENV, []);
	deepStrictEqual((await env_names(program)).sort(), base_names().sort());

	// a name that would climb out of the cache, or name a path outside it, is refused unread
	await dir.write('outside', ENV_NAMES_SCRIPT);
	await dir.set_mode('outside', 0o755);
	for (const climb of [
		'../outside',
		`${'a'.repeat(40)}-base/../../outside`,
		`${dir.path}/outside`
	]) {
		throws(
			() => side_program(climb),
			(err) => is_spawn_error(err, 'not_granted'),
			climb
		);
	}
	// and so is a link beneath it that leads out
	await cache.symlink('escape', `${dir.path}/outside`);
	throws(
		() => side_program('escape'),
		(err) => is_spawn_error(err, 'not_granted')
	);
});
