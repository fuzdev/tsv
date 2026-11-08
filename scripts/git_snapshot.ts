/**
 * What the two-tree build harnesses share (`scripts/perf_ab.ts`, `scripts/branch_report/`): run a
 * command, query git, extract a tree from the object store and hash bytes, each failing loudly
 * through `HarnessError`. A harness builds a tree it extracted, never the checkout.
 *
 * @module
 */

import { existsSync } from 'node:fs';
import { mkdir, rename, rm } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

export const ROOT = fileURLToPath(new URL('..', import.meta.url));

export class HarnessError extends Error {}

/** The variables a harness build keeps from the caller; everything else is cleared, so a
 * caller's `RUSTFLAGS`, `CARGO_PROFILE_*`, `RUSTUP_TOOLCHAIN` or `RUSTC_WRAPPER` reaches neither
 * side. Locating the toolchain needs `PATH`, `HOME` and the rustup / cargo homes;
 * `CARGO_BUILD_JOBS` only schedules the build (a release build's codegen is one unit, so the
 * artifact does not depend on it). */
export const BUILD_ENV = ['PATH', 'HOME', 'CARGO_HOME', 'RUSTUP_HOME', 'CARGO_BUILD_JOBS'];

/** The caller's values for `BUILD_ENV`, ready to extend with a build's own variables. */
export function kept_build_env(): Record<string, string> {
	const env: Record<string, string> = {};
	for (const name of BUILD_ENV) {
		const value = Deno.env.get(name);
		if (value !== undefined) env[name] = value;
	}
	return env;
}

export interface RunResult {
	success: boolean;
	code: number;
	stdout: string;
	stderr: string;
}

export async function run(
	cmd: string,
	args: ReadonlyArray<string>,
	options: { cwd?: string; env?: Record<string, string>; clear_env?: boolean } = {}
): Promise<RunResult> {
	const output = await new Deno.Command(cmd, {
		args: [...args],
		cwd: options.cwd ?? ROOT,
		env: options.env,
		clearEnv: options.clear_env ?? false,
		stdout: 'piped',
		stderr: 'piped'
	}).output();
	const decoder = new TextDecoder();
	return {
		success: output.success,
		code: output.code,
		stdout: decoder.decode(output.stdout),
		stderr: decoder.decode(output.stderr)
	};
}

export async function git(
	args: ReadonlyArray<string>,
	env?: Record<string, string>
): Promise<string> {
	const result = await run('git', args, { env });
	if (!result.success) {
		throw new HarnessError(`git ${args.join(' ')} failed:\n${result.stderr.trim()}`);
	}
	return result.stdout.trim();
}

/** The recursive, NUL-separated `git ls-tree -r -z` listing of `paths` in `tree`, run in `cwd`
 * (untrimmed: a raw path may end in whitespace). */
export async function list_tree(
	tree: string,
	paths: ReadonlyArray<string>,
	cwd: string = ROOT
): Promise<string> {
	const args = ['ls-tree', '-r', '-z', tree, '--', ...paths];
	const result = await run('git', args, { cwd });
	if (!result.success) {
		throw new HarnessError(`git ${args.join(' ')} failed:\n${result.stderr.trim()}`);
	}
	return result.stdout;
}

export async function sha256_hex(bytes: Uint8Array<ArrayBuffer>): Promise<string> {
	const digest = await crypto.subtle.digest('SHA-256', bytes);
	return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, '0')).join('');
}

/** Extract `paths` of `tree` (its whole tree when `paths` is empty) into `dir` with `git archive
 * | tar -x`, via a per-process partial directory so an interrupted extraction is never mistaken
 * for a snapshot and two concurrent runs never share one. */
export async function extract_tree(
	tree: string,
	paths: ReadonlyArray<string>,
	dir: string
): Promise<void> {
	const partial = `${dir}.partial.${Deno.pid}`;
	await rm(partial, { recursive: true, force: true });
	await mkdir(partial, { recursive: true });
	const archive = new Deno.Command('git', {
		args: ['archive', '--format=tar', tree, ...(paths.length > 0 ? ['--', ...paths] : [])],
		cwd: ROOT,
		stdout: 'piped',
		stderr: 'piped'
	}).spawn();
	const tar = new Deno.Command('tar', {
		args: ['-x', '-C', partial],
		stdin: 'piped',
		stdout: 'null',
		stderr: 'piped'
	}).spawn();
	await archive.stdout.pipeTo(tar.stdin);
	const [archive_out, tar_out] = await Promise.all([archive.output(), tar.output()]);
	if (!archive_out.success || !tar_out.success) {
		await rm(partial, { recursive: true, force: true });
		const stderr = new TextDecoder().decode(
			archive_out.success ? tar_out.stderr : archive_out.stderr
		);
		throw new HarnessError(`extracting tree ${tree} failed:\n${stderr.trim()}`);
	}
	try {
		await rename(partial, dir);
	} catch (err) {
		// a concurrent run finished the same extraction first; its copy is the same tree
		await rm(partial, { recursive: true, force: true });
		if (!existsSync(dir)) throw err;
	}
}
