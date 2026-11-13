/**
 * `perf:ab` over scratch git repos and fake programs: the `git ls-tree -z` listing, the strip
 * entry-point and driver-API probes, the driver build's reuse rule and its copy out of the target dir, each
 * driver made a program through the cache once its binary is in place, the environment each
 * child gets, and a whole run over a fake build, fake drivers and a fake `perf`. No cargo runs.
 *
 * The git children here read the live environment when each program is made, so a suite that
 * runs under a git hook clears git's repository variables first (`clear_git_repo_env`), and
 * makes every program after that.
 */

import { deepStrictEqual, ok, rejects, strictEqual } from 'node:assert';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { fs, type FsDir, type FsReadDir } from '@fuzdev/gro/fs.ts';
import { create_fs_paths } from '@fuzdev/gro/fs_paths.ts';
import { GIT_LOCAL_PASS_ENV } from '@fuzdev/gro/git.ts';
import { is_spawn_error } from '@fuzdev/gro/spawn_error.ts';
import { spawn, type SpawnChild, type SpawnProgram, type SpawnResult } from '@fuzdev/gro/spawn.ts';
import { afterEach, test, vi } from 'vitest';

import { BUILD_ENV, extract_tree, HarnessError, list_tree } from './git_snapshot.ts';
import {
	build_args,
	BUILD_FIXED_ENV,
	build_side,
	DRIVER_APIS,
	DRIVER_PASS_ENV,
	type DriverBuild,
	driver_binary_name,
	driver_recipe_hash,
	ensure_driver,
	type PerfAbCache,
	type PerfAbDeps,
	run_perf_ab_command,
	type Side,
	tree_driver_api,
	tree_has_strip
} from './perf_ab.ts';
import {
	type AbRow,
	build_inputs_of,
	row_is_ambiguous,
	row_is_settleable
} from './perf_ab_verdict.ts';

const CHECKOUT = join(dirname(fileURLToPath(import.meta.url)), '..');

/** The variables that point git at a repository other than the one its working directory is in.
 * A git hook runs with them set to the outer repo, and every git program made here reads them
 * when it's made, so the scratch repo's `add -A` would write the outer index. */
const GIT_REPO_ENV = ['GIT_DIR', 'GIT_INDEX_FILE', 'GIT_WORK_TREE'];

const clear_git_repo_env = (): void => {
	for (const name of GIT_REPO_ENV) delete process.env[name];
};

/** `git`, made now, so it reads the environment as it is now. */
const make_git = (): SpawnProgram => spawn.program('git', { pass_env: GIT_LOCAL_PASS_ENV });

const run_git = async (cwd: FsReadDir, args: ReadonlyArray<string>): Promise<string> => {
	const result = await make_git().out(['-c', 'commit.gpgsign=false', ...args], { cwd });
	if (result.kind !== 'exited' || result.code !== 0) {
		throw new Error(`git ${args.join(' ')} failed:\n${result.stderr}`);
	}
	return result.stdout.trim();
};

const commit_all = async (repo: FsDir): Promise<void> => {
	await run_git(repo, ['add', '-A']);
	await run_git(repo, ['-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-qm', 'x']);
};

afterEach(() => {
	vi.restoreAllMocks();
});

/** Build a scratch repo holding a non-ASCII path and read it back through `list_tree`. */
const assert_lists_raw_paths = async (): Promise<void> => {
	clear_git_repo_env();
	await using dir = await fs.temp_dir('perf_ab_test-');
	await run_git(dir, ['init', '--quiet']);
	await dir.mkdir('crates/é', { recursive: true });
	await dir.mkdir('src');
	await dir.write('Cargo.toml', '[workspace]\n');
	await dir.write('crates/é/lib.rs', '');
	await dir.write('src/lib.rs', '');
	await run_git(dir, ['add', '-A']);
	const tree = await run_git(dir, ['write-tree']);
	// the default listing quotes the path, which is the shape that must not reach the reader
	const quoted = await run_git(dir, ['-c', 'core.quotePath=true', 'ls-tree', '-r', tree]);
	strictEqual(quoted.includes('"crates/\\303\\251/lib.rs"'), true);

	const listing = await list_tree(make_git(), dir, tree, ['Cargo.toml', 'crates', 'src']);
	const read = build_inputs_of(listing, ['Cargo.toml', 'crates', 'src'], ['src/driver.rs']);
	deepStrictEqual(read.names, ['Cargo.toml', 'crates', 'src']);
	deepStrictEqual(read.missing, []);
	strictEqual(read.key_text.includes('\tcrates/é/lib.rs'), true);
	strictEqual(read.key_text.includes('"'), false);
};

test('list_tree reads a non-ASCII path raw, never C-quoted', async () => {
	await assert_lists_raw_paths();
});

test('the scratch repo ignores a caller git environment that names another repo', async () => {
	clear_git_repo_env();
	await using decoy = await fs.temp_dir('perf_ab_test_decoy-');
	await run_git(decoy, ['init', '--quiet']);
	try {
		process.env.GIT_DIR = `${decoy.path}/.git`;
		process.env.GIT_INDEX_FILE = `${decoy.path}/.git/index`;
		process.env.GIT_WORK_TREE = decoy.path;
		await assert_lists_raw_paths();
		// a fresh repo has no index until something is added to it
		strictEqual(await decoy.exists('.git/index'), false);
	} finally {
		clear_git_repo_env();
	}
});

/** Commit `strip_rs` as a scratch repo's `crates/tsv_ts/src/strip/mod.rs` and ask whether the
 * tree has the strip driver's entry point. */
const has_strip_with = async (dir: FsDir, strip_rs: string): Promise<boolean> => {
	await dir.write('crates/tsv_ts/src/strip/mod.rs', strip_rs);
	await run_git(dir, ['add', '-A']);
	return await tree_has_strip(make_git(), dir, await run_git(dir, ['write-tree']));
};

test('the strip entry point is found in each of its spellings, and nothing else', async () => {
	clear_git_repo_env();
	await using dir = await fs.temp_dir('perf_ab_test-');
	await run_git(dir, ['init', '--quiet']);
	await dir.mkdir('crates/tsv_ts/src/strip', { recursive: true });
	const borrowed = "pub fn strip_in<'s>(\n    source: &'s str,\n) {}\n";
	strictEqual(await has_strip_with(dir, borrowed), true);
	strictEqual(await has_strip_with(dir, 'pub fn strip_in(\n    source: &str,\n) {}\n'), true);
	strictEqual(await has_strip_with(dir, 'pub fn strip_into(\n    source: &str,\n) {}\n'), true);
	strictEqual(await has_strip_with(dir, 'pub fn strip_intox(source: &str) {}\n'), false);
	strictEqual(await has_strip_with(dir, 'pub fn strip(source: &str) {}\n'), false);
	strictEqual(await has_strip_with(dir, 'pub fn strip_input(source: &str) {}\n'), false);
	const mentioned =
		'// pub fn strip_in(source: &str)\n/// see pub fn strip_in(\npub fn strip() {}\n';
	strictEqual(await has_strip_with(dir, mentioned), false);
});

/** A tsv_ts manifest that depends on `bumpalo`, as every tree older than the current one's does. */
const BUMPALO_MANIFEST = '[dependencies]\ntsv_lang.workspace = true\nbumpalo.workspace = true\n';

/** Commit `private_rs` as a scratch repo's `crates/tsv_ts/src/__private.rs` and `manifest` as its
 * `crates/tsv_ts/Cargo.toml`, and ask which entry-point generation the drivers build against for
 * the tree. */
const driver_api_with = async (
	dir: FsDir,
	private_rs: string,
	manifest = BUMPALO_MANIFEST
): Promise<string | null> => {
	await dir.write('crates/tsv_ts/src/__private.rs', private_rs);
	await dir.write('crates/tsv_ts/Cargo.toml', manifest);
	await run_git(dir, ['add', '-A']);
	return (await tree_driver_api(make_git(), dir, await run_git(dir, ['write-tree']))).cfg;
};

test('a tree whose tsv_ts takes no bumpalo is the current generation, then the warm handle, then no Bump in the TypeScript parse, then the warm parse alone, any other the oldest', async () => {
	clear_git_repo_env();
	await using dir = await fs.temp_dir('perf_ab_test-');
	await run_git(dir, ['init', '--quiet']);
	await dir.mkdir('crates/tsv_ts/src', { recursive: true });
	const body =
		'    folded.parse_with(|text| parse_with_goal_or_fallback_into(text, goal, stores))\n';
	const no_bump = `pub fn parse_folded_into<'arena>(\n    goal: Option<Goal>,\n) {\n${body}}\n`;
	const handle = "pub struct WarmStores {\n    parked: Option<AstStores<'static>>,\n}\n";
	const free = '[dependencies]\ntsv_lang.workspace = true\nsmallvec.workspace = true\n';
	strictEqual(await driver_api_with(dir, `${handle}${no_bump}`, free), null);
	// the manifest answers, whatever the strip's body spells
	const respelled = '    let mut walk = Walk::new(source, Mode::Strip, view);\n';
	strictEqual(await driver_api_with(dir, `${handle}${respelled}`, free), null);
	strictEqual(await driver_api_with(dir, handle, `# bumpalo.workspace = true\n${free}`), null);
	strictEqual(await driver_api_with(dir, handle, `${free}bumpalo_x.workspace = true\n`), null);
	strictEqual(await driver_api_with(dir, `${handle}${respelled}`), 'strip_bump');
	// a manifest with no line is no evidence of the current generation
	strictEqual(await driver_api_with(dir, handle, ''), 'strip_bump');
	strictEqual(
		await driver_api_with(dir, handle, `${free}bumpalo = { workspace = true }\n`),
		'strip_bump'
	);
	strictEqual(await driver_api_with(dir, `${handle}${no_bump}`), 'strip_bump');
	strictEqual(await driver_api_with(dir, handle), 'strip_bump');
	// nor is a tree without the manifest, and a tree git can't read fails rather than answering
	await dir.remove('crates/tsv_ts/Cargo.toml');
	await run_git(dir, ['add', '-A']);
	const unmanifested = await run_git(dir, ['write-tree']);
	strictEqual((await tree_driver_api(make_git(), dir, unmanifested)).cfg, 'strip_bump');
	await rejects(tree_driver_api(make_git(), dir, '0'.repeat(40)), HarnessError);
	const named = `// pub struct WarmStores {\n/// pub struct WarmStores {\npub struct WarmStoresX {}\n${no_bump}`;
	strictEqual(await driver_api_with(dir, named), 'no_warm_stores');
	strictEqual(await driver_api_with(dir, no_bump), 'no_warm_stores');
	const lifetime_free = `pub fn parse_folded_into(\n    goal: Option<Goal>,\n) {\n${body}}\n`;
	strictEqual(await driver_api_with(dir, lifetime_free), 'no_warm_stores');
	const through_bump =
		"pub fn parse_folded_into<'arena>(\n    arena: &'arena Bump,\n) {\n    folded.parse_with(|text| parse_with_goal_or_fallback_into(text, goal, arena, stores))\n}\n";
	strictEqual(await driver_api_with(dir, through_bump), 'ts_parse_bump');
	const warm = "pub fn parse_folded_into<'arena>(\n    folded: &FoldedSource<'_>,\n) {}\n";
	strictEqual(await driver_api_with(dir, warm), 'ts_parse_bump');
	const commented = `pub fn parse_folded_into(\n) {}\n// ${body.trimStart()}    /// ${body.trimStart()}`;
	strictEqual(await driver_api_with(dir, commented), 'ts_parse_bump');
	const bump_only = "pub fn parse_folded<'arena>(\n    arena: &'arena Bump,\n) {}\n";
	strictEqual(await driver_api_with(dir, bump_only), 'no_stores');
	const lookalike = 'pub fn parse_folded_into_arena(source: &str) {}\n';
	strictEqual(await driver_api_with(dir, lookalike), 'no_stores');
	const mentioned =
		"// pub fn parse_folded_into<'arena>(\n    /// pub fn parse_folded_into(\npub fn parse_folded() {}\n";
	strictEqual(await driver_api_with(dir, mentioned), 'no_stores');
});

test('the workspace declares every older generation the drivers have an arm for', () => {
	const manifest = readFileSync(join(CHECKOUT, 'Cargo.toml'), 'utf8');
	const values = DRIVER_APIS.flatMap((api) => (api.cfg === null ? [] : [`"${api.cfg}"`]));
	strictEqual(manifest.includes(`'cfg(perf_ab_api, values(${values.join(', ')}))'`), true);
});

test('an older generation differs from the current build only by its cfg', () => {
	const [current, ...older] = DRIVER_APIS;
	strictEqual(current!.cfg, null);
	strictEqual(older.at(-1)!.probe, null);
	const plain = build_args('perf_ab', current!);
	const split = plain.indexOf('--');
	deepStrictEqual(plain.slice(split), [
		'--',
		'--check-cfg',
		`cfg(perf_ab_api, values(${older.map((api) => `"${api.cfg}"`).join(', ')}))`
	]);
	for (const api of older) {
		deepStrictEqual(build_args('perf_ab', api), [...plain, '--cfg', `perf_ab_api="${api.cfg}"`]);
	}
});

test('extract_tree extracts the named paths of a tree through git archive and tar', async () => {
	clear_git_repo_env();
	await using repo = await fs.temp_dir('perf_ab_test_repo-');
	await using out = await fs.temp_dir('perf_ab_test_out-');
	await run_git(repo, ['init', '--quiet']);
	await repo.mkdir('keep/deep', { recursive: true });
	await repo.mkdir('skip');
	await repo.write('keep/deep/a.rs', 'kept\n');
	await repo.write('skip/b.rs', 'skipped\n');
	await commit_all(repo);
	const tree = await run_git(repo, ['rev-parse', 'HEAD^{tree}']);
	const deps = { git: make_git(), tar: spawn.program('tar'), checkout: repo, pid: 7 };
	await extract_tree(deps, tree, ['keep'], out, 'src');
	strictEqual(await out.read_text('src/keep/deep/a.rs'), 'kept\n');
	strictEqual(await out.exists('src/skip'), false);
	strictEqual(await out.exists('src.partial.7'), false);
	// a tree git can't read fails the extraction and leaves no partial directory behind
	await rejects(
		extract_tree(deps, '0'.repeat(40), [], out, 'other'),
		(err) => err instanceof HarnessError && /extracting tree 0+ failed/.test(err.message)
	);
	strictEqual(await out.exists('other.partial.7'), false);
	strictEqual(await out.exists('other'), false);
});

// -- the driver build, over fake programs ---------------------------------------------------------

const KEY = 'a'.repeat(64);
const SIDE: Side = {
	label: 'HEAD (abcdef012)',
	commit: 'abcdef0123456789',
	tree: 'b'.repeat(40),
	key: KEY,
	inputs: ['Cargo.toml', 'crates', 'src'],
	has_strip: true,
	api: DRIVER_APIS[0]!
};
const DRIVER_HASH_DIGITS = { format: '1', strip: '2', one_shot: '3' } as const;
const driver_build = (kind: 'format' | 'strip' | 'one_shot'): DriverBuild => ({
	kind,
	source: new TextEncoder().encode(`// the ${kind} driver\n`),
	hash: DRIVER_HASH_DIGITS[kind].repeat(64)
});
const DRIVERS = { format: driver_build('format'), strip: driver_build('strip'), one_shot: null };
const EXAMPLES = {
	format: 'perf_ab',
	strip: 'perf_ab_strip',
	one_shot: 'perf_ab_one_shot'
} as const;
/** The name of `kind`'s binary for `SIDE`, whose recipe is its generation's. */
const binary_name = async (kind: 'format' | 'strip' | 'one_shot'): Promise<string> =>
	driver_binary_name(KEY, driver_build(kind), await driver_recipe_hash(EXAMPLES[kind], SIDE.api));

const unused = (name: string) => (): never => {
	throw new Error(`${name} must not run`);
};

const refusing_program = (name: string): SpawnProgram => ({
	name,
	path: `/fake/${name}`,
	run: unused(`${name}.run`),
	out: unused(`${name}.out`),
	out_bytes: unused(`${name}.out_bytes`),
	start: unused(`${name}.start`)
});

/** Lines as one stream's chunks, the way a child's pipe yields them. */
async function* chunks_of(lines: ReadonlyArray<string>): AsyncGenerator<Uint8Array> {
	for (const line of lines) yield new TextEncoder().encode(`${line}\n`);
}

interface BuildCall {
	args: ReadonlyArray<string>;
	cwd: string | undefined;
}

/** A build program that plays cargo: `play` writes the artifact and returns cargo's stdout lines. */
const fake_build = (
	play: (cwd: FsReadDir, args: ReadonlyArray<string>) => Promise<Array<string>>,
	ending: number | SpawnResult = 0
): { program: SpawnProgram; calls: Array<BuildCall> } => {
	const calls: Array<BuildCall> = [];
	const program: SpawnProgram = {
		...refusing_program('nice'),
		start: async (args, options) => {
			calls.push({ args, cwd: options?.cwd?.path });
			const lines = await play(options!.cwd!, args);
			const child: SpawnChild = {
				pid: 1,
				closed: Promise.resolve(
					typeof ending === 'number' ? { kind: 'exited', code: ending } : ending
				),
				stdout: chunks_of(lines),
				stderr: null,
				stdin: null,
				kill: () => false,
				[Symbol.asyncDispose]: async () => undefined
			};
			return child;
		}
	};
	return { program, calls };
};

const artifact_line = (example: string, executable: string): string =>
	JSON.stringify({
		reason: 'compiler-artifact',
		target: { kind: ['example'], name: example },
		executable
	});

/** A cache with the side's snapshot already extracted, so a build needs no git or tar. */
const cache_with_snapshot = async (): Promise<FsDir & AsyncDisposable> => {
	const cache = await fs.temp_dir('perf_ab_test_cache-');
	await cache.mkdir(`${KEY}/src`, { recursive: true });
	return cache;
};

const ensure_deps = (build: SpawnProgram) => ({
	build,
	git: refusing_program('git'),
	tar: refusing_program('tar'),
	checkout: fs.project,
	pid: 99
});

test('a driver whose content-addressed binary exists is reused, never rebuilt', async () => {
	vi.spyOn(console, 'error').mockImplementation(() => undefined);
	await using cache = await fs.temp_dir('perf_ab_test_cache-');
	const name = await binary_name('format');
	const recipe = await driver_recipe_hash('perf_ab', SIDE.api);
	strictEqual(name, `${KEY}/perf_ab-${'1'.repeat(16)}-${recipe.slice(0, 16)}`);
	await cache.mkdir(KEY);
	await cache.write(name, 'built before');
	// no snapshot, no git, no tar, no build: any of them would throw
	const found = await ensure_driver(
		ensure_deps(refusing_program('nice')),
		cache,
		SIDE,
		DRIVERS.format
	);
	strictEqual(found, name);
	strictEqual(await cache.read_text(name), 'built before');
});

test('a driver is built in its snapshot with the overlay, then copied out executable', async () => {
	vi.spyOn(console, 'error').mockImplementation(() => undefined);
	await using cache = await cache_with_snapshot();
	const build = fake_build(async (cwd, args) => {
		const example = args[args.indexOf('--example') + 1]!;
		await cache.mkdir(`${KEY}/target/release/examples`, { recursive: true });
		await cache.write(`${KEY}/target/release/examples/${example}-0123`, `${example} binary`);
		// cargo reports the path through the relative target dir it was given
		return [
			'not json',
			artifact_line('other', '/elsewhere/other'),
			artifact_line(example, `${cwd.path}/../target/release/examples/${example}-0123`)
		];
	});
	const name = await ensure_driver(ensure_deps(build.program), cache, SIDE, DRIVERS.strip);
	deepStrictEqual(build.calls, [
		{ args: build_args('perf_ab_strip', SIDE.api), cwd: `${cache.path}/${KEY}/src` }
	]);
	strictEqual(name, await binary_name('strip'));
	strictEqual(await cache.read_text(name), 'perf_ab_strip binary');
	strictEqual((await cache.stat(name))!.mode & 0o777, 0o755);
	strictEqual(
		await cache.read_text(`${KEY}/src/crates/tsv_debug/examples/perf_ab_strip.rs`),
		'// the strip driver\n'
	);
	strictEqual(await cache.exists(`${name}.partial.99`), false);
});

test('a build that reports an executable outside the cache is refused, nothing copied', async () => {
	vi.spyOn(console, 'error').mockImplementation(() => undefined);
	await using cache = await cache_with_snapshot();
	const build = fake_build(async () => [artifact_line('perf_ab', '/usr/bin/true')]);
	await rejects(
		ensure_driver(ensure_deps(build.program), cache, SIDE, DRIVERS.format),
		(err) => err instanceof HarnessError && /\/usr\/bin\/true is not beneath/.test(err.message)
	);
	strictEqual(await cache.exists(await binary_name('format')), false);
});

test('a failed build, or one that reports no executable, is a harness error', async () => {
	vi.spyOn(console, 'error').mockImplementation(() => undefined);
	await using cache = await cache_with_snapshot();
	await rejects(
		ensure_driver(
			ensure_deps(fake_build(async () => [], 101).program),
			cache,
			SIDE,
			DRIVERS.format
		),
		(err) =>
			err instanceof HarnessError &&
			err.message ===
				`building the perf_ab driver for ${SIDE.label} failed (cargo exited 101); cargo's output ` +
					"above says why. One possible cause: the driver compiles against each tree's " +
					"language-crate entry points, so a tree older than those entry points can't build it."
	);
	await rejects(
		ensure_driver(ensure_deps(fake_build(async () => []).program), cache, SIDE, DRIVERS.format),
		(err) =>
			err instanceof HarnessError &&
			err.message ===
				`building the perf_ab driver for ${SIDE.label} failed (no executable reported): cargo ` +
					"exited 0 without naming the perf_ab example's executable in its JSON messages."
	);
	// a killed cargo said nothing, so the message offers no cause
	await rejects(
		ensure_driver(
			ensure_deps(fake_build(async () => [], { kind: 'signaled', signal: 'SIGKILL' }).program),
			cache,
			SIDE,
			DRIVERS.format
		),
		(err) =>
			err instanceof HarnessError &&
			err.message ===
				`building the perf_ab driver for ${SIDE.label} failed (cargo was killed by SIGKILL).`
	);
});

test('each driver is made a program through the cache, by its name beneath it, once built', async () => {
	vi.spyOn(console, 'error').mockImplementation(() => undefined);
	await using dir = await cache_with_snapshot();
	const build = fake_build(async (cwd, args) => {
		const example = args[args.indexOf('--example') + 1]!;
		await dir.mkdir(`${KEY}/target/release/examples`, { recursive: true });
		await dir.write(`${KEY}/target/release/examples/${example}`, example);
		return [artifact_line(example, `${cwd.path}/../target/release/examples/${example}`)];
	});
	const made: Array<{ name: string; built: boolean }> = [];
	const cache: PerfAbCache = {
		dir,
		driver: (name) => {
			made.push({ name, built: dir.exists_sync(name) });
			return refusing_program(name);
		}
	};
	const binaries = await build_side(ensure_deps(build.program), cache, SIDE, DRIVERS, true);
	const format = await binary_name('format');
	const strip = await binary_name('strip');
	deepStrictEqual(made, [
		{ name: format, built: true },
		{ name: strip, built: true }
	]);
	deepStrictEqual(
		[binaries.format?.name, binaries.format?.program.name, binaries.strip?.name],
		[format, format, strip]
	);
	// a side without the strip builds and makes the format driver alone
	made.length = 0;
	const without = await build_side(ensure_deps(build.program), cache, SIDE, DRIVERS, false);
	deepStrictEqual(
		[made.map((m) => m.name), without.strip, without.one_shot],
		[[format], null, null]
	);
	// the one-shot driver is built only when the run reads it
	made.length = 0;
	const one_shot_drivers = { ...DRIVERS, one_shot: driver_build('one_shot') };
	const with_one_shot = await build_side(
		ensure_deps(build.program),
		cache,
		SIDE,
		one_shot_drivers,
		true
	);
	const one_shot = await binary_name('one_shot');
	deepStrictEqual(
		[made.map((m) => m.name), with_one_shot.one_shot?.name],
		[[format, strip, one_shot], one_shot]
	);
});

test('a driver name that climbs out of the cache, or is absolute, is refused', async () => {
	await using cache = await fs.temp_dir('perf_ab_test_cache-');
	for (const name of ['../outside', `${KEY}/../../outside`, '/usr/bin/true']) {
		let thrown: unknown = null;
		try {
			spawn.program_in(cache, name, { pass_env: DRIVER_PASS_ENV });
		} catch (err) {
			thrown = err;
		}
		ok(is_spawn_error(thrown, 'not_granted'), name);
	}
});

// -- the environment each child gets ---------------------------------------------------------

/** The names in the environment a program made with `options` gets, read by a script that
 * prints them (the values stay in the child). */
const child_env_names = async (
	options: Parameters<typeof spawn.program_in>[2]
): Promise<Array<string>> => {
	await using dir = await fs.temp_dir('perf_ab_test_env-');
	await dir.write('names.sh', '#!/bin/sh\nenv | sed "s/=.*//" | sort\n', { mode: 0o755 });
	const result = await spawn.program_in(dir, 'names.sh', options).out([]);
	strictEqual(result.kind === 'exited' && result.code, 0);
	return result.stdout
		.split('\n')
		.filter((name) => name !== '' && name !== 'PWD' && name !== 'SHLVL' && name !== '_');
};

/** gro's base environment, as far as this host has it. */
const BASE_NAMES = ['PATH', 'HOME', 'TMPDIR', 'TERM', 'LC_ALL', 'NO_COLOR', 'FORCE_COLOR'];

/** Two names the tests below stub, as the host had them before any test ran. */
const HOST_STUBBED = { RUSTFLAGS: process.env.RUSTFLAGS, UNRELATED: process.env.UNRELATED };

/** Stubs each of `vars` in the host for this test (`vitest.config.ts` restores them after it);
 * `undefined` clears one. */
const stub_env = (vars: Record<string, string | undefined>): void => {
	for (const [name, value] of Object.entries(vars)) vi.stubEnv(name, value);
};

test('a build gets the base environment, the kept toolchain variables and its own', async () => {
	// a toolchain variable the pass list names that the host sets (`RUSTUP_HOME`,
	// `CARGO_BUILD_JOBS`) would pass too, so it's cleared and the caller environment is the
	// same on every machine
	for (const name of BUILD_ENV) if (!BASE_NAMES.includes(name)) vi.stubEnv(name, undefined);
	stub_env({
		RUSTFLAGS: '-C target-cpu=native',
		CARGO_PROFILE_RELEASE_LTO: 'off',
		RUSTUP_TOOLCHAIN: 'nightly',
		CARGO_HOME: '/tmp/perf_ab_test_cargo',
		CARGO_TARGET_DIR: '/tmp/elsewhere',
		UNRELATED: '1'
	});
	const names = await child_env_names({ pass_env: BUILD_ENV, env: BUILD_FIXED_ENV });
	deepStrictEqual(
		names.filter((name) => !BASE_NAMES.includes(name)).sort(),
		['CARGO_HOME', 'CARGO_TARGET_DIR'].sort()
	);
	await using dir = await fs.temp_dir('perf_ab_test_env-');
	await dir.write('value.sh', '#!/bin/sh\nprintf %s "$CARGO_TARGET_DIR"\n', { mode: 0o755 });
	const value = await spawn
		.program_in(dir, 'value.sh', { pass_env: BUILD_ENV, env: BUILD_FIXED_ENV })
		.out([]);
	strictEqual(value.stdout, '../target');
});

test('a driver, and perf around it, gets the base environment alone', async () => {
	stub_env({ RUSTFLAGS: '-C target-cpu=native', CARGO_HOME: '/x', UNRELATED: '1' });
	const names = await child_env_names({ pass_env: DRIVER_PASS_ENV });
	deepStrictEqual(
		names.filter((name) => !BASE_NAMES.includes(name)),
		[]
	);
	ok(names.includes('LC_ALL'));
});

test('what the tests above stubbed in the host is restored after each', () => {
	// tests in a file run in order, so this one runs after the stubs; `vitest.config.ts`'s
	// `unstubEnvs` is what restores them
	deepStrictEqual(
		{ RUSTFLAGS: process.env.RUSTFLAGS, UNRELATED: process.env.UNRELATED },
		HOST_STUBBED
	);
});

// -- a whole run over a fake build, fake drivers and a fake perf -------------------------------

/** The driver a fake build "compiles": it logs its `$0` and arguments, and prints the JSON line
 * a real driver prints, the same on every run, so the two sides tie. */
const driver_script = (log: string): string => `#!/bin/sh
printf '%s %s\\n' "$0" "$*" >> '${log}'
iterations=0; previous=''
for arg in "$@"; do [ "$previous" = --iterations ] && iterations="$arg"; previous="$arg"; done
echo "{\\"files\\":2,\\"bytes\\":100,\\"errors\\":0,\\"iterations\\":$iterations,\\"parse_us\\":100,\\"format_us\\":200,\\"strip_us\\":50,\\"output_bytes\\":64}"
`;

/** A build that plays cargo: it logs its arguments and the variables it was given, and
 * "compiles" the example as `driver_script`. */
const build_script = (log: string, driver: string): string => `#!/bin/sh
printf '%s|%s|%s\\n' "$*" "\${CARGO_TARGET_DIR-unset}" "\${RUSTFLAGS-unset}" >> '${log}'
example=''; previous=''
for arg in "$@"; do [ "$previous" = --example ] && example="$arg"; previous="$arg"; done
mkdir -p "$CARGO_TARGET_DIR/release/examples"
cp '${driver}' "$CARGO_TARGET_DIR/release/examples/$example-0123"
chmod +x "$CARGO_TARGET_DIR/release/examples/$example-0123"
echo "{\\"reason\\":\\"compiler-artifact\\",\\"target\\":{\\"kind\\":[\\"example\\"],\\"name\\":\\"$example\\"},\\"executable\\":\\"$(pwd)/$CARGO_TARGET_DIR/release/examples/$example-0123\\"}"
`;

/** A \`perf stat -x, -e EVENTS [-o FILE] -- PROGRAM ARGS\` that logs its arguments, runs the
 * program, and counts by mode: a full run more than a parse-only run more than a zero-iteration
 * one, as a real count would. */
const perf_script = (log: string): string => `#!/bin/sh
printf 'perf %s\\n' "$*" >> '${log}'
out=''
while [ "$1" != -- ]; do
	[ "$1" = -o ] && { shift; out="$1"; }
	shift
done
shift
case " $* " in
	*" --iterations 0 "*) n=1000 ;;
	*" --parse-only "*) n=2000 ;;
	*) n=5000 ;;
esac
"$@"
code=$?
csv="$n,,instructions:u,1,100.00,,
$((n * 2)),,cycles:u,1,100.00,,"
if [ -n "$out" ]; then printf '%s\\n' "$csv" > "$out"; else printf '%s\\n' "$csv" >&2; fi
exit $code
`;

/** A scratch tsv workspace: the build inputs a key needs, the strip's entry point, the drivers. */
const make_workspace = async (): Promise<FsDir & AsyncDisposable> => {
	const repo = await fs.temp_dir('perf_ab_test_workspace-');
	await run_git(repo, ['init', '--quiet']);
	await repo.mkdir('crates/tsv_ts/src', { recursive: true });
	await repo.mkdir('crates/tsv_debug/examples', { recursive: true });
	await repo.mkdir('src');
	await repo.write('Cargo.toml', '[workspace]\n');
	await repo.write('src/lib.rs', '');
	await repo.write('crates/tsv_ts/src/lib.rs', 'pub fn strip_in(source: &str) {}\n');
	await repo.write('crates/tsv_debug/examples/perf_ab.rs', 'fn main() {}\n');
	await repo.write('crates/tsv_debug/examples/perf_ab_strip.rs', 'fn main() {}\n');
	await commit_all(repo);
	return repo;
};

interface FakeRun {
	code: number;
	out: string;
	err: string;
	log: Array<string>;
}

const run_over_fakes = async (
	workspace: FsDir,
	tools: FsDir,
	cache_dir: FsDir,
	args: ReadonlyArray<string>,
	with_perf: boolean
): Promise<FakeRun> => {
	const out: Array<string> = [];
	const err: Array<string> = [];
	vi.spyOn(console, 'log').mockImplementation((line: unknown) => void out.push(String(line)));
	vi.spyOn(console, 'error').mockImplementation((line: unknown) => void err.push(String(line)));
	const deps: PerfAbDeps = {
		checkout: workspace.readonly(),
		fs_paths: create_fs_paths(fs),
		// a run requires `HOME` even when `--cache` names the cache
		home: '/nonexistent/home',
		open_cache: async (path) => {
			strictEqual(path, cache_dir.path);
			return {
				dir: cache_dir,
				driver: (name) => spawn.program_in(cache_dir, name, { pass_env: DRIVER_PASS_ENV })
			};
		},
		git: make_git(),
		git_with_index: (index) =>
			spawn.program('git', { pass_env: GIT_LOCAL_PASS_ENV, env: { GIT_INDEX_FILE: index } }),
		tar: spawn.program('tar'),
		build: spawn.program_in(tools, 'build.sh', { pass_env: BUILD_ENV, env: BUILD_FIXED_ENV }),
		perf: with_perf ? spawn.program_in(tools, 'perf', { pass_env: DRIVER_PASS_ENV }) : null,
		pgrep: null,
		pid: 4242,
		load_1m: () => 0
	};
	const code = await run_perf_ab_command(deps, [
		...args,
		'--corpus',
		tools.path,
		'--cache',
		cache_dir.path,
		'--rounds',
		'2',
		'--iterations',
		'3'
	]);
	vi.restoreAllMocks();
	const log = (await tools.read_text_if_exists('log')) ?? '';
	await tools.remove('log', { missing_ok: true });
	return { code, out: out.join('\n'), err: err.join('\n'), log: log.split('\n').filter(Boolean) };
};

const make_tools = async (): Promise<FsDir & AsyncDisposable> => {
	const tools = await fs.temp_dir('perf_ab_test_tools-');
	const log = `${tools.path}/log`;
	await tools.write('driver.sh', driver_script(log), { mode: 0o755 });
	await tools.write('build.sh', build_script(log, `${tools.path}/driver.sh`), { mode: 0o755 });
	await tools.write('perf', perf_script(log), { mode: 0o755 });
	return tools;
};

test('a run builds each driver once, runs it beneath the cache under perf, and passes a tie', async () => {
	clear_git_repo_env();
	await using workspace = await make_workspace();
	await using tools = await make_tools();
	await using cache = await fs.temp_dir('perf_ab_test_cache-');

	const first = await run_over_fakes(workspace, tools, cache, ['HEAD', 'HEAD'], true);
	strictEqual(first.code, 0, first.err);
	ok(first.out.endsWith('perf:ab: PASS'), first.out);
	const builds = first.log.filter((line) => line.startsWith('-n 19 ionice'));
	deepStrictEqual(
		builds.map((line) => line.split('|').slice(1)),
		[
			['../target', 'unset'],
			['../target', 'unset']
		]
	);
	// the scratch workspace has no warm parse, so each side is of the oldest generation
	const oldest = DRIVER_APIS.at(-1)!;
	deepStrictEqual(
		builds.map((line) => line.split('|')[0]),
		[build_args('perf_ab', oldest).join(' '), build_args('perf_ab_strip', oldest).join(' ')]
	);
	// every counted run hands perf the driver's real path beneath the cache, after `--`
	const counted = first.log.filter((line) =>
		line.startsWith('perf stat -x, -e instructions:u,cycles:u')
	);
	ok(counted.length > 0);
	for (const line of counted) {
		const [, after] = line.split(' -- ');
		ok(after!.startsWith(`${cache.path}/`), line);
		ok(line.includes(` -o ${cache.path}/perf_stat.4242.csv -- `), line);
	}
	// and each driver ran as the path it was found at beneath the cache, `$0` included
	for (const line of first.log.filter((l) => l.includes('/perf_ab-') && !l.startsWith('perf '))) {
		ok(line.startsWith(`${cache.path}/`), line);
	}
	strictEqual(await cache.exists('perf_stat.4242.csv'), false);

	// a rerun reuses both binaries: nothing is built
	const second = await run_over_fakes(workspace, tools, cache, ['HEAD', 'HEAD'], true);
	strictEqual(second.code, 0, second.err);
	deepStrictEqual(
		second.log.filter((line) => line.startsWith('-n 19 ionice')),
		[]
	);
	ok(/reusing .*\/perf_ab-/.test(second.err), second.err);
});

test('a run without perf grades the wall alone, and the working tree is the default candidate', async () => {
	clear_git_repo_env();
	await using workspace = await make_workspace();
	await using tools = await make_tools();
	await using cache = await fs.temp_dir('perf_ab_test_cache-');
	await workspace.write('notes.md', 'outside the build inputs\n');

	const run = await run_over_fakes(workspace, tools, cache, ['HEAD'], false);
	strictEqual(run.code, 0, run.err);
	ok(run.err.includes('NO INSTRUCTION COUNTS — `perf` is not installed'), run.err);
	ok(run.out.includes('cand: worktree (HEAD '), run.out);
	ok(run.out.includes(' + changes)'), run.out);
	ok(run.out.includes('NO INSTRUCTION COUNTS THIS RUN: wall alone'), run.out);
	deepStrictEqual(
		run.log.filter((line) => line.startsWith('perf ')),
		[]
	);
	// the scratch index is gone, and the checkout's own index never moved
	strictEqual(await cache.exists('index.4242'), false);
	strictEqual(await run_git(workspace, ['status', '--porcelain']), '?? notes.md');
});

test('usage errors exit 2 with the usage, --help exits 0, before anything is opened', async () => {
	// the drivers' sources, and nothing else
	await using checkout = await fs.temp_dir('perf_ab_test_checkout-');
	await checkout.mkdir('crates/tsv_debug/examples', { recursive: true });
	await checkout.write('crates/tsv_debug/examples/perf_ab.rs', 'fn main() {}\n');
	await checkout.write('crates/tsv_debug/examples/perf_ab_strip.rs', 'fn main() {}\n');
	const refusing: PerfAbDeps = {
		checkout,
		fs_paths: create_fs_paths(fs),
		home: undefined,
		open_cache: unused('open_cache'),
		git: refusing_program('git'),
		git_with_index: unused('git_with_index'),
		tar: refusing_program('tar'),
		build: refusing_program('nice'),
		perf: null,
		pgrep: null,
		pid: 1,
		load_1m: unused('load_1m')
	};
	const capture = async (args: ReadonlyArray<string>) => {
		const out: Array<string> = [];
		const err: Array<string> = [];
		vi.spyOn(console, 'log').mockImplementation((line: unknown) => void out.push(String(line)));
		vi.spyOn(console, 'error').mockImplementation((line: unknown) => void err.push(String(line)));
		const code = await run_perf_ab_command(refusing, args);
		vi.restoreAllMocks();
		return { code, out, err };
	};
	const usage = await capture([]);
	strictEqual(usage.code, 2);
	strictEqual(usage.err[0], 'perf:ab: error: expected <base-ref> and an optional <cand-ref>');
	ok(usage.err[1]!.startsWith('usage: npm run perf:ab -- <base-ref>'));
	const help = await capture(['--help']);
	deepStrictEqual([help.code, help.err], [0, []]);
	ok(help.out[0]!.startsWith('usage: npm run perf:ab -- <base-ref>'));
	const odd = await capture(['HEAD', '--rounds', '3']);
	deepStrictEqual(
		[odd.code, odd.err],
		[2, ['perf:ab: error: --rounds 3: takes an even count, so each side goes first equally often']]
	);
	const home = await capture(['HEAD', '--corpus', '.']);
	deepStrictEqual([home.code, home.err], [2, ['perf:ab: error: HOME is not set']]);
});

/** A build that stamps each driver with the snapshot it was built in, so the two sides' binaries
 * differ in bytes as two revisions' builds do. */
const stamping_build_script = (driver: string): string => `#!/bin/sh
example=''; previous=''
for arg in "$@"; do [ "$previous" = --example ] && example="$arg"; previous="$arg"; done
mkdir -p "$CARGO_TARGET_DIR/release/examples"
{ cat '${driver}'; printf '# built in %s\\n' "$(pwd)"; } > "$CARGO_TARGET_DIR/release/examples/$example-0123"
chmod +x "$CARGO_TARGET_DIR/release/examples/$example-0123"
echo "{\\"reason\\":\\"compiler-artifact\\",\\"target\\":{\\"kind\\":[\\"example\\"],\\"name\\":\\"$example\\"},\\"executable\\":\\"$(pwd)/$CARGO_TARGET_DIR/release/examples/$example-0123\\"}"
`;

test('each side runs its own build, base first in even rounds and the candidate first in odd ones', async () => {
	clear_git_repo_env();
	await using workspace = await make_workspace();
	await workspace.write('src/lib.rs', '// the candidate\n');
	await commit_all(workspace);
	await using tools = await make_tools();
	await tools.write('build.sh', stamping_build_script(`${tools.path}/driver.sh`), { mode: 0o755 });
	await using cache = await fs.temp_dir('perf_ab_test_cache-');

	const run = await run_over_fakes(workspace, tools, cache, ['HEAD~1', 'HEAD'], true);
	strictEqual(run.code, 0, run.err);
	const keys = (await Array.fromAsync(cache.list()))
		.filter((entry) => entry.kind === 'directory')
		.map((entry) => entry.name);
	strictEqual(keys.length, 2);
	// each side's key, by the snapshot its base-side label names
	const sha_of = (side: 'base' | 'cand'): string =>
		/binary sha256 ([0-9a-f]{64})/.exec(
			run.out.split('\n').find((l) => l.startsWith(`${side}:`))!
		)![1]!;
	ok(sha_of('base') !== sha_of('cand'), run.out);
	// the full TS runs, in order, by the snapshot their binary sits in
	const full_ts = run.log
		.filter((line) => !line.startsWith('perf ') && / --lang ts --iterations 3$/.test(line))
		.map((line) => line.slice(cache.path.length + 1, cache.path.length + 65));
	strictEqual(full_ts.length, 4, run.log.join('\n'));
	const [base_key, cand_key] = [full_ts[0]!, full_ts[1]!];
	ok(base_key !== cand_key);
	deepStrictEqual(full_ts, [base_key, cand_key, cand_key, base_key]);
	// and the base side's binary is the one built from the base commit's snapshot
	const base_binary = run.log.find((line) => line.startsWith(`${cache.path}/${base_key}/`))!;
	ok(base_binary.includes('/perf_ab-'), base_binary);
	const base_tree = await run_git(workspace, ['rev-parse', 'HEAD~1^{tree}']);
	ok(run.err.includes(`snapshotting tree ${base_tree.slice(0, 12)}`), run.err);
	ok(
		(await cache.read_text(`${base_key}/src/src/lib.rs`)) === '',
		'the base key holds the base commit'
	);
});

test('each side is graded on the samples its own binary measured', async () => {
	clear_git_repo_env();
	await using workspace = await make_workspace();
	await workspace.write('src/lib.rs', '// the candidate\n');
	await commit_all(workspace);
	await using tools = await make_tools();
	// a driver that reads the snapshot it was built in, beside it, and parses three times slower
	// when that is the candidate's
	const log = `${tools.path}/log`;
	await tools.write(
		'driver.sh',
		driver_script(log)
			.replace(
				'iterations=0;',
				`parse_us=100; grep -q candidate "$(dirname "$0")/src/src/lib.rs" && parse_us=300\niterations=0;`
			)
			.replace('\\"parse_us\\":100', '\\"parse_us\\":$parse_us'),
		{ mode: 0o755 }
	);
	await tools.write('build.sh', stamping_build_script(`${tools.path}/driver.sh`), { mode: 0o755 });
	await using cache = await fs.temp_dir('perf_ab_test_cache-');

	const run = await run_over_fakes(workspace, tools, cache, ['HEAD~1', 'HEAD', '--json'], true);
	const json = JSON.parse(run.out) as {
		rows: Array<{ lang: string; phase: string; base_wall_us: number; cand_wall_us: number }>;
	};
	const parse = json.rows.find((row) => row.lang === 'ts' && row.phase === 'parse')!;
	// flat instructions, so the wall move reads as layout and the run passes: only the rows tell
	strictEqual(run.code, 0, run.err);
	strictEqual(parse.cand_wall_us, 3 * parse.base_wall_us, JSON.stringify(parse));
});

test('without --one-shot the one-shot driver is neither read, built nor run, and the JSON holds the keys it always has', async () => {
	clear_git_repo_env();
	await using workspace = await make_workspace();
	await using tools = await make_tools();
	await using cache = await fs.temp_dir('perf_ab_test_cache-');

	const run = await run_over_fakes(workspace, tools, cache, ['HEAD', 'HEAD', '--json'], true);
	strictEqual(run.code, 0, run.err);
	deepStrictEqual(
		run.log.filter((line) => line.includes('perf_ab_one_shot')),
		[]
	);
	deepStrictEqual(Object.keys(JSON.parse(run.out) as object), [
		'base',
		'cand',
		'strip_skipped',
		'corpus',
		'rounds',
		'iterations',
		'instructions',
		'thresholds',
		'load_before',
		'load_after',
		'files',
		'absent',
		'noisy',
		'unsettled',
		'sign_off',
		'layout_reading',
		'cycles',
		'rows',
		'passes'
	]);
	const { base, cand } = JSON.parse(run.out) as Record<'base' | 'cand', { binary_sha256: object }>;
	deepStrictEqual(Object.keys(base.binary_sha256), ['format', 'strip']);
	deepStrictEqual(Object.keys(cand.binary_sha256), ['format', 'strip']);
});

test('--one-shot refuses a side without the strip before building anything', async () => {
	clear_git_repo_env();
	await using workspace = await make_workspace();
	await workspace.write('crates/tsv_ts/src/lib.rs', 'pub fn strip(source: &str) {}\n');
	await commit_all(workspace);
	await workspace.write('crates/tsv_ts/src/lib.rs', 'pub fn strip_in(source: &str) {}\n');
	await workspace.write('crates/tsv_debug/examples/perf_ab_one_shot.rs', 'fn main() {}\n');
	await commit_all(workspace);
	await using tools = await make_tools();
	await using cache = await fs.temp_dir('perf_ab_test_cache-');

	const run = await run_over_fakes(workspace, tools, cache, ['HEAD~1', 'HEAD', '--one-shot'], true);
	strictEqual(run.code, 2, run.err);
	ok(run.err.includes('--one-shot: the base lacks'), run.err);
	deepStrictEqual(
		run.log.filter((line) => line.startsWith('-n 19 ionice')),
		[]
	);
});

/** A shell line setting `one_shot_cand` when `binary` (a shell word) is the one-shot driver built
 * from the candidate's snapshot, read beside it. */
const is_cand_one_shot = (binary: string): string =>
	`case "${binary}" in */perf_ab_one_shot-*) grep -q candidate "$(dirname "${binary}")/src/src/lib.rs" && one_shot_cand=1 ;; esac`;

/** A scratch workspace whose candidate commit carries the one-shot driver and a marked library. */
const make_one_shot_workspace = async (): Promise<FsDir & AsyncDisposable> => {
	const workspace = await make_workspace();
	await workspace.write('crates/tsv_debug/examples/perf_ab_one_shot.rs', 'fn main() {}\n');
	await workspace.write('src/lib.rs', '// the candidate\n');
	await commit_all(workspace);
	return workspace;
};

test('--one-shot rows are informational: a one-shot regression never fails the run', async () => {
	clear_git_repo_env();
	await using workspace = await make_one_shot_workspace();
	await using tools = await make_tools();
	const log = `${tools.path}/log`;
	// the candidate's one-shot calls take three times the wall and twice the instructions; its
	// warm path ties
	await tools.write(
		'driver.sh',
		driver_script(log)
			.replace(
				'iterations=0;',
				`one_shot_cand=''; ${is_cand_one_shot('$0')}\nparse_us=100; [ -n "$one_shot_cand" ] && parse_us=300\niterations=0;`
			)
			.replace('\\"parse_us\\":100', '\\"parse_us\\":$parse_us'),
		{ mode: 0o755 }
	);
	await tools.write(
		'perf',
		perf_script(log).replace(
			'"$@"\ncode=$?',
			`one_shot_cand=''; ${is_cand_one_shot('$1')}\ncase " $* " in *" --iterations 0 "*) ;; *) [ -n "$one_shot_cand" ] && n=$((n * 2)) ;; esac\n"$@"\ncode=$?`
		),
		{ mode: 0o755 }
	);
	await tools.write('build.sh', stamping_build_script(`${tools.path}/driver.sh`), { mode: 0o755 });
	await using cache = await fs.temp_dir('perf_ab_test_cache-');

	const json_run = await run_over_fakes(
		workspace,
		tools,
		cache,
		['HEAD~1', 'HEAD', '--one-shot', '--json'],
		true
	);
	strictEqual(json_run.code, 0, json_run.err);
	const json = JSON.parse(json_run.out) as {
		rows: Array<{ lang: string; phase: string; verdict: string }>;
		one_shot_rows: Array<{
			lang: string;
			phase: string;
			verdict: string;
			base_wall_us: number;
			cand_wall_us: number;
			instructions_pct: number;
		}>;
		one_shot_files: Record<string, unknown>;
		one_shot_cycles: Record<string, unknown>;
		one_shot_note: string;
		passes: boolean;
		sign_off: Array<string>;
		noisy: Array<string>;
		unsettled: Array<string>;
		layout_reading: unknown;
	};
	// the graded rows tie and pass; every one-shot row fails by the rule, and nothing reads it
	ok(json.passes);
	deepStrictEqual(
		json.rows.map((row) => `${row.lang} ${row.phase} ${row.verdict}`),
		[
			'ts parse pass',
			'ts format pass',
			'svelte parse pass',
			'svelte format pass',
			'css parse pass',
			'css format pass',
			'strip parse pass',
			'strip strip pass'
		]
	);
	deepStrictEqual(
		json.one_shot_rows.map((row) => `${row.lang} ${row.phase} ${row.verdict}`),
		[
			'ts json fail',
			'ts format fail',
			'svelte json fail',
			'svelte format fail',
			'css json fail',
			'css format fail',
			'strip imports fail',
			'strip strip fail'
		]
	);
	const json_row = json.one_shot_rows[0]!;
	strictEqual(json_row.cand_wall_us, 3 * json_row.base_wall_us, JSON.stringify(json_row));
	deepStrictEqual([json.sign_off, json.noisy, json.unsettled], [[], [], []]);
	deepStrictEqual(Object.keys(json.one_shot_files), ['ts', 'svelte', 'css', 'strip']);
	deepStrictEqual(Object.keys(json.one_shot_cycles).sort(), [
		'css format',
		'css json',
		'strip imports',
		'strip strip',
		'svelte format',
		'svelte json',
		'ts format',
		'ts json'
	]);
	ok(json.one_shot_note.includes('INFORMATIONAL'));
	// the one-shot driver runs full, parse-only and zero iterations per language and round, its
	// own fixed cost: 2 rounds × 2 sides × 4 languages × 3, every language named, the strip too
	const one_shot_runs = json_run.log.filter(
		(line) => !line.startsWith('perf ') && line.includes('/perf_ab_one_shot-')
	);
	strictEqual(one_shot_runs.length, 48, one_shot_runs.join('\n'));
	strictEqual(
		one_shot_runs.filter((line) => line.includes('--iterations 0')).length,
		16,
		one_shot_runs.join('\n')
	);
	strictEqual(
		one_shot_runs.filter((line) => line.includes(' --lang strip ')).length,
		12,
		one_shot_runs.join('\n')
	);
	// and the warm drivers never run on the one-shot path
	strictEqual(
		json_run.log.filter((line) => line.includes('/perf_ab-') && line.includes('--lang strip'))
			.length,
		0
	);

	// the table run prints the one-shot rows in their own table, verdicts in parentheses
	const table_run = await run_over_fakes(
		workspace,
		tools,
		cache,
		['HEAD~1', 'HEAD', '--one-shot'],
		true
	);
	strictEqual(table_run.code, 0, table_run.err);
	ok(table_run.out.endsWith('perf:ab: PASS'), table_run.out);
	ok(table_run.out.includes('one-shot rows — '), table_run.out);
	ok(/\| TS json +\|.*\| \(FAIL\) +\|/.test(table_run.out), table_run.out);
});

test('one-shot rows shaped like a sign-off, a layout move, noise and a near-line wall reach none of those readings', async () => {
	clear_git_repo_env();
	await using workspace = await make_one_shot_workspace();
	await using tools = await make_tools();
	const log = `${tools.path}/log`;
	// the candidate's one-shot calls: the parse column +1.3% then +3.3% a round (paired median
	// +2.3%, a 0.98% round spread) at +1% instructions — a sign-off-shaped, noisy row near the
	// wall line that more rounds could settle; the second column +5% wall at flat instructions —
	// a layout-shaped row. The warm path ties.
	await tools.write(
		'driver.sh',
		driver_script(log)
			.replace(
				'iterations=0;',
				[
					`one_shot_cand=''; ${is_cand_one_shot('$0')}`,
					'parse_us=100; work_us=200; strip_us=50',
					'if [ -n "$one_shot_cand" ]; then',
					'	work_us=210; strip_us=52.5',
					'	case " $* " in',
					'		*" --parse-only "*|*" --iterations 0 "*) ;;',
					'		*)',
					'			lang=strip; previous=""',
					'			for arg in "$@"; do [ "$previous" = --lang ] && lang="$arg"; previous="$arg"; done',
					`			count=$(cat '${tools.path}/count_'"$lang" 2>/dev/null || echo 0); count=$((count + 1))`,
					`			echo "$count" > '${tools.path}/count_'"$lang"`,
					'			if [ $((count % 2)) = 1 ]; then parse_us=101.3; else parse_us=103.3; fi ;;',
					'	esac',
					'fi',
					'iterations=0;'
				].join('\n')
			)
			.replace('\\"parse_us\\":100', '\\"parse_us\\":$parse_us')
			.replace('\\"format_us\\":200', '\\"format_us\\":$work_us')
			.replace('\\"strip_us\\":50', '\\"strip_us\\":$strip_us'),
		{ mode: 0o755 }
	);
	await tools.write(
		'perf',
		perf_script(log).replace(
			'"$@"\ncode=$?',
			`one_shot_cand=''; ${is_cand_one_shot('$1')}\ncase " $* " in *" --iterations 0 "*) ;; *) [ -n "$one_shot_cand" ] && n=$((n + 10)) ;; esac\n"$@"\ncode=$?`
		),
		{ mode: 0o755 }
	);
	await tools.write('build.sh', stamping_build_script(`${tools.path}/driver.sh`), { mode: 0o755 });
	await using cache = await fs.temp_dir('perf_ab_test_cache-');

	const run = await run_over_fakes(
		workspace,
		tools,
		cache,
		['HEAD~1', 'HEAD', '--one-shot', '--json'],
		true
	);
	strictEqual(run.code, 0, run.err);
	const json = JSON.parse(run.out) as {
		rounds: number;
		rows: Array<{ lang: string; phase: string; verdict: string }>;
		one_shot_rows: Array<{
			lang: string;
			phase: string;
			verdict: string;
			sign_off: boolean;
			wall_pct: number;
			base_instructions: number;
			cand_instructions: number;
			instructions_pct: number;
			cand_wall_spread_pct: number;
		}>;
		passes: boolean;
		sign_off: Array<string>;
		noisy: Array<string>;
		unsettled: Array<string>;
		layout_reading: { row: string; wall_pct: number } | null;
	};
	const one_shot = (lang: string, phase: string) =>
		json.one_shot_rows.find((row) => row.lang === lang && row.phase === phase)!;
	// the fakes are shaped as described, so each reading below would name a one-shot row if it read
	// them
	const json_row = one_shot('ts', 'json');
	strictEqual(json_row.sign_off, true, JSON.stringify(json_row));
	strictEqual(json_row.wall_pct.toFixed(3), '2.300');
	ok(json_row.cand_wall_spread_pct > 0.75, JSON.stringify(json_row));
	strictEqual(json_row.instructions_pct.toFixed(3), '1.000');
	const as_row = json_row as unknown as AbRow;
	ok(row_is_ambiguous(as_row) && row_is_settleable(as_row), JSON.stringify(json_row));
	strictEqual(one_shot('svelte', 'format').instructions_pct, 0);
	strictEqual(one_shot('svelte', 'format').wall_pct.toFixed(3), '5.000');
	// and none does: no sign-off, no noise, nothing unsettled, no extra round, a warm layout reading
	ok(json.passes);
	deepStrictEqual([json.sign_off, json.noisy, json.unsettled], [[], [], []]);
	strictEqual(json.rounds, 2);
	deepStrictEqual(json.layout_reading?.wall_pct, 0);
	ok(
		json.rows.some((row) => `${row.lang} ${row.phase}` === json.layout_reading!.row),
		JSON.stringify(json.layout_reading)
	);
	// the one-shot strip row is the whole call: full − parse-only, never the warm strip's
	// full − 2 × parse-only + fixed (perf counts 5000 full, 2000 parse-only, 1000 fixed, over 3
	// iterations; the candidate 10 more on each timed one-shot run)
	const strip = one_shot('strip', 'strip');
	strictEqual(strip.base_instructions, (5000 - 2000) / 3);
	strictEqual(strip.cand_instructions, (5010 - 2010) / 3);
	strictEqual(one_shot('strip', 'imports').base_instructions, (2000 - 1000) / 3);
});

test('the root exits 2, never the regression code, when a program it needs is missing', async () => {
	// `node` found on this `PATH`, the root run with an empty one, so `git`, the first program it
	// makes, is missing; `FORCE_COLOR=0`, so a host setting both `NO_COLOR` and `FORCE_COLOR`
	// gets no warning from Node on the stderr compared here
	await using empty = await fs.temp_dir('perf_ab_test_path-');
	const root = new URL('./roots/perf_ab.ts', import.meta.url).pathname;
	const result = await spawn
		.program('node', { env: { PATH: empty.path, FORCE_COLOR: '0' } })
		.out([root, '--help']);
	deepStrictEqual(
		[result.kind === 'exited' && result.code, result.stdout, result.stderr],
		[2, '', 'perf:ab: error: not_found: no program git\n']
	);
});
