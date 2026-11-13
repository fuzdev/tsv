/**
 * Tests for `corpus_compare_parse.ts` over fake engines, a fake `tsv_debug`, and a scratch
 * directory as the corpus: the panic gate fails a run whose only finding is a native panic,
 * which the parse buckets alone would read as one more tsv-side parse failure.
 */

import { ok, strictEqual } from 'node:assert';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll, afterEach, test, vi } from 'vitest';

import { type CorpusCompareParseDeps, run_corpus_compare_parse } from './corpus_compare_parse.ts';
import type { CompareImplementations } from './lib/compare_cli.ts';
import { TsvRejection } from './lib/divergence/panic_errors.ts';
import type { TsvDebugChild, TsvDebugProgram } from './lib/tsv_debug_server.ts';
import { raise_wasm_trap } from './lib/wasm_trap_fakes.ts';
import { reconstruct_locations } from '../../crates/tsv_wasm/npm/locations.js';

// the binary is faked below, so its freshness is not this suite's business
vi.mock('./lib/check_artifact_freshness.ts', async (original) => ({
	...(await original<typeof import('./lib/check_artifact_freshness.ts')>()),
	check_artifact_freshness: async () => {}
}));

const corpus = mkdtempSync(join(tmpdir(), 'corpus_compare_parse_test_'));
writeFileSync(join(corpus, 'a.ts'), 'let crash;\n');
// a file both sides parse alike, so a run is never one where nothing was compared
writeFileSync(join(corpus, 'b.ts'), 'let x;\n');

afterAll(() => {
	rmSync(corpus, { recursive: true, force: true });
});

afterEach(() => {
	vi.restoreAllMocks();
});

const capture_console = (): string[] => {
	const lines: string[] = [];
	vi.spyOn(console, 'log').mockImplementation((...a: unknown[]) => void lines.push(a.join(' ')));
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void lines.push(a.join(' ')));
	return lines;
};

const SPAN_AST = { type: 'Program', start: 0, end: 7, body: [] };
/** The loc wire is by definition the span-only wire with its locations reconstructed. */
const loc_ast = (source: string): unknown =>
	reconstruct_locations(structuredClone(SPAN_AST), source, { language: 'typescript' });

/**
 * How the fake tsv treats a source holding `crash`: parses it, panics (both wires), traps (the
 * span wire, a real WASM trap), or returns a rejection whose text reads `panic: boom` (both
 * wires); or it rejects every source.
 */
type Mode = 'parse' | 'panic' | 'trap' | 'panic_text' | 'reject';

/** A `tsv_debug loc_wires --stdin` answering as `mode` says. */
const fake_tsv_debug = (mode: Mode): TsvDebugProgram => ({
	start: async () => {
		const replies: string[] = [];
		let wake: (() => void) | null = null;
		let closed = false;
		const encoder = new TextEncoder();
		const child: TsvDebugChild = {
			stdin: {
				write: async (data) => {
					const { source } = JSON.parse(data) as { source: string };
					replies.push(
						JSON.stringify(
							mode === 'reject'
								? { error: 'Unexpected token' }
								: mode === 'panic' && source.includes('crash')
									? { panic: 'boom' }
									: mode === 'panic_text' && source.includes('crash')
										? { error: 'panic: boom' }
										: { loc: loc_ast(source) }
						) + '\n'
					);
					wake?.();
					return true;
				},
				close: async () => {
					closed = true;
					wake?.();
				}
			},
			stdout: (async function* () {
				for (;;) {
					if (replies.length > 0) yield encoder.encode(replies.shift()!);
					else if (closed) return;
					else await new Promise<void>((r) => (wake = r));
				}
			})(),
			closed: Promise.resolve({ kind: 'exited', code: 0 }),
			kill: () => false
		};
		return child;
	}
});

const fake_impls = (mode: Mode) => {
	const state = { disposed: 0 };
	const impls: CompareImplementations = {
		canonical: {
			format_async: async (source) => source,
			parse: (source) => loc_ast(source),
			enable_format_cache: () => null
		},
		native: {
			format: (source) => source,
			parse: (source) => {
				if (mode === 'reject') throw new Error('Unexpected token');
				if (source.includes('crash')) {
					if (mode === 'panic') throw new Error('panic: boom');
					if (mode === 'trap') raise_wasm_trap();
					if (mode === 'panic_text') throw new TsvRejection('panic: boom');
				}
				return SPAN_AST;
			},
			parse_internal: () => {}
		},
		dispose: () => void state.disposed++
	};
	return { state, impls };
};

const deps = (mode: Mode): { deps: CorpusCompareParseDeps; state: { disposed: number } } => {
	const { state, impls } = fake_impls(mode);
	return {
		state,
		deps: {
			init_implementations: async () => impls,
			open_tsv_debug: () => fake_tsv_debug(mode),
			home: '/home/nobody',
			stdout: { write: async () => true },
			pins: {
				read: () => Promise.reject(new Error('the pins are graded by --all only'))
			}
		}
	};
};

test('a native panic on both wires (one per wire) fails the run, where the buckets alone read a parse failure', async () => {
	const lines = capture_console();
	const { deps: d, state } = deps('panic');
	strictEqual(await run_corpus_compare_parse(d, [corpus]), 1);
	strictEqual(state.disposed, 1);
	ok(
		lines.some((l) => l.includes('FAIL: 2 panic(s)')),
		lines.join('\n')
	);
	ok(lines.includes('  a.ts: panic: boom'));
});

test('a trap on the span wire is a crash too, named by its type', async () => {
	const lines = capture_console();
	const { deps: d, state } = deps('trap');
	strictEqual(await run_corpus_compare_parse(d, [corpus]), 1);
	strictEqual(state.disposed, 1);
	ok(
		lines.some((l) => l.includes('FAIL: 1 panic(s)')),
		lines.join('\n')
	);
	ok(lines.includes('  a.ts: RuntimeError: unreachable'));
});

test("a rejection whose text reads `panic: ` is the source's verdict, not a crash", async () => {
	const lines = capture_console();
	const { deps: d, state } = deps('panic_text');
	strictEqual(await run_corpus_compare_parse(d, [corpus]), 0, lines.join('\n'));
	strictEqual(state.disposed, 1);
	ok(!lines.some((l) => l.includes('panic(s)')));
});

test('the same run without the panic passes', async () => {
	const lines = capture_console();
	const { deps: d, state } = deps('parse');
	strictEqual(await run_corpus_compare_parse(d, [corpus]), 0, lines.join('\n'));
	strictEqual(state.disposed, 1);
});

test('a run where tsv rejects every file compares nothing and fails', async () => {
	const lines = capture_console();
	const { deps: d, state } = deps('reject');
	strictEqual(await run_corpus_compare_parse(d, [corpus]), 1);
	strictEqual(state.disposed, 1);
	ok(
		lines.some((l) => l.includes('FAIL: 0 of 2 files compared')),
		lines.join('\n')
	);
});
