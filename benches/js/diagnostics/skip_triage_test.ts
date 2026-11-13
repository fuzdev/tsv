/**
 * `run_skip_triage` over a fake corpus and fake parsers: each asymmetry's bucket, the sanction
 * and known-gap catalogues, the panic bucket a crash lands in whatever the oracle says, the JSON
 * report on stdout, and the exit code an untracked over-rejection, a panic, or an engine that
 * failed to load sets. Package-free, so it gates in `npm run test:node`.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { afterEach, beforeEach, test, vi } from 'vitest';

import type { CompareImplementations } from '../lib/compare_cli.ts';
import { TsvRejection } from '../lib/divergence/panic_errors.ts';
import type { Language, SourceFile } from '../lib/types.ts';
import { raise_wasm_trap } from '../lib/wasm_trap_fakes.ts';
import { run_skip_triage, type SkipTriageDeps } from './skip_triage.ts';

let err: string[];
beforeEach(() => {
	err = [];
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void err.push(a.join(' ')));
});
afterEach(() => {
	vi.restoreAllMocks();
});

const file = (path: string, language: Language, content: string): SourceFile =>
	({ path, language, content, bytes: content.length }) as SourceFile;

/** A parser refusing a source that holds `word`, its error naming the parser. */
const refusing = (name: string, word: string) => (source: string) => {
	if (source.includes(word)) throw new Error(`${name} refuses\nsecond line`);
};

/**
 * tsv's fake: refuses `TSV`, panics on `PANIC` the way the `corpus` addon throws one, traps on
 * `TRAP` as a WASM build does, and returns a rejection reading `panic: ` on `TEXT`.
 */
const fake_tsv = (source: string) => {
	if (source.includes('PANIC')) throw new Error('panic: x\nat a frame');
	if (source.includes('TRAP')) raise_wasm_trap();
	if (source.includes('TEXT')) throw new TsvRejection('panic: quoted\nsecond line');
	refusing('tsv', 'TSV')(source);
};

const fake_deps = (files: SourceFile[], options: { loads?: boolean } = {}) => {
	const state = { loaded: [] as Array<string | undefined>, out: '', disposed: 0 };
	const engines = {
		native: { parse_internal: fake_tsv },
		canonical: { parse: refusing('canon', 'CANON') },
		dispose: () => void state.disposed++
	} as unknown as CompareImplementations;
	const deps: SkipTriageDeps = {
		load_files: async (path) => {
			state.loaded.push(path);
			return files;
		},
		init_implementations: async () => (options.loads === false ? null : engines),
		stdout: { write: async (data) => void (state.out += data) }
	};
	return { deps, state };
};

test('each asymmetry lands in its bucket, and parity alone passes', async () => {
	const { deps, state } = fake_deps([
		file('a.svelte', 'svelte', 'TSV CANON'),
		file('b.ts', 'typescript', 'CANON'),
		file('c.css', 'css', 'fine'),
		file('../prettier/tests/format/js/import-assertions/x.ts', 'typescript', 'TSV')
	]);
	strictEqual(await run_skip_triage(deps, ['some/dir']), 0);
	deepStrictEqual(state.loaded, ['some/dir']);
	const report = JSON.parse(state.out);
	deepStrictEqual(report.svelte.parity, [
		{ path: 'a.svelte', tsv_error: 'tsv refuses', canonical_error: 'canon refuses' }
	]);
	deepStrictEqual(report.typescript.over_acceptance, [{ path: 'b.ts', error: 'canon refuses' }]);
	strictEqual(report.typescript.sanctioned_over_rejection.length, 1);
	strictEqual(report.typescript.sanctioned_over_rejection[0].error, 'tsv refuses');
	deepStrictEqual(report.css, {
		unexpected_over_rejection: [],
		sanctioned_over_rejection: [],
		known_gap_over_rejection: [],
		over_acceptance: [],
		parity: [],
		panic: []
	});
	ok(err.includes('\nOK: no un-sanctioned parse over-rejections (parity holds).'));
});

test('an untracked over-rejection fails the run and is listed', async () => {
	const { deps, state } = fake_deps([file('d.css', 'css', 'TSV')]);
	strictEqual(await run_skip_triage(deps, ['--flag']), 1);
	deepStrictEqual(state.loaded, [undefined]);
	deepStrictEqual(JSON.parse(state.out).css.unexpected_over_rejection, [
		{ path: 'd.css', error: 'tsv refuses' }
	]);
	ok(err.includes('    ✗ d.css\n        tsv refuses'));
	ok(err.some((l) => l.startsWith('\nFAIL: 1 untracked over-rejection(s)')));
});

test('a known gap is reported as one and does not fail the run', async () => {
	const { deps, state } = fake_deps([
		file(
			'benches/js/.cache/wpt_css/css-animations/missing-values-middle-keyframe__0.css',
			'css',
			'TSV'
		)
	]);
	strictEqual(await run_skip_triage(deps, []), 0);
	strictEqual(JSON.parse(state.out).css.known_gap_over_rejection.length, 1);
});

test('a panic fails the run loud and is never parity, even where the oracle rejects too', async () => {
	const { deps, state } = fake_deps([
		file('p.svelte', 'svelte', 'PANIC CANON'),
		file('q.css', 'css', 'PANIC'),
		file('r.ts', 'typescript', 'TSV CANON')
	]);
	strictEqual(await run_skip_triage(deps, []), 1);
	const report = JSON.parse(state.out);
	deepStrictEqual(report.svelte.panic, [{ path: 'p.svelte', error: 'panic: x' }]);
	deepStrictEqual(report.svelte.parity, []);
	// a panic where the oracle accepts is a crash too, not an over-rejection
	deepStrictEqual(report.css.panic, [{ path: 'q.css', error: 'panic: x' }]);
	deepStrictEqual(report.css.unexpected_over_rejection, []);
	strictEqual(report.typescript.parity.length, 1);
	ok(err.some((l) => l.startsWith('\nFAIL: 2 panic(s) — tsv crashed on real code')));
	ok(err.includes('  p.svelte: panic: x'));
	ok(err.includes('  q.css: panic: x'));
	ok(!err.some((l) => l.startsWith('\nOK:')));
	strictEqual(state.disposed, 1);
});

test('a trap is a crash too, and a rejection whose text reads `panic: ` is a verdict', async () => {
	const { deps, state } = fake_deps([
		file('t.css', 'css', 'TRAP CANON'),
		file('u.css', 'css', 'TEXT CANON')
	]);
	strictEqual(await run_skip_triage(deps, []), 1);
	const report = JSON.parse(state.out);
	deepStrictEqual(report.css.panic, [{ path: 't.css', error: 'RuntimeError: unreachable' }]);
	deepStrictEqual(report.css.parity, [
		{ path: 'u.css', tsv_error: 'panic: quoted', canonical_error: 'canon refuses' }
	]);
	ok(err.includes('  t.css: RuntimeError: unreachable'));
	ok(!err.some((l) => l.includes('u.css')));
});

test('engines that fail to load fail the run before any triage', async () => {
	const { deps, state } = fake_deps([file('a.ts', 'typescript', 'fine')], { loads: false });
	strictEqual(await run_skip_triage(deps, []), 1);
	strictEqual(state.out, '');
});
