import { deepStrictEqual } from 'node:assert';
import { existsSync } from 'node:fs';
import { mkdir, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

import { list_tree } from './perf_ab.ts';
import { build_inputs_of } from './perf_ab_verdict.ts';

/** The variables that point git at a repository other than the one its working directory is in.
 * A git hook runs with them set to the outer repo, and every git this file spawns — its own and
 * `list_tree`'s — inherits them, so the scratch repo's `add -A` would write the outer index. */
const GIT_REPO_ENV = ['GIT_DIR', 'GIT_INDEX_FILE', 'GIT_WORK_TREE'];

function clear_git_repo_env(): void {
	for (const name of GIT_REPO_ENV) Deno.env.delete(name);
}

async function git(cwd: string, args: ReadonlyArray<string>): Promise<string> {
	const output = await new Deno.Command('git', {
		args: [...args],
		cwd,
		stdout: 'piped',
		stderr: 'piped'
	}).output();
	const decoder = new TextDecoder();
	if (!output.success) {
		throw new Error(`git ${args.join(' ')} failed:\n${decoder.decode(output.stderr)}`);
	}
	return decoder.decode(output.stdout).trim();
}

/** Build a scratch repo holding a non-ASCII path and read it back through `list_tree`. */
async function assert_lists_raw_paths(): Promise<void> {
	clear_git_repo_env();
	const dir = await Deno.makeTempDir({ prefix: 'perf_ab_test-' });
	try {
		await git(dir, ['init', '--quiet']);
		await mkdir(join(dir, 'crates/é'), { recursive: true });
		await mkdir(join(dir, 'src'));
		await writeFile(join(dir, 'Cargo.toml'), '[workspace]\n');
		await writeFile(join(dir, 'crates/é/lib.rs'), '');
		await writeFile(join(dir, 'src/lib.rs'), '');
		await git(dir, ['add', '-A']);
		const tree = await git(dir, ['write-tree']);
		// the default listing quotes the path, which is the shape that must not reach the reader
		const quoted = await git(dir, ['-c', 'core.quotePath=true', 'ls-tree', '-r', tree]);
		deepStrictEqual(quoted.includes('"crates/\\303\\251/lib.rs"'), true);

		const listing = await list_tree(tree, ['Cargo.toml', 'crates', 'src'], dir);
		const read = build_inputs_of(listing, ['Cargo.toml', 'crates', 'src'], ['src/driver.rs']);
		deepStrictEqual(read.names, ['Cargo.toml', 'crates', 'src']);
		deepStrictEqual(read.missing, []);
		deepStrictEqual(read.key_text.includes('\tcrates/é/lib.rs'), true);
		deepStrictEqual(read.key_text.includes('"'), false);
	} finally {
		await rm(dir, { recursive: true, force: true });
	}
}

Deno.test('list_tree reads a non-ASCII path raw, never C-quoted', async () => {
	await assert_lists_raw_paths();
});

Deno.test('the scratch repo ignores a caller git environment that names another repo', async () => {
	const decoy = await Deno.makeTempDir({ prefix: 'perf_ab_test_decoy-' });
	try {
		await git(decoy, ['init', '--quiet']);
		Deno.env.set('GIT_DIR', join(decoy, '.git'));
		Deno.env.set('GIT_INDEX_FILE', join(decoy, '.git/index'));
		Deno.env.set('GIT_WORK_TREE', decoy);
		await assert_lists_raw_paths();
		// a fresh repo has no index until something is added to it
		deepStrictEqual(existsSync(join(decoy, '.git/index')), false);
	} finally {
		clear_git_repo_env();
		await rm(decoy, { recursive: true, force: true });
	}
});
