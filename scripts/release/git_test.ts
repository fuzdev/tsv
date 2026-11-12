/**
 * The release's local git calls against real `git` in a temp repository the test makes, with
 * the user's and the system's git config shut out: the status lines and a rename's path, the
 * fingerprint, the branch, a revision and a tag's commit, the pathspec commit, and the
 * annotated tag. Nothing here reaches a remote; `fetch`, `ls-remote`, and `push` are pinned
 * by `scripts/publish_test.ts` over fakes.
 *
 * @module
 */

import { deepStrictEqual, notStrictEqual, ok, strictEqual } from 'node:assert';

import { fs, type FsDir } from '@fuzdev/gro/fs.ts';
import { spawn, type SpawnProgram } from '@fuzdev/gro/spawn.ts';
import { test } from 'vitest';

import {
	git_checkout_fingerprint,
	git_commit_paths,
	git_current_branch,
	git_head_subject,
	git_local_tag_commit,
	git_rev,
	git_status_line_path,
	git_status_lines,
	git_tag_annotated,
	type ReleaseGit
} from './git.ts';

/** A temp repository on `main` with one commit, and a `git` confined to it. */
const make_repo = async (): Promise<{
	dir: FsDir & AsyncDisposable;
	git: ReleaseGit;
	sh: (args: Array<string>) => Promise<string>;
}> => {
	const dir = await fs.temp_dir('tsv_release_git_test_');
	const program: SpawnProgram = spawn.program('git', {
		env: {
			HOME: dir.path,
			GIT_CONFIG_GLOBAL: '/dev/null',
			GIT_CONFIG_NOSYSTEM: '1',
			GIT_AUTHOR_NAME: 't',
			GIT_AUTHOR_EMAIL: 't@example.invalid',
			GIT_COMMITTER_NAME: 't',
			GIT_COMMITTER_EMAIL: 't@example.invalid'
		}
	});
	const sh = async (args: Array<string>): Promise<string> => {
		const result = await program.out(args, { cwd: dir });
		if (result.kind !== 'exited' || result.code !== 0) throw new Error(result.stderr);
		return result.stdout.trim();
	};
	await sh(['init', '-q', '-b', 'main']);
	for (const name of ['Cargo.toml', 'Cargo.lock', 'CHANGELOG.md', 'src.rs']) {
		await dir.write(name, `${name}\n`);
	}
	await sh(['add', '.']);
	await sh(['commit', '-q', '-m', 'first']);
	return { dir, git: { local: program, user: program, cwd: dir }, sh };
};

test('the status lines, a rename read as its destination, and none on a clean tree', async () => {
	const { dir, git, sh } = await make_repo();
	await using _ = dir;
	deepStrictEqual(await git_status_lines(git), { ok: true, lines: [] });
	await dir.write('Cargo.toml', 'edited\n');
	await dir.write('new.txt', 'x');
	await sh(['mv', 'src.rs', 'moved.rs']);
	const status = await git_status_lines(git);
	ok(status.ok);
	deepStrictEqual(status.lines, [' M Cargo.toml', 'R  src.rs -> moved.rs', '?? new.txt']);
	deepStrictEqual(status.lines.map(git_status_line_path), ['Cargo.toml', 'moved.rs', 'new.txt']);
});

test('the fingerprint holds still, and moves on a further edit, a new file, and a commit', async () => {
	const { dir, git, sh } = await make_repo();
	await using _ = dir;
	const read = async () => {
		const result = await git_checkout_fingerprint(git);
		ok(result.ok);
		return result.fingerprint;
	};
	await dir.write('Cargo.toml', 'one\n');
	const first = await read();
	strictEqual(await read(), first);
	await dir.write('Cargo.toml', 'two\n');
	const edited = await read();
	notStrictEqual(edited, first);
	await dir.write('untracked.txt', 'a');
	const added = await read();
	notStrictEqual(added, edited);
	// an untracked file's content is not seen, as documented
	await dir.write('untracked.txt', 'b');
	strictEqual(await read(), added);
	await sh(['commit', '-q', '-am', 'second']);
	notStrictEqual(await read(), added);
});

test('the branch, detached HEAD, a revision, and a missing tag', async () => {
	const { dir, git, sh } = await make_repo();
	await using _ = dir;
	strictEqual(await git_current_branch(git), 'main');
	const head = await sh(['rev-parse', 'HEAD']);
	strictEqual(await git_rev(git, 'HEAD'), head);
	strictEqual(await git_rev(git, 'no-such-rev'), null);
	strictEqual(await git_local_tag_commit(git, 'v9.9.9'), null);
	strictEqual(await git_head_subject(git), 'first');
	await sh(['checkout', '-q', '--detach']);
	strictEqual(await git_current_branch(git), '');
});

test('the pathspec commit takes the release files alone, then has nothing to commit', async () => {
	const { dir, git, sh } = await make_repo();
	await using _ = dir;
	await dir.write('Cargo.toml', 'bumped\n');
	await dir.write('CHANGELOG.md', 'stamped\n');
	await dir.write('src.rs', 'a stray edit\n');
	const paths = ['Cargo.toml', 'Cargo.lock', 'CHANGELOG.md'];
	deepStrictEqual(await git_commit_paths(git, paths, 'chore: publish v0.8.2'), {
		ok: true,
		committed: true
	});
	strictEqual(await git_head_subject(git), 'chore: publish v0.8.2');
	strictEqual(await sh(['show', '--name-only', '--format=', 'HEAD']), 'CHANGELOG.md\nCargo.toml');
	deepStrictEqual(await git_status_lines(git), { ok: true, lines: [' M src.rs'] });
	deepStrictEqual(await git_commit_paths(git, paths, 'again'), { ok: true, committed: false });
});

test('the tag is annotated, on HEAD, and read back peeled to its commit', async () => {
	const { dir, git, sh } = await make_repo();
	await using _ = dir;
	deepStrictEqual(await git_tag_annotated(git, 'v0.8.2'), { ok: true });
	strictEqual(await sh(['cat-file', '-t', 'refs/tags/v0.8.2']), 'tag');
	strictEqual(await sh(['tag', '-l', '--format=%(contents:subject)', 'v0.8.2']), 'v0.8.2');
	strictEqual(await git_local_tag_commit(git, 'v0.8.2'), await sh(['rev-parse', 'HEAD']));
	const again = await git_tag_annotated(git, 'v0.8.2');
	strictEqual(again.ok, false);
	ok(!again.ok && again.error.startsWith('git tag failed: fatal:'), JSON.stringify(again));
});
