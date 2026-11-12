/**
 * `run_corpus_audit` and `run_idempotency_sweep` over fakes for the corpus seeds, the
 * `../prettier` checkout, and `cargo`: the argv of every cargo call, which legs take which seeds, the
 * report-only leg, and the exit code for a failed build, a failed leg, a signal, and an empty
 * corpus. Package-free, so it gates in `npm run test:node`.
 */

import { deepStrictEqual, strictEqual } from 'node:assert';
import { afterEach, beforeEach, describe, test, vi } from 'vitest';

import { type CorpusAuditDeps, run_corpus_audit } from './corpus_audit.ts';
import { type CargoStatus, run_idempotency_sweep } from './idempotency_sweep.ts';

let err: string[];
beforeEach(() => {
	err = [];
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void err.push(a.join(' ')));
});
afterEach(() => {
	vi.restoreAllMocks();
});

const SEEDS = {
	dirs: ['../corpora', 'benches/js/.cache/svelte_styles'],
	live_files: ['../zzz/a.ts']
};
const REAL = [...SEEDS.dirs, ...SEEDS.live_files];
const RUN = [
	'run',
	'--profile',
	'corpus',
	'-q',
	'-p',
	'tsv_debug',
	'--features',
	'_internal_audits',
	'--'
];

/** A `cargo` answering each call by its first audit argument (`build` for the build). */
const fake_cargo = (answers: Record<string, CargoStatus> = {}) => {
	const calls: Array<ReadonlyArray<string>> = [];
	return {
		calls,
		run: async (args: ReadonlyArray<string>): Promise<CargoStatus> => {
			calls.push(args);
			const key = args[0] === 'build' ? 'build' : `${args[RUN.length]} ${args[RUN.length + 1]}`;
			return answers[key] ?? { kind: 'exited', code: 0 };
		}
	};
};

describe('audit:corpus', () => {
	const fake_deps = (
		options: {
			seeds?: typeof SEEDS;
			present?: string[];
			answers?: Record<string, CargoStatus>;
		} = {}
	) => {
		const cargo = fake_cargo(options.answers);
		const present = options.present ?? [
			'tests/format/typescript',
			'tests/format/js',
			'tests/format/css'
		];
		const deps: CorpusAuditDeps = {
			seeds: async () => options.seeds ?? SEEDS,
			prettier: { exists: async (path) => present.includes(path) },
			cargo
		};
		return { deps, cargo };
	};

	test('builds once, then runs each leg over its seeds', async () => {
		const run = fake_deps({ present: ['tests/format/js'] });
		strictEqual(await run_corpus_audit(run.deps), 0);
		const suites = ['../prettier/tests/format/js'];
		const all = [...REAL, ...suites];
		deepStrictEqual(run.cargo.calls, [
			['build', '--profile', 'corpus', '-q', '-p', 'tsv_debug', '--features', '_internal_audits'],
			[...RUN, 'roundtrip_audit', '--gate', ...all],
			[...RUN, 'comment_audit', ...all],
			[...RUN, 'swallow_audit', ...all],
			[...RUN, 'binding_audit', '--gate', ...REAL],
			[...RUN, 'binding_audit', '--gate', ...suites],
			[...RUN, 'authoring_audit', ...REAL],
			[...RUN, 'paren_audit', ...REAL],
			[...RUN, 'census_audit', ...all],
			[...RUN, 'fabrication_audit', ...all],
			[...RUN, 'fuzz', '--iterations', '0', ...REAL]
		]);
		deepStrictEqual(err.slice(0, 2), [
			'  ⚠ prettier suite missing, skipped: ../prettier/tests/format/typescript',
			'  ⚠ prettier suite missing, skipped: ../prettier/tests/format/css'
		]);
		strictEqual(
			err.at(-1),
			'\n✓ corpus audit clean — no content-loss / panic / non-idempotency over real code.'
		);
	});

	test('a failed gating leg fails the run after every leg ran; a report-only one does not', async () => {
		const failing = fake_deps({
			answers: {
				'comment_audit ../corpora': { kind: 'exited', code: 1 },
				'binding_audit --gate': { kind: 'signaled' }
			}
		});
		strictEqual(await run_corpus_audit(failing.deps), 1);
		strictEqual(failing.cargo.calls.length, 11);
		deepStrictEqual(
			err.filter((l) => l.includes('FAILED') || l.includes('report-only leg')),
			[
				'  ✗ comment_audit FAILED (exit 1)',
				'  ✗ binding_audit --gate (real code) FAILED (exit null)',
				'  (report-only leg exited null — not gating)'
			]
		);
		strictEqual(err.at(-1), '\n✗ corpus audit found a robustness regression over real code.');
	});

	test('no prettier suite skips the report-only leg; no real code fails before any build', async () => {
		const run = fake_deps({ present: [] });
		strictEqual(await run_corpus_audit(run.deps), 0);
		strictEqual(run.cargo.calls.length, 10);
		strictEqual(
			err.includes('\n─ binding_audit --gate (prettier suites): SKIPPED (no seeds present)'),
			true
		);
		const empty = fake_deps({ seeds: { dirs: [], live_files: [] } });
		strictEqual(await run_corpus_audit(empty.deps), 1);
		deepStrictEqual(empty.cargo.calls, []);
		strictEqual(err.at(-1), 'Error: no real-code corpus present — nothing to audit.');
	});

	test("a failed build ends the run with the build's code, 1 for a signal", async () => {
		for (const [status, code] of [
			[{ kind: 'exited', code: 101 }, 101],
			[{ kind: 'signaled' }, 1]
		] as const) {
			const run = fake_deps({ answers: { build: status } });
			strictEqual(await run_corpus_audit(run.deps), code);
			strictEqual(run.cargo.calls.length, 1);
			strictEqual(err.at(-1), 'Error: build failed.');
		}
	});
});

describe('idempotency:sweep', () => {
	test('one cargo run over every seed, exiting with its code', async () => {
		for (const [status, code] of [
			[{ kind: 'exited', code: 0 }, 0],
			[{ kind: 'exited', code: 3 }, 3],
			[{ kind: 'signaled' }, 1]
		] as const) {
			err = [];
			const calls: Array<ReadonlyArray<string>> = [];
			strictEqual(
				await run_idempotency_sweep({
					seeds: async () => SEEDS,
					cargo: { run: async (args) => (calls.push(args), status) }
				}),
				code
			);
			deepStrictEqual(calls, [
				[
					'run',
					'--profile',
					'corpus',
					'-q',
					'-p',
					'tsv_debug',
					'--',
					'fuzz',
					'--iterations',
					'0',
					...REAL
				]
			]);
			deepStrictEqual(err, ['idempotency sweep over 2 directory seeds + 1 live-diff files\n']);
		}
	});

	test('no corpus fails without running cargo', async () => {
		let ran = false;
		strictEqual(
			await run_idempotency_sweep({
				seeds: async () => ({ dirs: [], live_files: [] }),
				cargo: { run: async () => ((ran = true), { kind: 'exited', code: 0 }) }
			}),
			1
		);
		strictEqual(ran, false);
		deepStrictEqual(err, ['Error: no corpus present — nothing to sweep.']);
	});
});
