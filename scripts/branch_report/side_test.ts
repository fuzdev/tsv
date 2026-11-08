import { deepStrictEqual } from 'node:assert';
import { mkdir, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

import { corpus_identity } from './side.ts';

/** A git hook sets these to the outer repo, and every git spawned here inherits them. */
function clear_git_repo_env(): void {
	for (const name of ['GIT_DIR', 'GIT_INDEX_FILE', 'GIT_WORK_TREE']) Deno.env.delete(name);
}

async function git(cwd: string, args: ReadonlyArray<string>): Promise<void> {
	const output = await new Deno.Command('git', { args: [...args], cwd, stderr: 'piped' }).output();
	if (!output.success) throw new Error(new TextDecoder().decode(output.stderr));
}

Deno.test(
	'corpus_identity tells a subdirectory corpus from the whole, and sees its status',
	async () => {
		clear_git_repo_env();
		const repo = await Deno.makeTempDir({ prefix: 'branch_report_side-' });
		try {
			await mkdir(join(repo, 'collections/zzz'), { recursive: true });
			await writeFile(join(repo, 'collections/zzz/a.ts'), 'a\n');
			await git(repo, ['init', '--quiet']);
			await git(repo, ['add', '-A']);
			await git(repo, ['-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-qm', 'x']);

			const whole = await corpus_identity(join(repo, 'collections'));
			const sub = await corpus_identity(join(repo, 'collections/zzz'));
			deepStrictEqual(whole.tree === sub.tree, false);
			deepStrictEqual(whole.relative_path, 'collections/');
			deepStrictEqual(sub.relative_path, 'collections/zzz/');
			deepStrictEqual(whole.dirty, false);
			deepStrictEqual(sub.dirty, false);

			await writeFile(join(repo, 'collections/zzz/a.ts'), 'changed\n');
			deepStrictEqual((await corpus_identity(join(repo, 'collections'))).dirty, true);
			// the corpus-relative pathspec sees the change from a subdirectory corpus too
			deepStrictEqual((await corpus_identity(join(repo, 'collections/zzz'))).dirty, true);
		} finally {
			await rm(repo, { recursive: true, force: true });
		}
	}
);
