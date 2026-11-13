/**
 * Tests for the fixtures gates' engine (`fixtures_gate.ts`) over an in-memory suite and fake
 * engines: discovery's order and pruning, the empty-suite failure and its `--json` report, an
 * unexpected over-rejection, a full-suite run's pins, an engine that failed to load, and a tsv
 * crash, graded in no bucket and failing the gate whatever the oracle said.
 * Package-free, so it gates in `npm run test:node`.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { afterEach, test, vi } from 'vitest';

import type { CompareImplementations, CompareReadDir } from './compare_cli.ts';
import { TsvRejection } from './divergence/panic_errors.ts';
import {
	type FixturesGateConfig,
	type FixturesGateDeps,
	run_fixtures_gate
} from './fixtures_gate.ts';
import { raise_wasm_trap } from './wasm_trap_fakes.ts';

afterEach(() => {
	vi.restoreAllMocks();
});

const capture_console = (): string[] => {
	const lines: string[] = [];
	vi.spyOn(console, 'log').mockImplementation((...a: unknown[]) => void lines.push(a.join(' ')));
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void lines.push(a.join(' ')));
	return lines;
};

/** A read-only directory over `files` (path → text), listing each directory in reverse order. */
const memory_dir = (files: Record<string, string>): CompareReadDir => ({
	list: async function* (path = '') {
		const prefix = path === '' ? '' : `${path}/`;
		const names = new Map<string, string>();
		for (const file of Object.keys(files)) {
			if (!file.startsWith(prefix)) continue;
			const [name, ...rest] = file.slice(prefix.length).split('/');
			names.set(name!, rest.length > 0 ? 'directory' : 'file');
		}
		if (names.size === 0) throw new Error(`not_found: ${path}`);
		for (const [name, kind] of [...names].reverse()) yield { name, kind };
	},
	read_text: async (path) => {
		const text = files[path];
		if (text === undefined) throw new Error(`not_found: ${path}`);
		return text;
	}
});

const CONFIG: FixturesGateConfig = {
	title: 'Test-fixtures',
	language: 'typescript',
	default_root: '../suite',
	input_basenames: new Set(['input.ts']),
	input_noun: '.ts inputs',
	prune_dir: (name) => name.startsWith('_'),
	sanctioned: [],
	sanctioned_note: 'none',
	known_gaps: [],
	oracle_name: 'the oracle',
	pins: { scanned: 3, both_accept: 3, over_acceptance: 0 }
};

const SUITE = {
	'b/input.ts': 'b',
	'a/input.ts': 'a',
	'a/other.ts': 'skipped: not an input basename',
	'_generated/input.ts': 'pruned',
	'c/d/input.ts': 'd'
};

/** How the fake tsv fails on a source: a returned rejection, a caught native panic, a trap. */
type FakeFailure = 'reject' | 'panic' | 'trap';

/**
 * Fake engines: tsv fails on a source in `tsv_fails` as it says, the oracle rejects a source in
 * `oracle_rejects`, and both parse the rest to the same tree.
 */
const fake_impls = (
	parsed: string[],
	tsv_fails: Iterable<string> | Map<string, FakeFailure> = [],
	oracle_rejects = new Set<string>()
) => {
	const fails =
		tsv_fails instanceof Map
			? tsv_fails
			: new Map([...tsv_fails].map((s): [string, FakeFailure] => [s, 'reject']));
	const state = { disposed: 0 };
	const impls: CompareImplementations = {
		canonical: {
			format_async: async (s) => s,
			parse: (source) => {
				if (oracle_rejects.has(source)) throw new SyntaxError('Unexpected token (1:1)');
				return { type: 'Program' };
			},
			enable_format_cache: () => null
		},
		native: {
			format: (s) => s,
			parse: (source) => {
				parsed.push(source);
				const failure = fails.get(source);
				if (failure === 'trap') raise_wasm_trap();
				if (failure === 'panic') throw new Error('panic: boom\n  at a frame');
				if (failure === 'reject') throw new TsvRejection('Unexpected token\n  at 1:1');
				return { type: 'Program' };
			},
			parse_internal: () => {}
		},
		dispose: () => void state.disposed++
	};
	return { state, impls };
};

const deps = (impls: CompareImplementations | null, written: string[] = []): FixturesGateDeps => ({
	init_implementations: async () => impls,
	open_dir: async (path) => {
		if (path !== '../suite') throw new Error(`not_found: ${path}`);
		return memory_dir(SUITE);
	},
	stdout: { write: async (data: string) => written.push(data) }
});

test('the suite is walked in readdir order, pruned, and graded green against its pins', async () => {
	const lines = capture_console();
	const parsed: string[] = [];
	const { state, impls } = fake_impls(parsed);
	strictEqual(await run_fixtures_gate(deps(impls), CONFIG, []), 0);
	deepStrictEqual(parsed, ['a', 'b', 'd']);
	strictEqual(state.disposed, 1);
	ok(lines.includes('  scanned: 3 .ts inputs\n'));
});

test('an unexpected over-rejection fails the gate and names the file', async () => {
	const lines = capture_console();
	const { impls } = fake_impls([], new Set(['b']));
	strictEqual(await run_fixtures_gate(deps(impls), CONFIG, []), 1);
	ok(lines.some((l) => l.includes('UNEXPECTED over-rejection: ../suite/b/input.ts')));
	ok(lines.some((l) => l.includes('FAIL: 1 unexpected over-rejection(s)')));
});

test('a full-suite run fails on a pinned count that moved', async () => {
	const lines = capture_console();
	const { impls } = fake_impls([]);
	const moved = { ...CONFIG, pins: { ...CONFIG.pins, scanned: 4 } };
	strictEqual(await run_fixtures_gate(deps(impls), moved, []), 1);
	ok(lines.some((l) => l.includes('scanned 3 ≠ pinned 4')));
});

test('an empty suite fails, and --json writes the zero report', async () => {
	const lines = capture_console();
	const written: string[] = [];
	const { state, impls } = fake_impls([]);
	strictEqual(await run_fixtures_gate(deps(impls, written), CONFIG, ['../missing', '--json']), 1);
	deepStrictEqual(written, [JSON.stringify({ root: '../missing', scanned: 0 }, null, '\t') + '\n']);
	strictEqual(state.disposed, 1);
	ok(lines[0]!.startsWith('Cannot read ../missing: not_found: ../missing'));
});

test('an engine that failed to load fails the gate', async () => {
	capture_console();
	strictEqual(await run_fixtures_gate(deps(null), CONFIG, []), 1);
});

/** The gate's `--json` report's verdict, from what it wrote to stdout. */
const verdict_of = (written: string[]): Record<string, unknown> =>
	(JSON.parse(written.join('')) as { verdict: Record<string, unknown> }).verdict;

test('a tsv crash, first and last, on inputs the oracle accepts and rejects, fails in no bucket', async () => {
	const lines = capture_console();
	const written: string[] = [];
	// `a` is the first input, `d` the last; the oracle accepts `a` and rejects `d`
	const { state, impls } = fake_impls(
		[],
		new Map<string, FakeFailure>([
			['a', 'trap'],
			['d', 'panic']
		]),
		new Set(['d'])
	);
	// a ledgered path doesn't absorb a crash either
	const ledgered: FixturesGateConfig = {
		...CONFIG,
		sanctioned: [{ pattern: 'a/input.ts', reason: 'stricter' }]
	};
	strictEqual(await run_fixtures_gate(deps(impls, written), ledgered, ['--json']), 1);
	strictEqual(state.disposed, 1);
	const at = lines.findIndex((l) => l.startsWith('FAIL: 2 panic(s)'));
	ok(at > 0, lines.join('\n'));
	deepStrictEqual(lines.slice(at + 1), [
		'  tsv: ../suite/a/input.ts: RuntimeError: unreachable',
		'  tsv: ../suite/c/d/input.ts: panic: boom'
	]);
	ok(!lines.some((l) => l.includes('OK:')));
	const verdict = verdict_of(written);
	strictEqual(verdict.parity, 0);
	strictEqual(verdict.both_accept, 1);
	deepStrictEqual(verdict.sanctioned, []);
	deepStrictEqual(verdict.unexpected, []);
});

test('a crash fails a run whose buckets alone would pass', async () => {
	const lines = capture_console();
	// `b` crashes where the oracle rejects: without the classifier, reject parity
	const { impls } = fake_impls([], new Map<string, FakeFailure>([['b', 'panic']]), new Set(['b']));
	const pins = { ...CONFIG, pins: { scanned: 3, both_accept: 2, over_acceptance: 0 } };
	strictEqual(await run_fixtures_gate(deps(impls), pins, []), 1);
	ok(lines.includes('  tsv: ../suite/b/input.ts: panic: boom'), lines.join('\n'));
	ok(lines.includes('    parity (both reject):     0'), lines.join('\n'));
});

test('a tsv rejection whose text reads `panic: ` is a rejection: the engine returned it', async () => {
	const lines = capture_console();
	const { impls } = fake_impls([], [], new Set(['b']));
	impls.native.parse = (source) => {
		if (source === 'b') throw new TsvRejection('panic: boom\n  at a frame');
		return { type: 'Program' };
	};
	const pins = { ...CONFIG, pins: { scanned: 3, both_accept: 2, over_acceptance: 0 } };
	strictEqual(await run_fixtures_gate(deps(impls), pins, []), 0);
	ok(lines.includes('    parity (both reject):     1'), lines.join('\n'));
	ok(!lines.some((l) => l.includes('panic(s)')));
});
