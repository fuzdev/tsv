/**
 * `roundtrip_audit_prettier.ts` over fakes: an absent checkout passes unrun, saying so; a present
 * one holding none of the suites fails unrun, naming them; and a present one audits the suites
 * it holds, naming those it lacks, with cargo's exit code passed on.
 */

import { deepStrictEqual } from 'node:assert';
import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';
import { afterEach, test, vi } from 'vitest';

import {
	PRETTIER_ROUNDTRIP_SUITES,
	run_roundtrip_audit_prettier
} from './roundtrip_audit_prettier.ts';

afterEach(() => {
	vi.restoreAllMocks();
});

const checkout = { path: '/dev/tsv' } as unknown as FsReadDir;

/** A `cargo` exiting `code`, or killed by it when a signal name, recording each call's arguments. */
const fake_cargo = (code: number | string) => {
	const calls: Array<ReadonlyArray<string>> = [];
	const program = {
		run: async (args: ReadonlyArray<string>) => {
			calls.push(args);
			return typeof code === 'number'
				? { kind: 'exited', code }
				: { kind: 'signaled', signal: code };
		}
	} as unknown as SpawnProgram;
	return { program, calls };
};

/** A `../prettier` checkout holding `suites` (paths beneath it), or none at all for `null`. */
const prettier_reads = (suites: ReadonlyArray<string> | null) => ({
	open: async () => (suites === null ? null : ({ path: '/dev/prettier' } as unknown as FsReadDir)),
	exists: async (path: string) => suites !== null && suites.includes(path)
});

/** The run's exit code, what it printed, and what cargo was asked. */
const audit = async (suites: ReadonlyArray<string> | null, cargo_code: number | string = 0) => {
	const lines: Array<string> = [];
	vi.spyOn(console, 'error').mockImplementation((line: string) => void lines.push(line));
	const cargo = fake_cargo(cargo_code);
	const code = await run_roundtrip_audit_prettier({
		checkout,
		prettier: prettier_reads(suites),
		cargo: cargo.program
	});
	return { code, lines, cargo: cargo.calls };
};

test('an absent checkout passes without running cargo, saying so', async () => {
	deepStrictEqual(await audit(null), {
		code: 0,
		lines: [
			'⚠ roundtrip:audit:prettier NOT RUN — no ../prettier checkout (3 suites skipped).',
			'  This leg is the only one in `check` that reaches non-format-stable inputs.',
			'  Clone prettier beside this repo (see `npm run doctor`) to gate it locally.'
		],
		cargo: []
	});
});

test('a present checkout holding none of the suites fails without running cargo, naming them', async () => {
	deepStrictEqual(await audit([]), {
		code: 1,
		lines: [
			`Error: a ../prettier checkout is present but holds none of the suites this leg grades (${PRETTIER_ROUNDTRIP_SUITES.join(', ')}) — a broken checkout, not a machine without one; repair it, or remove it to skip the leg.`
		],
		cargo: []
	});
});

test('a present checkout audits the suites it holds, names those it lacks, and passes on the exit code', async () => {
	const run = await audit(['tests/format/js'], 3);
	deepStrictEqual(run.code, 3);
	deepStrictEqual(run.lines, [
		'⚠ prettier suite missing from a present ../prettier checkout: ../prettier/tests/format/typescript',
		'⚠ prettier suite missing from a present ../prettier checkout: ../prettier/tests/format/css'
	]);
	deepStrictEqual(run.cargo, [
		[
			'run',
			'--profile',
			'corpus',
			'-q',
			'-p',
			'tsv_debug',
			'--features',
			'_internal_audits',
			'--',
			'roundtrip_audit',
			'--gate',
			'../prettier/tests/format/js'
		]
	]);
});

test('a cargo a signal ended fails the run', async () => {
	const run = await audit(
		['tests/format/js', 'tests/format/typescript', 'tests/format/css'],
		'SIGKILL'
	);
	deepStrictEqual([run.code, run.lines, run.cargo.length], [1, [], 1]);
});
