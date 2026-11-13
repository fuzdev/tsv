/**
 * Tests for the TypeScript-repo gate (`ts_repo_compare.ts`) over an in-memory checkout and a
 * fake tsv: a tsv crash, at either goal, on a unit tsc's baselines accept or refuse, is graded
 * in no bucket and fails the run, listed with its engine and unit; a rejection tsv returned
 * stays a rejection whatever its text. Package-free but for tsc, a root dependency, so it gates
 * in `npm run test:node`.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { afterEach, test, vi } from 'vitest';

import type { CompareImplementations, CompareReadDir } from '../lib/compare_cli.ts';
import { TsvRejection } from '../lib/divergence/panic_errors.ts';
import { raise_wasm_trap } from '../lib/wasm_trap_fakes.ts';
import { run_ts_repo_compare, type TsRepoCompareDeps } from './ts_repo_compare.ts';

afterEach(() => {
	vi.restoreAllMocks();
});

const capture_console = (): string[] => {
	const lines: string[] = [];
	vi.spyOn(console, 'log').mockImplementation((...a: unknown[]) => void lines.push(a.join(' ')));
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void lines.push(a.join(' ')));
	return lines;
};

/** A read-only directory over `files` (path → text). */
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
		for (const [name, kind] of names) yield { name, kind };
	},
	read_text: async (path) => {
		const text = files[path];
		if (text === undefined) throw new Error(`not_found: ${path}`);
		return text;
	}
});

/** A subtree run (no ledger freshness or pins): `a.ts` first, `z.ts` last. */
const ROOT = '../typescript/tests/cases/fake';

/** Each case's source is its own name, so the fake engines can key on it. */
const CASES: Record<string, string> = {
	'a.ts': 'export const a = 1; // a',
	'b.ts': 'export const b = 1; // b',
	// tsc reads it as a script: no import or export
	'script.ts': 'await (1); // script',
	// a ledgered path (`KNOWN_GAPS`'s beyond-acorn entry)
	'arbitraryModuleNamespaceIdentifiers_module.ts': 'export {}; // ledgered',
	'z.ts': 'export const z = 1; // z'
};

/** tsc's baselines: `z` carries a grammar error, so tsc refuses it; the rest are valid. */
const BASELINES: Record<string, string> = {
	'z.errors.txt': 'z.ts(1,1): error TS1005: expected.\n'
};

/** How the fake tsv fails on a source, at the module goal or the script goal. */
type FakeFailure = 'reject' | 'reject_panic_text' | 'panic' | 'trap';

/** Throws as tsv's binding does for `failure`. */
const fail = (failure: FakeFailure | undefined): void => {
	if (failure === 'trap') raise_wasm_trap();
	if (failure === 'panic') throw new Error('panic: boom\n  at a frame');
	if (failure === 'reject') throw new TsvRejection('Unexpected token\n  at 1:1');
	if (failure === 'reject_panic_text') throw new TsvRejection('panic: boom\n  at a frame');
};

/** Fake engines: tsv fails per `module` / `script` (keyed by source), acorn accepts all. */
const fake_impls = (
	module: Record<string, FakeFailure>,
	script: Record<string, FakeFailure> = {}
): { state: { disposed: number }; impls: CompareImplementations } => {
	const name_of = (source: string): string =>
		Object.entries(CASES).find(([, content]) => content === source)?.[0] ?? '';
	const state = { disposed: 0 };
	const impls: CompareImplementations = {
		canonical: {
			format_async: async (s) => s,
			parse: (source) => {
				if (source.includes('ledgered')) throw new SyntaxError('Unexpected token (1:1)');
				return { type: 'Program' };
			},
			enable_format_cache: () => null
		},
		native: {
			format: (s) => s,
			parse: () => ({ type: 'Program' }),
			parse_internal: (source, _language, goal) => {
				const name = name_of(source);
				fail(goal === 'script' ? script[name] : module[name]);
			}
		},
		dispose: () => void state.disposed++
	};
	return { state, impls };
};

const deps = (impls: CompareImplementations, written: string[] = []): TsRepoCompareDeps => ({
	init_implementations: async () => impls,
	open_dir: async (path) => {
		if (path === '../typescript') return memory_dir({ 'package.json': '{}' });
		if (path === '../typescript/tests/baselines/reference') return memory_dir(BASELINES);
		if (path === ROOT) return memory_dir(CASES);
		throw new Error(`not_found: ${path}`);
	},
	stdout: { write: async (data: string) => written.push(data) }
});

/** The run's FAIL block's entries, after its headline. */
const panic_entries = (lines: string[]): string[] => {
	const at = lines.findIndex((l) => /^FAIL: \d+ panic\(s\)/.test(l));
	ok(at > 0, lines.join('\n'));
	return lines.slice(at + 1);
};

test('a tsv crash, first and last, on units tsc accepts and refuses, fails in no bucket', async () => {
	const lines = capture_console();
	const written: string[] = [];
	const { state, impls } = fake_impls({
		'a.ts': 'trap',
		'arbitraryModuleNamespaceIdentifiers_module.ts': 'panic',
		'z.ts': 'panic'
	});
	strictEqual(await run_ts_repo_compare(deps(impls, written), [ROOT, '--json']), 1);
	strictEqual(state.disposed, 1);
	deepStrictEqual(panic_entries(lines), [
		`  tsv: ${ROOT}/a.ts: RuntimeError: unreachable`,
		`  tsv: ${ROOT}/arbitraryModuleNamespaceIdentifiers_module.ts: panic: boom`,
		`  tsv: ${ROOT}/z.ts: panic: boom`
	]);
	ok(!lines.some((l) => l.startsWith('OK:')));
	const report = JSON.parse(written.join('')) as Record<string, unknown> & {
		beyond_acorn: Record<string, unknown>;
	};
	strictEqual(report.accept_parity, 2);
	strictEqual(report.reject_parity, 0);
	deepStrictEqual(report.gap_unexpected, []);
	deepStrictEqual(report.beyond_acorn.known, []);
	deepStrictEqual(report.beyond_acorn.unexpected, []);
});

test('a crash at the script goal fails the run, never a script-goal artifact', async () => {
	const lines = capture_console();
	const written: string[] = [];
	const { impls } = fake_impls({ 'script.ts': 'reject' }, { 'script.ts': 'panic' });
	strictEqual(await run_ts_repo_compare(deps(impls, written), [ROOT, '--json']), 1);
	deepStrictEqual(panic_entries(lines), [`  tsv (script goal): ${ROOT}/script.ts: panic: boom`]);
	const report = JSON.parse(written.join('')) as Record<string, unknown>;
	deepStrictEqual(report.script_goal, []);
	deepStrictEqual(report.gap_unexpected, []);
});

test('a crash fails a run whose buckets alone would pass', async () => {
	const lines = capture_console();
	// tsc refuses `z`: without the classifier its crash reads as reject parity
	const { impls } = fake_impls({ 'z.ts': 'trap' });
	strictEqual(await run_ts_repo_compare(deps(impls), [ROOT]), 1);
	deepStrictEqual(panic_entries(lines), [`  tsv: ${ROOT}/z.ts: RuntimeError: unreachable`]);
	ok(lines.includes('  parity reject (tsv + tsc both reject):  0'), lines.join('\n'));
});

test('a rejection tsv returned is a rejection whatever its text, at either goal', async () => {
	const lines = capture_console();
	const { impls } = fake_impls(
		{ 'script.ts': 'reject_panic_text', 'z.ts': 'reject_panic_text' },
		{ 'script.ts': 'reject_panic_text' }
	);
	// the script's over-rejection is untracked, which fails the run, but no crash is listed
	strictEqual(await run_ts_repo_compare(deps(impls), [ROOT]), 1);
	ok(lines.includes('  parity reject (tsv + tsc both reject):  1'), lines.join('\n'));
	ok(!lines.some((l) => l.includes('panic(s)')), lines.join('\n'));
	ok(lines.some((l) => l.includes(`UNTRACKED gap (acorn accepts): ${ROOT}/script.ts`)));
});

test('a run with no crash is the gate it was', async () => {
	const lines = capture_console();
	const { impls } = fake_impls({ 'z.ts': 'reject' });
	strictEqual(await run_ts_repo_compare(deps(impls), [ROOT]), 0);
	ok(lines.includes('  parity reject (tsv + tsc both reject):  1'), lines.join('\n'));
	ok(!lines.some((l) => l.includes('panic(s)')));
});
