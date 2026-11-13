/**
 * A side of the branch report over scratch git repos: the corpus identity, each era's runner
 * for a side's own task (a tree whose tasks are in `deno.json`, one after they moved to
 * `package.json`, one lacking the task), a missing runner, and the cache's eviction.
 *
 * The git children here read the live environment when each program is made, so the suite
 * clears git's repository variables first (`clear_git_repo_env`), and makes every program after.
 */

import { deepStrictEqual, ok, rejects, strictEqual } from 'node:assert';

import { fs, type FsDir, type FsReadDir } from '@fuzdev/gro/fs.ts';
import { GIT_LOCAL_PASS_ENV } from '@fuzdev/gro/git.ts';
import { spawn, type SpawnProgram } from '@fuzdev/gro/spawn.ts';
import { test } from 'vitest';

import { HarnessError } from '../git_snapshot.ts';
import { run_leg } from './legs.ts';
import { type FakeCall, fake_host, fake_program, fake_side } from './report_fakes.ts';
import {
	corpus_identity,
	evict_sides,
	KEEP_SIDES,
	MissingTool,
	prepare_side,
	type ReportHost,
	require_tool,
	run_side_task,
	side_task,
	UnavailableOnRef
} from './side.ts';

/** A git hook sets these to the outer repo, and every git program made here reads them. */
const clear_git_repo_env = (): void => {
	for (const name of ['GIT_DIR', 'GIT_INDEX_FILE', 'GIT_WORK_TREE']) delete process.env[name];
};

clear_git_repo_env();

const make_git = (): SpawnProgram => spawn.program('git', { pass_env: GIT_LOCAL_PASS_ENV });

const run_git = async (cwd: FsReadDir, args: ReadonlyArray<string>): Promise<string> => {
	const result = await make_git().out(['-c', 'commit.gpgsign=false', ...args], { cwd });
	if (result.kind !== 'exited' || result.code !== 0) {
		throw new Error(`git ${args.join(' ')} failed:\n${result.stderr}`);
	}
	return result.stdout.trim();
};

const commit_all = async (repo: FsDir): Promise<string> => {
	await run_git(repo, ['add', '-A']);
	await run_git(repo, ['-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-qm', 'x']);
	return await run_git(repo, ['rev-parse', 'HEAD']);
};

test('corpus_identity tells a subdirectory corpus from the whole, and sees its status', async () => {
	clear_git_repo_env();
	await using repo = await fs.temp_dir('branch_report_side-');
	await repo.mkdir('collections/zzz', { recursive: true });
	await repo.write('collections/zzz/a.ts', 'a\n');
	await run_git(repo, ['init', '--quiet']);
	await commit_all(repo);

	const git = make_git();
	const identity = (path: string) => corpus_identity(git, repo, `${repo.path}/${path}`);
	const whole = await identity('collections');
	const sub = await identity('collections/zzz');
	deepStrictEqual(whole.tree === sub.tree, false);
	deepStrictEqual(whole.relative_path, 'collections/');
	deepStrictEqual(sub.relative_path, 'collections/zzz/');
	deepStrictEqual(whole.dirty, false);
	deepStrictEqual(sub.dirty, false);

	await repo.write('collections/zzz/a.ts', 'changed\n');
	deepStrictEqual((await identity('collections')).dirty, true);
	// the corpus-relative pathspec sees the change from a subdirectory corpus too
	deepStrictEqual((await identity('collections/zzz')).dirty, true);
});

test('corpus_identity asks git without optional locks, and names a corpus outside git', async () => {
	await using dir = await fs.temp_dir('branch_report_side-');
	const calls: Array<FakeCall> = [];
	const git = fake_program('git', calls, () => ({ code: 128, stderr: 'not a git repository' }));
	const identity = await corpus_identity(git, dir, '/somewhere/corpus');
	deepStrictEqual(identity, {
		tree: '(no git checkout at /somewhere/corpus)',
		dirty: false,
		relative_path: '/somewhere/corpus'
	});
	for (const call of calls)
		deepStrictEqual(call.args.slice(0, 3), ['--no-optional-locks', '-C', '/somewhere/corpus']);
	deepStrictEqual(calls.length, 3);
});

/** A host whose `git` and `tar` are real, over `repo`, and whose `rustc` / `cargo` are fakes. */
const repo_host = (cache: FsDir, repo: FsReadDir, calls: Array<FakeCall>): ReportHost => {
	const host = fake_host(cache, repo, calls, {
		tools: {
			rustc: fake_program('rustc', calls, () => ({ stdout: 'rustc 1.94.1\nhost: x\n' })),
			cargo: fake_program('cargo', calls, () => ({ stdout: 'cargo 1.94.1\n' }))
		}
	});
	return { ...host, git: make_git(), tar: spawn.program('tar') };
};

test("each era's tree runs its own tasks: deno.json's by deno task, package.json's by npm run", async () => {
	clear_git_repo_env();
	await using repo = await fs.temp_dir('branch_report_era-');
	await using cache = await fs.temp_dir('branch_report_cache-');
	await run_git(repo, ['init', '--quiet']);
	await repo.write('Cargo.toml', '[workspace]\n');

	// the tasks all in deno.json, as before they moved
	await repo.write(
		'deno.json',
		JSON.stringify({ tasks: { 'build:bench': 'deno task x', 'bench:node:run': 'node b.ts' } })
	);
	await repo.write('package.json', JSON.stringify({ name: 'tsv' }));
	const deno_era = await commit_all(repo);

	// after the move: only the Deno-running tasks stay in deno.json
	await repo.write('deno.json', JSON.stringify({ tasks: { 'bench:deno:run': 'deno run b.ts' } }));
	await repo.write(
		'package.json',
		JSON.stringify({ scripts: { 'build:bench': 'npm run x', 'bench:node:run': 'node b.ts' } })
	);
	const npm_era = await commit_all(repo);

	// a tree lacking the bench's tasks altogether, with no deno.json
	await repo.remove('deno.json');
	await repo.write('package.json', JSON.stringify({ scripts: { check: 'true' } }));
	const bare = await commit_all(repo);

	const calls: Array<FakeCall> = [];
	const host = repo_host(cache, repo, calls);
	const old_side = await prepare_side(host, deno_era, 'base');
	const new_side = await prepare_side(host, npm_era, 'cand');
	strictEqual(old_side.rustc, 'rustc 1.94.1 host: x');
	strictEqual(await old_side.src_handle.exists('Cargo.toml'), true);

	deepStrictEqual(await side_task(old_side, 'bench:node:run', ['--save-report']), {
		runner: 'deno',
		argv: ['deno', 'task', 'bench:node:run', '--save-report']
	});
	deepStrictEqual(await side_task(new_side, 'bench:node:run', ['--save-report']), {
		runner: 'npm',
		argv: ['npm', 'run', 'bench:node:run', '--', '--save-report']
	});
	deepStrictEqual(await side_task(new_side, 'build:bench'), {
		runner: 'npm',
		argv: ['npm', 'run', 'build:bench']
	});
	// a task whose body runs Deno stays in deno.json, so Deno runs it in the new era too
	deepStrictEqual((await side_task(new_side, 'bench:deno:run')).runner, 'deno');

	const bare_side = await prepare_side(host, bare, 'base');
	await rejects(side_task(bare_side, 'build:bench'), (err) => {
		ok(err instanceof UnavailableOnRef);
		ok(err.message.includes('has no task build:bench'));
		return true;
	});
	// a name only `Object.prototype` holds is no task
	await rejects(side_task(bare_side, 'toString'), UnavailableOnRef);
});

test('a side task the tree lacks is a leg not run, and a missing runner a missing tool', async () => {
	await using cache = await fs.temp_dir('branch_report_cache-');
	await using checkout = await fs.temp_dir('branch_report_checkout-');
	const calls: Array<FakeCall> = [];
	const host = fake_host(cache, checkout, calls);
	const side = await fake_side(cache, 'base', {
		'package.json': JSON.stringify({ scripts: { 'build:bench': 'true' } })
	});

	const not_run = await run_leg('a leg', (log) => run_side_task(host, side, 'bench:x', [], log));
	strictEqual(not_run.status, 'unavailable_on_ref');
	const no_npm = await run_leg('a leg', (log) => run_side_task(host, side, 'build:bench', [], log));
	strictEqual(no_npm.status, 'missing_tool');
	strictEqual(no_npm.reason, '`npm` is not installed');
	// neither started anything
	strictEqual(calls.length, 0);

	const with_npm = { ...host, tools: { ...host.tools, npm: fake_program('npm', calls) } };
	const ran = await run_leg('a leg', (log) =>
		run_side_task(with_npm, side, 'build:bench', [], log, { default_target_dir: true })
	);
	strictEqual(ran.status, 'ok');
	deepStrictEqual(
		calls.map((c) => [c.program, c.args, c.cwd, c.env]),
		[['nice', ['-n', '19', 'ionice', '-c', '3', 'npm', 'run', 'build:bench'], side.src, {}]]
	);
});

test('a side tree whose deno.json is not JSON fails loudly, never reads as taskless', async () => {
	await using cache = await fs.temp_dir('branch_report_cache-');
	const side = await fake_side(cache, 'cand', { 'deno.json': '{ "tasks": ' });
	await rejects(side_task(side, 'build:bench'), (err) => {
		ok(err instanceof HarnessError && !(err instanceof UnavailableOnRef));
		ok(err.message.includes('deno.json is not JSON'));
		return true;
	});
});

test('a missing tool is named, and time by its path', async () => {
	await using cache = await fs.temp_dir('branch_report_cache-');
	await using checkout = await fs.temp_dir('branch_report_checkout-');
	const host = fake_host(cache, checkout, []);
	const time = await run_leg('a leg', async () => {
		require_tool(host, 'time');
	});
	strictEqual(time.reason, '`/usr/bin/time` is not installed');
	ok(new MissingTool('perf').message.includes('`perf`'));
});

test('eviction keeps the sides in use and the most recent others, and nothing else', async () => {
	await using cache = await fs.temp_dir('branch_report_cache-');
	const names = Array.from({ length: KEEP_SIDES + 2 }, (_, i) => `${String(i).repeat(40)}-base`);
	for (const [i, name] of names.entries()) {
		await cache.mkdir(name);
		await cache.set_times(name, { mtime_ms: 1_000_000 + i * 1000 });
	}
	await cache.mkdir('reports');
	await cache.write(`${'f'.repeat(40)}-cand`, 'a file, not a side');
	const in_use = await fake_side(cache, 'cand', {}, 'e'.repeat(40));
	await cache.set_times(in_use.name, { mtime_ms: 1 });
	await evict_sides(cache, [in_use]);
	const left = [];
	for await (const entry of cache.list()) left.push(entry.name);
	// the side in use (touched now), the three newest others, and what isn't a side directory
	deepStrictEqual(
		left.sort(),
		[...names.slice(-(KEEP_SIDES - 1)), in_use.name, `${'f'.repeat(40)}-cand`, 'reports'].sort()
	);
});
