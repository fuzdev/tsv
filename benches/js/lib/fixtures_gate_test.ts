/**
 * Tests for the fixtures gates' engine (`fixtures_gate.ts`) over an in-memory suite and fake
 * engines: discovery's order and pruning, the empty-suite failure and its `--json` report, an
 * unexpected over-rejection, a full-suite run's pins, and an engine that failed to load.
 * Package-free, so it gates in `npm run test:node`.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { afterEach, test, vi } from 'vitest';

import type { CompareImplementations, CompareReadDir } from './compare_cli.ts';
import {
	type FixturesGateConfig,
	type FixturesGateDeps,
	run_fixtures_gate
} from './fixtures_gate.ts';

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

/** Fake engines: tsv rejects a source in `tsv_rejects`, and both parse to the same tree. */
const fake_impls = (parsed: string[], tsv_rejects = new Set<string>()) => {
	const state = { disposed: 0 };
	const impls: CompareImplementations = {
		canonical: {
			format_async: async (s) => s,
			parse: () => ({ type: 'Program' }),
			enable_format_cache: () => null
		},
		native: {
			format: (s) => s,
			parse: (source) => {
				parsed.push(source);
				if (tsv_rejects.has(source)) throw new Error('Unexpected token\n  at 1:1');
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
