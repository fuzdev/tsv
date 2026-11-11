/**
 * `check_canonical_pins.ts`' mode flags, and what `--checkouts` says when the run has no git:
 * the presence of the `git` handle is what decides whether the drift half runs.
 */

import { deepStrictEqual, strictEqual } from 'node:assert';
import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';
import { afterEach, test, vi } from 'vitest';

import { parse_pins_args, run_check_canonical_pins } from './check_canonical_pins.ts';

test('neither flag or both select both modes; one flag selects its mode', () => {
	deepStrictEqual(parse_pins_args([]), { run_pins: true, run_checkouts: true });
	deepStrictEqual(parse_pins_args(['--pins', '--checkouts']), {
		run_pins: true,
		run_checkouts: true
	});
	deepStrictEqual(parse_pins_args(['--pins']), { run_pins: true, run_checkouts: false });
	deepStrictEqual(parse_pins_args(['--checkouts']), { run_pins: false, run_checkouts: true });
});

test('an argument that is neither flag is a usage error naming it', () => {
	deepStrictEqual(parse_pins_args(['--pins', '--bogus', 'x']), {
		error:
			'FAIL: unknown argument(s) --bogus, x — expected --pins and/or --checkouts (neither = both modes)'
	});
});

/** A read-only handle over `files`; every other path is absent. */
const fake_dir = (files: Record<string, string>): FsReadDir =>
	({
		read_text: async (path: string) => {
			const text = files[path];
			if (text === undefined) throw new Error(`not_found: ${path}`);
			return text;
		},
		read_text_sync: (path: string) => {
			const text = files[path];
			if (text === undefined) throw new Error(`not_found: ${path}`);
			return text;
		},
		dir: (path: string) => {
			throw new Error(`not_found: ${path}`);
		}
	}) as unknown as FsReadDir;

/** The pin sites every run reads, empty: `--checkouts` alone grades none of them. */
const PIN_SITES = {
	'crates/tsv_debug/src/deno/sidecar.ts': '',
	'crates/tsv_debug/src/deno/actor.rs': '',
	'benches/js/package.json': '{}',
	'crates/tsv_debug/src/deno/deno.lock': '{}'
};

const logged: string[] = [];
afterEach(() => {
	vi.restoreAllMocks();
	logged.length = 0;
});
const capture_log = () =>
	vi.spyOn(console, 'log').mockImplementation((line: string) => void logged.push(line));

test('--checkouts without git skips the drift half and says so', async () => {
	capture_log();
	const code = await run_check_canonical_pins(
		{ checkout: fake_dir(PIN_SITES), siblings: fake_dir({}), git: null },
		{ run_pins: false, run_checkouts: true }
	);
	strictEqual(code, 0);
	deepStrictEqual(logged, [
		'pins:audit:checkouts OK — checkouts aligned (checkout drift not checked — git is not on PATH; ' +
			'svelte checkout absent — alignment not checked; ' +
			'@sveltejs/acorn-typescript checkout absent — alignment not checked)'
	]);
});

test('--checkouts with git asks it for each sibling checkout that exists, and only those', async () => {
	capture_log();
	const asked: unknown[] = [];
	const git = {
		out: async (args: ReadonlyArray<string>, options: unknown) => {
			asked.push({ args, options });
			return { kind: 'exited', code: 128, stdout: '', stderr: '', truncated: false };
		}
	} as unknown as SpawnProgram;
	const code = await run_check_canonical_pins(
		{ checkout: fake_dir(PIN_SITES), siblings: fake_dir({}), git },
		{ run_pins: false, run_checkouts: true }
	);
	strictEqual(code, 0);
	// no sibling directory exists in the fake, so git is never run
	deepStrictEqual(asked, []);
	strictEqual(logged.length, 1);
	strictEqual(logged[0]!.includes('git is not on PATH'), false);
	strictEqual(logged[0]!.includes('../corpora absent — drift not checked'), true);
});

test('--checkouts runs git in each present sibling checkout, asking for its pinned object', async () => {
	capture_log();
	const asked: Array<{ args: ReadonlyArray<string>; cwd: unknown }> = [];
	// a handle per sibling, so the test sees which directory each git call ran in
	const handles = new Map<string, FsReadDir>();
	const siblings = {
		...fake_dir({}),
		dir: (path: string) => {
			let handle = handles.get(path);
			if (handle === undefined) {
				handle = { path: `/siblings/${path}` } as unknown as FsReadDir;
				handles.set(path, handle);
			}
			return handle;
		}
	} as unknown as FsReadDir;
	const git = {
		out: async (args: ReadonlyArray<string>, options: { cwd: unknown }) => {
			asked.push({ args, cwd: options.cwd });
			return { kind: 'exited', code: 128, stdout: '', stderr: '', truncated: false };
		}
	} as unknown as SpawnProgram;
	await run_check_canonical_pins(
		{ checkout: fake_dir(PIN_SITES), siblings, git },
		{ run_pins: false, run_checkouts: true }
	);
	const corpora = asked.find((a) => a.args.join(' ') === 'rev-parse HEAD:collections');
	strictEqual(corpora?.cwd, handles.get('corpora'));
	const svelte = asked.find((a) => a.cwd === handles.get('svelte'));
	deepStrictEqual(svelte?.args, ['rev-parse', 'HEAD']);
});
