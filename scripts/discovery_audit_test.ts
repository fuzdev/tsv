/**
 * `discovery_audit.ts`' grading over fakes: an absent snapshot passes unrun, a modified one is
 * refused before tsv runs, and git and tsv each run where the audit says.
 */

import { deepStrictEqual, strictEqual } from 'node:assert';
import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';
import { afterEach, test, vi } from 'vitest';

import { run_discovery_audit } from './discovery_audit.ts';

afterEach(() => {
	vi.restoreAllMocks();
});

/** A program whose stdout is `stdout`, ending with `code`, recording each call. */
const fake_program = (
	respond: (args: ReadonlyArray<string>) => { stdout: string; code: number }
) => {
	const calls: Array<{ args: ReadonlyArray<string>; cwd: unknown }> = [];
	const program = {
		start: async (args: ReadonlyArray<string>, options: { cwd: unknown }) => {
			calls.push({ args, cwd: options.cwd });
			const { stdout, code } = respond(args);
			return {
				stdout: (async function* () {
					yield new TextEncoder().encode(stdout);
				})(),
				closed: Promise.resolve({ kind: 'exited', code })
			};
		}
	} as unknown as SpawnProgram;
	return { program, calls };
};

const checkout = { path: '/dev/tsv' } as unknown as FsReadDir;
const corpora = { path: '/dev/corpora' } as unknown as FsReadDir;
const siblings = (present: boolean) =>
	({
		exists: async (path: string) => present && path === 'corpora/collections',
		dir: (path: string) => {
			if (path !== 'corpora') throw new Error(`unexpected sibling ${path}`);
			return corpora;
		}
	}) as unknown as FsReadDir;

test('an absent snapshot passes without running git or tsv', async () => {
	vi.spyOn(console, 'error').mockImplementation(() => undefined);
	const git = fake_program(() => ({ stdout: '', code: 0 }));
	const tsv = fake_program(() => ({ stdout: '', code: 0 }));
	const code = await run_discovery_audit({
		checkout,
		siblings: siblings(false),
		git: git.program,
		tsv: tsv.program
	});
	deepStrictEqual([code, git.calls.length, tsv.calls.length], [0, 0, 0]);
});

test('a modified snapshot is refused before tsv runs', async () => {
	vi.spyOn(console, 'error').mockImplementation(() => undefined);
	const git = fake_program(() => ({ stdout: ' M collections/a.ts\n', code: 0 }));
	const tsv = fake_program(() => ({ stdout: '', code: 0 }));
	const code = await run_discovery_audit({
		checkout,
		siblings: siblings(true),
		git: git.program,
		tsv: tsv.program
	});
	deepStrictEqual([code, tsv.calls.length], [1, 0]);
	deepStrictEqual(git.calls, [
		{ args: ['status', '--porcelain', '--', 'collections'], cwd: corpora }
	]);
});

test('git lists the snapshot in it, tsv lists it from the checkout, and agreement passes', async () => {
	vi.spyOn(console, 'error').mockImplementation(() => undefined);
	const git = fake_program((args) =>
		args[0] === 'ls-files'
			? { stdout: 'collections/a.ts\0collections/b.md\0', code: 0 }
			: { stdout: '', code: 0 }
	);
	const tsv = fake_program(() => ({ stdout: '../corpora/collections/a.ts\n', code: 0 }));
	const code = await run_discovery_audit({
		checkout,
		siblings: siblings(true),
		git: git.program,
		tsv: tsv.program
	});
	strictEqual(code, 0);
	deepStrictEqual(git.calls[1], {
		args: ['ls-files', '-z', '--', 'collections'],
		cwd: corpora
	});
	deepStrictEqual(tsv.calls, [
		{ args: ['format', '--list', '../corpora/collections'], cwd: checkout }
	]);
});
