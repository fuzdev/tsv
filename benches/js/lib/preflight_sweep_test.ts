/**
 * Tests for the bench pre-flight's per-row sweep (`sweep_preflight_task`) over fake rows: a
 * tsv crash, a caught native panic or a WASM trap, in one row or both, on the first input or
 * the last, at either goal, lands in the crash list and in no accept or skip set, while an
 * ordinary rejection, and any third-party row's throw, is the skip it always was.
 * Package-free, so it gates in `npm run test:node`.
 */

import { deepStrictEqual, strictEqual } from 'node:assert';
import { describe, test } from 'vitest';

import { TsvRejection } from './divergence/panic_errors.ts';
import {
	is_tsv_task,
	type PreflightSweep,
	type PreflightTask,
	sweep_preflight_task
} from './preflight_sweep.ts';
import type { ParseGoal, SourceFile } from './types.ts';
import { raise_wasm_trap } from './wasm_trap_fakes.ts';

const file = (path: string, extra: Partial<SourceFile> = {}): SourceFile => ({
	path,
	content: `/* ${path} */ a;`,
	language: 'typescript',
	bytes: 10,
	...extra
});

const FILES = [file('a.ts'), file('b.ts'), file('c.ts')];

/** A row that formats every file to itself, but for the paths `on` handles. */
const row = (
	name: string,
	impl: string,
	on: Record<string, (goal: ParseGoal | undefined) => unknown> = {}
): PreflightTask => ({
	name,
	impl,
	tracking_key: `format/typescript/${impl}`,
	is_async: false,
	run: (source, _language, goal) => {
		const path = Object.keys(on).find((p) => source.includes(`/* ${p} */`));
		return path === undefined ? source : on[path]!(goal);
	}
});

const panic = (): never => {
	throw new Error('panic: index out of bounds\n  at a frame');
};

/** The sweep's sets as plain arrays, for one `deepStrictEqual`. */
const sets = (sweep: PreflightSweep) => ({
	accepted: [...sweep.accepted],
	script_only: [...sweep.script_only],
	skipped: [...sweep.skipped],
	crashes: sweep.crashes
});

describe('sweep_preflight_task', () => {
	test('a trap in the wasm row on the first input is a crash, in no accept or skip set', async () => {
		const sweep = await sweep_preflight_task(
			row('tsv-wasm', 'wasm', { 'a.ts': () => raise_wasm_trap() }),
			FILES,
			'typescript',
			null
		);
		deepStrictEqual(sets(sweep), {
			accepted: ['b.ts', 'c.ts'],
			script_only: [],
			skipped: [],
			crashes: [{ label: 'format/typescript/wasm: a.ts', error: 'RuntimeError: unreachable' }]
		});
		strictEqual(sweep.bytes, 20);
	});

	test('a panic in the native row on the last input is a crash', async () => {
		const sweep = await sweep_preflight_task(
			row('tsv', 'native', { 'c.ts': panic }),
			FILES,
			'typescript',
			null
		);
		deepStrictEqual(sets(sweep), {
			accepted: ['a.ts', 'b.ts'],
			script_only: [],
			skipped: [],
			crashes: [{ label: 'format/typescript/native: c.ts', error: 'panic: index out of bounds' }]
		});
	});

	test('the engine both rows share crashing crashes both rows, which still agree on the rest', async () => {
		const native = await sweep_preflight_task(
			row('tsv', 'native', { 'b.ts': panic }),
			FILES,
			'typescript',
			null
		);
		const wasm = await sweep_preflight_task(
			row('tsv-wasm', 'wasm', { 'b.ts': () => raise_wasm_trap() }),
			FILES,
			'typescript',
			null
		);
		deepStrictEqual(
			[...native.crashes, ...wasm.crashes].map((c) => c.label),
			['format/typescript/native: b.ts', 'format/typescript/wasm: b.ts']
		);
		deepStrictEqual([...native.accepted], ['a.ts', 'c.ts']);
		deepStrictEqual([...wasm.accepted], ['a.ts', 'c.ts']);
		strictEqual(native.skipped.size + wasm.skipped.size, 0);
	});

	test('an async row crashes as a sync one does', async () => {
		const task: PreflightTask = {
			...row('tsv-forced-async', 'native'),
			is_async: true,
			run_async: async (source) => (source.includes('/* b.ts */') ? panic() : source)
		};
		const sweep = await sweep_preflight_task(task, FILES, 'typescript', null);
		deepStrictEqual(
			sweep.crashes.map((c) => c.label),
			['format/typescript/native: b.ts']
		);
		deepStrictEqual([...sweep.accepted], ['a.ts', 'c.ts']);
	});

	test('a rejection is a skip with its message, a `panic: ` text and a `TypeError` included', async () => {
		const sweep = await sweep_preflight_task(
			row('tsv', 'native', {
				'a.ts': () => {
					throw new TsvRejection('panic: is a name here\n  at 1:1');
				},
				'c.ts': () => {
					throw new TypeError('panic: an argument refusal');
				}
			}),
			FILES,
			'typescript',
			null
		);
		deepStrictEqual(sets(sweep), {
			accepted: ['b.ts'],
			script_only: [],
			skipped: [
				['a.ts', 'panic: is a name here\n  at 1:1'],
				['c.ts', 'panic: an argument refusal']
			],
			crashes: []
		});
	});

	test("a third-party row's trap or panic is its skip, never tsv's crash", async () => {
		const sweep = await sweep_preflight_task(
			row('biome-wasm', 'biome', { 'a.ts': () => raise_wasm_trap(), 'b.ts': panic }),
			FILES,
			'typescript',
			null
		);
		deepStrictEqual(sets(sweep), {
			accepted: ['c.ts'],
			script_only: [],
			skipped: [
				['a.ts', 'unreachable'],
				['b.ts', 'panic: index out of bounds\n  at a frame']
			],
			crashes: []
		});
	});

	describe('a goal_fallback file', () => {
		const files = [file('a.ts', { goal_fallback: true }), file('b.ts', { goal_fallback: true })];

		test('accepted at script is script-only; rejected at both records the module error', async () => {
			const sweep = await sweep_preflight_task(
				row('tsv', 'native', {
					'a.ts': (goal) => {
						if (goal === 'script') return 'ok';
						throw new TsvRejection('module error');
					},
					'b.ts': (goal) => {
						throw new TsvRejection(goal === 'script' ? 'script error' : 'module error');
					}
				}),
				files,
				'typescript',
				null
			);
			deepStrictEqual(sets(sweep), {
				accepted: ['a.ts'],
				script_only: ['a.ts'],
				skipped: [['b.ts', 'module error']],
				crashes: []
			});
		});

		test('a crash on the script-goal retry is a crash, labeled with the goal', async () => {
			const sweep = await sweep_preflight_task(
				row('tsv-wasm', 'wasm', {
					'a.ts': (goal) => {
						if (goal === 'script') raise_wasm_trap('divide');
						throw new TsvRejection('module error');
					}
				}),
				files,
				'typescript',
				null
			);
			deepStrictEqual(sets(sweep), {
				accepted: ['b.ts'],
				script_only: [],
				skipped: [],
				crashes: [
					{
						label: 'format/typescript/wasm (script goal): a.ts',
						error: 'RuntimeError: divide by zero'
					}
				]
			});
		});

		test('a crash at module is not retried at script', async () => {
			const goals: Array<ParseGoal | undefined> = [];
			const sweep = await sweep_preflight_task(
				row('tsv', 'native', {
					'a.ts': (goal) => {
						goals.push(goal);
						return goal === 'script' ? 'ok' : panic();
					}
				}),
				files,
				'typescript',
				null
			);
			deepStrictEqual(goals, [undefined]);
			deepStrictEqual(
				sweep.crashes.map((c) => c.label),
				['format/typescript/native: a.ts']
			);
			deepStrictEqual([...sweep.accepted], ['b.ts']);
		});
	});

	test('an empty output is a skip, and digests cover the accepted outputs only', async () => {
		const sweep = await sweep_preflight_task(
			row('tsv', 'native', { 'a.ts': () => ' \n', 'b.ts': () => ({ ungraded: true }) }),
			FILES,
			'typescript',
			(result) => (typeof result === 'string' ? `digest of ${result.length}` : null)
		);
		deepStrictEqual([...sweep.accepted], ['b.ts', 'c.ts']);
		deepStrictEqual([...sweep.skipped.keys()], ['a.ts']);
		deepStrictEqual([...sweep.digests!], [['c.ts', `digest of ${FILES[2]!.content.length}`]]);
		deepStrictEqual(sweep.ungraded, { count: 1, example: 'b.ts' });
	});

	test("the sweep's quiet rules: bytes, a nullish product, an ungraded row, a repeated path", async () => {
		const graded = await sweep_preflight_task(
			row('tsv', 'native', { 'a.ts': () => ' \n', 'b.ts': () => null }),
			FILES,
			'typescript',
			(result) => (typeof result === 'string' ? 'digest' : null)
		);
		// an empty-output skip adds no bytes; a nullish product is accepted, undigested, and
		// not counted as ungraded (the `-internal` shape)
		strictEqual(graded.bytes, 20);
		deepStrictEqual([...graded.accepted], ['b.ts', 'c.ts']);
		deepStrictEqual([...graded.digests!], [['c.ts', 'digest']]);
		strictEqual(graded.ungraded, null);
		// a row given no digest keeps no digest map
		const ungraded = await sweep_preflight_task(row('tsv', 'native'), FILES, 'typescript', null);
		strictEqual(ungraded.digests, null);
		// the first verdict on a path listed twice stands
		let calls = 0;
		const twice = await sweep_preflight_task(
			row('tsv', 'native', {
				'a.ts': () => {
					throw new TsvRejection(`rejection ${++calls}`);
				}
			}),
			[file('a.ts'), file('a.ts')],
			'typescript',
			null
		);
		deepStrictEqual([...twice.skipped], [['a.ts', 'rejection 1']]);
	});
});

test('is_tsv_task: the native and wasm slots, nothing else', () => {
	deepStrictEqual(
		['native', 'wasm', 'canonical', 'oxc', 'oxc_wasm', 'biome', 'yuku_wasm'].map((impl) =>
			is_tsv_task({ impl })
		),
		[true, true, false, false, false, false, false]
	);
});
