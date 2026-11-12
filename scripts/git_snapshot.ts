/**
 * What the two-tree build harnesses share: query git, extract a tree from the object store and
 * hash bytes, each failing loudly through `HarnessError`. A harness builds a tree it extracted,
 * never the checkout. Each operation takes the programs it runs as handles its root builds and
 * the directories it works in as fs handles, so the root decides every child's environment.
 *
 * @module
 */

import type { FsDir, FsReadDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram, SpawnResult } from '@fuzdev/gro/spawn.ts';

export class HarnessError extends Error {}

/** The host variables a harness build is given beside gro's base environment (`PATH`, `HOME`,
 * `TMPDIR`, `TERM`, `LC_ALL=C`, `NO_COLOR`, `FORCE_COLOR`); nothing else reaches it, so a
 * caller's `RUSTFLAGS`, `CARGO_PROFILE_*`, `RUSTUP_TOOLCHAIN` or `RUSTC_WRAPPER` reaches neither
 * side. Locating the toolchain needs `PATH`, `HOME` and the rustup / cargo homes;
 * `CARGO_BUILD_JOBS` only schedules the build (a release build's codegen is one unit, so the
 * artifact does not depend on it). */
export const BUILD_ENV: ReadonlyArray<string> = [
	'PATH',
	'HOME',
	'CARGO_HOME',
	'RUSTUP_HOME',
	'CARGO_BUILD_JOBS'
];

/** A child's exit code, or `null` when it ended any other way (a signal, a stop). */
export const exit_code_of = (result: SpawnResult): number | null =>
	result.kind === 'exited' ? result.code : null;

/** How a child ended, for a message: `exited 3`, `was killed by SIGKILL`. */
export const format_ending = (result: SpawnResult): string => {
	if (result.kind === 'exited') return `exited ${result.code}`;
	if (result.kind === 'signaled') return `was killed by ${result.signal}`;
	return `was stopped (${result.kind})`;
};

/** Run `git args` in `cwd`; its trimmed stdout. */
export async function git_out(
	git: SpawnProgram,
	cwd: FsReadDir,
	args: ReadonlyArray<string>
): Promise<string> {
	const result = await git.out(args, { cwd });
	if (exit_code_of(result) !== 0) {
		throw new HarnessError(`git ${args.join(' ')} failed:\n${result.stderr.trim()}`);
	}
	return result.stdout.trim();
}

/** The recursive, NUL-separated `git ls-tree -r -z` listing of `paths` in `tree`, run in `cwd`
 * (untrimmed: a raw path may end in whitespace). */
export async function list_tree(
	git: SpawnProgram,
	cwd: FsReadDir,
	tree: string,
	paths: ReadonlyArray<string>
): Promise<string> {
	const args = ['ls-tree', '-r', '-z', tree, '--', ...paths];
	const result = await git.out(args, { cwd });
	if (exit_code_of(result) !== 0) {
		throw new HarnessError(`git ${args.join(' ')} failed:\n${result.stderr.trim()}`);
	}
	return result.stdout;
}

export async function sha256_hex(bytes: Uint8Array): Promise<string> {
	const digest = await crypto.subtle.digest('SHA-256', new Uint8Array(bytes));
	return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, '0')).join('');
}

/** What `extract_tree` runs: `git` in the checkout, `tar` in the directory it fills. */
export interface ExtractTreeDeps {
	git: SpawnProgram;
	tar: SpawnProgram;
	/** The repository the tree is read from, `git archive`'s working directory. */
	checkout: FsReadDir;
	/** Names the partial directory, so two concurrent runs never share one. */
	pid: number;
}

/** Every chunk of `stream`, joined and decoded. */
const read_text = async (stream: AsyncIterable<Uint8Array>): Promise<string> => {
	const decoder = new TextDecoder();
	let text = '';
	for await (const chunk of stream) text += decoder.decode(chunk, { stream: true });
	return text + decoder.decode();
};

/** Extract `paths` of `tree` (its whole tree when `paths` is empty) into `parent`'s `name` with
 * `git archive | tar -x`, via a per-process partial directory so an interrupted extraction is
 * never mistaken for a snapshot and two concurrent runs never share one. */
export async function extract_tree(
	deps: ExtractTreeDeps,
	tree: string,
	paths: ReadonlyArray<string>,
	parent: FsDir,
	name: string
): Promise<void> {
	const partial = `${name}.partial.${deps.pid}`;
	await parent.remove(partial, { recursive: true, missing_ok: true });
	await parent.mkdir(partial, { recursive: true });
	const archive = await deps.git.start(
		['archive', '--format=tar', tree, ...(paths.length > 0 ? ['--', ...paths] : [])],
		{ cwd: deps.checkout, stdio: ['ignore', 'pipe', 'pipe'] }
	);
	const tar = await deps.tar.start(['-x'], {
		cwd: parent.dir(partial),
		stdio: ['pipe', 'ignore', 'pipe']
	});
	const pump = async (): Promise<void> => {
		for await (const chunk of archive.stdout!) {
			if (!(await tar.stdin!.write(chunk))) break;
		}
		await tar.stdin!.close();
	};
	const [archive_stderr, tar_stderr] = await Promise.all([
		read_text(archive.stderr!),
		read_text(tar.stderr!),
		pump()
	]);
	const [archive_result, tar_result] = await Promise.all([archive.closed, tar.closed]);
	const archive_ok = exit_code_of(archive_result) === 0;
	if (!archive_ok || exit_code_of(tar_result) !== 0) {
		await parent.remove(partial, { recursive: true, missing_ok: true });
		throw new HarnessError(
			`extracting tree ${tree} failed:\n${(archive_ok ? tar_stderr : archive_stderr).trim()}`
		);
	}
	try {
		await parent.rename(partial, name);
	} catch (err) {
		// a concurrent run finished the same extraction first; its copy is the same tree
		await parent.remove(partial, { recursive: true, missing_ok: true });
		if (!(await parent.exists(name))) throw err;
	}
}
