/**
 * Tests for `corpus_compare_format.ts` over fake engines, a scratch directory as the corpus,
 * and an in-memory pins file: the usage errors, an engine that failed to load, and the panic
 * gate — a native `panic: …` fails the run loudly, even on a file an expected-error pattern
 * would otherwise absorb. Package-free, so it gates in `npm run test:node`.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll, afterEach, test, vi } from 'vitest';

import {
	type CorpusCompareFormatDeps,
	corpus_compare_format_main,
	run_corpus_compare_format
} from './corpus_compare_format.ts';
import type { CompareImplementations } from './lib/compare_cli.ts';
import { TsvRejection } from './lib/divergence/panic_errors.ts';
import { raise_wasm_trap } from './lib/wasm_trap_fakes.ts';

const corpus = mkdtempSync(join(tmpdir(), 'corpus_compare_format_test_'));
// SCSS `$variable`: an expected-error pattern's content (`lib/divergence/expected_errors.ts`)
writeFileSync(join(corpus, 'a.css'), '$x: 1;\na {\n\tcolor: $x;\n}\n');
// a file the fakes format alike, so a run is never one where every file errored
writeFileSync(join(corpus, 'b.css'), 'b {\n}\n');

afterAll(() => {
	rmSync(corpus, { recursive: true, force: true });
});

// `--json` reroutes `console.log` to stderr for the rest of the process (`redirect_logs_to_stderr`)
const console_log = console.log;
afterEach(() => {
	vi.restoreAllMocks();
	console.log = console_log;
});

/** Collects what `console.log` and `console.error` print. */
const capture_console = (): string[] => {
	const lines: string[] = [];
	vi.spyOn(console, 'log').mockImplementation((...a: unknown[]) => void lines.push(a.join(' ')));
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void lines.push(a.join(' ')));
	return lines;
};

/**
 * Engines whose tsv side formats with `native_format`; prettier echoes its input, or formats
 * with `prettier_format`.
 */
const fake_impls = (
	native_format: (source: string) => string,
	prettier_format: (source: string) => string = (source) => source
) => {
	const state = { disposed: 0 };
	const impls: CompareImplementations = {
		canonical: {
			format_async: async (source) => prettier_format(source),
			parse: () => ({}),
			enable_format_cache: () => null
		},
		native: {
			format: native_format,
			parse: () => ({}),
			parse_internal: () => {}
		},
		dispose: () => void state.disposed++
	};
	return { state, impls };
};

const deps = (impls: CompareImplementations | null): CorpusCompareFormatDeps => ({
	init_implementations: async () => impls,
	home: '/home/nobody',
	stdout: { write: async () => true },
	pins: {
		read: () => Promise.reject(new Error('the pins are graded by --all only')),
		write: () => Promise.reject(new Error('the pins are rewritten by --all --update only'))
	}
});

test('a native panic fails the run, and the expected-error pattern its file matches cannot absorb it', async () => {
	const lines = capture_console();
	const { state, impls } = fake_impls((source) => {
		if (!source.includes('$x')) return source;
		throw new Error('panic: index out of bounds: the len is 3 but the index is 5');
	});
	strictEqual(await run_corpus_compare_format(deps(impls), [corpus]), 1);
	strictEqual(state.disposed, 1);
	ok(lines.some((l) => l.includes('FAIL: 1 panic(s)')));
	ok(lines.includes('  a.css: panic: index out of bounds: the len is 3 but the index is 5'));
	ok(!lines.some((l) => l.includes('expected errors')));
});

test('an ordinary rejection on that file is absorbed as expected, and the run passes', async () => {
	const lines = capture_console();
	const { state, impls } = fake_impls((source) => {
		if (!source.includes('$x')) return source;
		throw new Error('Unexpected character `$`');
	});
	strictEqual(await run_corpus_compare_format(deps(impls), [corpus]), 0);
	strictEqual(state.disposed, 1);
	ok(lines.some((l) => l.includes('expected errors')));
	ok(!lines.some((l) => l.includes('panic')));
});

test('a WASM trap out of the tsv call is a crash too, named by its type', async () => {
	const lines = capture_console();
	const { state, impls } = fake_impls((source) =>
		source.includes('$x') ? raise_wasm_trap() : source
	);
	strictEqual(await run_corpus_compare_format(deps(impls), [corpus]), 1);
	strictEqual(state.disposed, 1);
	ok(lines.some((l) => l.includes('FAIL: 1 panic(s)')));
	ok(lines.includes('  a.css: RuntimeError: unreachable'));
});

test("a returned rejection whose text reads `panic: ` is the source's verdict: absorbed as expected", async () => {
	const lines = capture_console();
	const { state, impls } = fake_impls((source) => {
		if (!source.includes('$x')) return source;
		throw new TsvRejection('panic: index out of bounds');
	});
	strictEqual(await run_corpus_compare_format(deps(impls), [corpus]), 0);
	strictEqual(state.disposed, 1);
	ok(lines.some((l) => l.includes('expected errors')));
	ok(!lines.some((l) => l.includes('panic(s)')));
});

test("a throw out of prettier is no crash of tsv's, whatever it reads", async () => {
	const lines = capture_console();
	const { state, impls } = fake_impls(
		(source) => source,
		(source) => {
			if (source.includes('$x')) throw new Error('panic: prettier wrote this');
			return source;
		}
	);
	strictEqual(await run_corpus_compare_format(deps(impls), [corpus]), 0);
	strictEqual(state.disposed, 1);
	ok(!lines.some((l) => l.includes('panic(s)')));
});

test('a matching run passes and releases the engines', async () => {
	capture_console();
	const { state, impls } = fake_impls((source) => source);
	strictEqual(await run_corpus_compare_format(deps(impls), [corpus]), 0);
	strictEqual(state.disposed, 1);
});

test('usage errors exit 1 before any engine loads', async () => {
	const lines = capture_console();
	let loads = 0;
	const counting: CorpusCompareFormatDeps = {
		...deps(null),
		init_implementations: async () => {
			loads++;
			return null;
		}
	};
	strictEqual(await run_corpus_compare_format(counting, []), 1);
	strictEqual(await run_corpus_compare_format(counting, [corpus, '--filter', 'cobol']), 1);
	strictEqual(await run_corpus_compare_format(counting, [corpus, '--update']), 1);
	strictEqual(await run_corpus_compare_format(counting, ['--help']), 0);
	strictEqual(loads, 0);
	ok(lines.some((l) => l.startsWith('Error: --update rewrites the pins from a full run')));
	strictEqual(await run_corpus_compare_format(counting, [corpus]), 1);
	strictEqual(loads, 1);
});

test('a throw under --json still writes an error-shaped report', async () => {
	capture_console();
	const written: string[] = [];
	const throwing: CorpusCompareFormatDeps = {
		...deps(null),
		init_implementations: () => Promise.reject(new Error('addon refused')),
		stdout: { write: async (data: string) => written.push(data) }
	};
	strictEqual(await corpus_compare_format_main(throwing, [corpus, '--json']), 1);
	strictEqual(written.length, 1);
	const report = JSON.parse(written[0]!) as { error: string; safety: unknown[] };
	strictEqual(report.error, 'addon refused');
	deepStrictEqual(report.safety, []);
});

test('a file whose formatting loses content prettier keeps fails the run as a SAFETY violation', async () => {
	const lines = capture_console();
	const { state, impls } = fake_impls((source) => source.replace('b {', 'b{').replace('b', ''));
	strictEqual(await run_corpus_compare_format(deps(impls), [corpus]), 1);
	strictEqual(state.disposed, 1);
	ok(
		lines.some((l) => l.includes('FAIL: 1 safety violations')),
		lines.join('\n')
	);
});
