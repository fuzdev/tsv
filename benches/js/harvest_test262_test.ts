/**
 * `run_harvest_test262` over fakes for `git`, the checkout's files, `../test262`, and `cargo`:
 * the bytes of the path list and the stamp it writes for a small manifest, the stamp's
 * freshness rule, the cargo run it makes, and every refusal, each of which writes nothing.
 * Package-free, so it gates in `npm run test:node`.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { afterEach, beforeEach, test, vi } from 'vitest';

import {
	run_harvest_test262,
	TEST262_FILES_PATH,
	TEST262_MANIFEST_PATH,
	type HarvestTest262Deps,
	type Test262ManifestEntry,
	test262_positive_files
} from './harvest_test262.ts';
import { GATE_CHECKOUT_IDS, TEST262_POSITIVES_PIN } from './lib/gate_counts.ts';
import {
	fake_files,
	fake_id,
	fake_stamp_git,
	measured_id,
	SKEW_TAIL
} from './lib/harvest_fakes.ts';
import { HARVEST_STAMPS } from './lib/harvest_stamp_table.ts';

const STAMP_PATH = HARVEST_STAMPS.test262.path;
const COMMIT = fake_id('test262');

let err: string[];
beforeEach(() => {
	err = [];
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void err.push(a.join(' ')));
});
afterEach(() => {
	vi.restoreAllMocks();
});

/** `positives` accepted rows (every third module-flagged), two rejected ones, in a shuffled
 * order whose names sort differently by locale than by code point. */
const manifest_text = (positives = TEST262_POSITIVES_PIN): string => {
	const names = ['b', 'B', '_u', 'a-b', 'a_b', 'é'];
	const tests: Test262ManifestEntry[] = [
		...Array.from({ length: positives }, (_, i) => ({
			relative_path: `test/${names[i % names.length]}${positives - i}.js`,
			module: i % 3 === 0,
			strict: false,
			expected: 'accept' as const,
			tsv: 'accept' as const
		})),
		{
			relative_path: 'test/neg1.js',
			module: true,
			strict: false,
			expected: 'reject',
			tsv: 'reject'
		},
		{
			relative_path: 'test/neg2.js',
			module: false,
			strict: true,
			expected: 'reject',
			tsv: 'accept'
		}
	];
	return JSON.stringify({ test262_root: '../test262', count: tests.length, tests });
};

/** The stamp's bytes as the harvest writes them, spelled out. */
const stamp_text = (commit: string): string =>
	`{\n\t"harvest": "test262",\n\t"source_commit": "${commit}",\n\t"positives_pin": ${TEST262_POSITIVES_PIN}\n}\n`;

interface FakeOptions {
	present?: boolean;
	commit?: string | null;
	files?: Record<string, string>;
	cargo_code?: number | 'signaled';
	manifest?: string;
}

const fake_deps = (options: FakeOptions = {}) => {
	const files = fake_files(options.files);
	const git = fake_stamp_git(
		options.commit === null ? {} : { '../test262': options.commit ?? COMMIT }
	);
	const cargo_calls: Array<ReadonlyArray<string>> = [];
	const test262_reads: string[] = [];
	const deps: HarvestTest262Deps = {
		git,
		files,
		test262: {
			exists: async (path) => {
				test262_reads.push(path);
				return options.present ?? true;
			}
		},
		cargo: {
			run: async (args) => {
				cargo_calls.push(args);
				if (options.cargo_code === 'signaled') return { kind: 'signaled' };
				const code = options.cargo_code ?? 0;
				// the grade writes the manifest itself
				if (code === 0) files.files.set(TEST262_MANIFEST_PATH, options.manifest ?? manifest_text());
				return { kind: 'exited', code };
			}
		}
	};
	return { deps, files, git, cargo_calls, test262_reads };
};

test('the positives become `{path, goal}` entries, sorted by locale', () => {
	const files = test262_positive_files(JSON.parse(manifest_text(6)));
	deepStrictEqual(files, [
		{ path: '../test262/test/_u4.js', goal: 'script' },
		{ path: '../test262/test/a_b2.js', goal: 'script' },
		{ path: '../test262/test/a-b3.js', goal: 'module' },
		{ path: '../test262/test/B5.js', goal: 'script' },
		{ path: '../test262/test/b6.js', goal: 'module' },
		{ path: '../test262/test/é1.js', goal: 'script' }
	]);
});

test('a stale stamp grades through cargo and writes the list and the stamp, byte for byte', async () => {
	const { deps, files, cargo_calls } = fake_deps({
		files: { [STAMP_PATH]: stamp_text(fake_id('older')) }
	});
	strictEqual(await run_harvest_test262(deps, ['--if-present']), 0);
	deepStrictEqual(cargo_calls, [
		[
			'run',
			'--release',
			'-p',
			'tsv_debug',
			'--',
			'test262',
			'--path',
			'../test262',
			'--emit-manifest',
			'benches/js/.cache/test262_manifest.json'
		]
	]);
	deepStrictEqual(files.log, [
		'mkdir benches/js/.cache',
		`write ${TEST262_FILES_PATH}`,
		`write ${STAMP_PATH}`
	]);
	const list = files.files.get(TEST262_FILES_PATH)!;
	// tab-indented, no trailing newline, as the cache has always been written
	ok(list.startsWith('[\n\t{\n\t\t"path": "../test262/test/_u'));
	ok(list.endsWith('",\n\t\t"goal": "script"\n\t}\n]'));
	strictEqual(JSON.parse(list).length, TEST262_POSITIVES_PIN);
	strictEqual(files.files.get(STAMP_PATH), stamp_text(COMMIT));
	strictEqual(
		err.at(-1),
		`test262 harvest: ${TEST262_POSITIVES_PIN} expected-positive of ${TEST262_POSITIVES_PIN + 2} graded tests ` +
			`(${Math.ceil(TEST262_POSITIVES_PIN / 3)} module-flagged) → ${TEST262_FILES_PATH}`
	);
});

test('a fresh stamp skips the grade with its line; --force grades past it', async () => {
	const fresh = {
		[STAMP_PATH]: stamp_text(COMMIT),
		[TEST262_FILES_PATH]: '[]',
		[TEST262_MANIFEST_PATH]: '{}'
	};
	const skipped = fake_deps({ files: fresh });
	strictEqual(await run_harvest_test262(skipped.deps, []), 0);
	deepStrictEqual(skipped.cargo_calls, []);
	deepStrictEqual(skipped.files.log, ['mkdir benches/js/.cache']);
	deepStrictEqual(err, [
		`test262 harvest up to date (../test262 at ${COMMIT.slice(0, 9)}, pin ${TEST262_POSITIVES_PIN}) — skipping; --force to re-grade.`
	]);
	deepStrictEqual(skipped.git.calls, [['-C', '../test262', 'rev-parse', 'HEAD']]);

	const forced = fake_deps({ files: fresh });
	strictEqual(await run_harvest_test262(forced.deps, ['--force']), 0);
	strictEqual(forced.cargo_calls.length, 1);
});

test('a missing cache file reads stale even under a matching stamp', async () => {
	const { deps, cargo_calls } = fake_deps({
		files: { [STAMP_PATH]: stamp_text(COMMIT), [TEST262_FILES_PATH]: '[]' }
	});
	strictEqual(await run_harvest_test262(deps, []), 0);
	strictEqual(cargo_calls.length, 1);
});

test('a missing checkout warn-skips under --if-present and fails otherwise, touching nothing', async () => {
	for (const [argv, code, line] of [
		[
			['--if-present'],
			0,
			'test262 checkout not found at ../test262 — skipping harvest (--if-present)'
		],
		[[], 1, 'test262 checkout not found: ../test262'],
		[['--force'], 1, 'test262 checkout not found: ../test262']
	] as const) {
		err = [];
		const { deps, files, git, cargo_calls, test262_reads } = fake_deps({ present: false });
		strictEqual(await run_harvest_test262(deps, argv), code);
		deepStrictEqual(err, [line]);
		deepStrictEqual(test262_reads, ['test']);
		deepStrictEqual(files.log, []);
		deepStrictEqual(git.calls, []);
		deepStrictEqual(cargo_calls, []);
	}
});

test('a failed grade writes nothing', async () => {
	for (const cargo_code of [101, 'signaled'] as const) {
		err = [];
		const { deps, files } = fake_deps({ cargo_code });
		strictEqual(await run_harvest_test262(deps, []), 1);
		deepStrictEqual(files.log, ['mkdir benches/js/.cache']);
		strictEqual(err.at(-1), 'tsv_debug test262 --emit-manifest failed');
	}
});

test('a positive count off the pin at the measured commit writes nothing, naming the re-pin', async () => {
	const { deps, files } = fake_deps({
		manifest: manifest_text(TEST262_POSITIVES_PIN - 1),
		commit: measured_id('../test262')
	});
	strictEqual(await run_harvest_test262(deps, []), 1);
	deepStrictEqual(files.log, ['mkdir benches/js/.cache']);
	strictEqual(
		err.at(-1),
		`FAIL: pinned count mismatch — ${TEST262_POSITIVES_PIN - 1} expected-positive ≠ pinned ${TEST262_POSITIVES_PIN}; ` +
			'cache not written. If the move is deliberate, re-pin in lib/gate_counts.ts.'
	);
});

test('a positive count off the pin in a checkout off its measured commit says so, not the re-pin', async () => {
	const { deps } = fake_deps({ manifest: manifest_text(TEST262_POSITIVES_PIN - 1) });
	strictEqual(await run_harvest_test262(deps, []), 1);
	strictEqual(
		err.at(-1),
		`FAIL: pinned count mismatch — ${TEST262_POSITIVES_PIN - 1} expected-positive ≠ pinned ${TEST262_POSITIVES_PIN}; ` +
			`cache not written. ../test262 is at ${COMMIT.slice(0, 9)}, not ` +
			`${GATE_CHECKOUT_IDS['../test262']!.hash}${SKEW_TAIL}`
	);
});

test('a checkout with no commit grades and writes the list but no stamp', async () => {
	const { deps, files, cargo_calls } = fake_deps({ commit: null });
	strictEqual(await run_harvest_test262(deps, []), 0);
	strictEqual(cargo_calls.length, 1);
	deepStrictEqual(files.log, ['mkdir benches/js/.cache', `write ${TEST262_FILES_PATH}`]);
});
