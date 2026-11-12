/**
 * Tests for the comparison tools' shared scaffolding (`compare_cli.ts`): the base path, the
 * language filter's refusal, the panic gate's verdict, the failure contract's release of the
 * engines, and `run_compare_main`'s error report under `--json`. Over fakes; package-free, so it
 * gates in `npm run test:node`.
 */

import { deepStrictEqual, strictEqual } from 'node:assert';
import { afterEach, test, vi } from 'vitest';
import * as s from '@fuzdev/fuz_schema/schema.ts';

import {
	type CompareImplementations,
	fail_compare,
	gate_on_panics,
	parse_language_filter,
	resolve_compare_base_path,
	run_compare_main
} from './compare_cli.ts';

/** Collects what `console.log` and `console.error` print, for the test's life. */
const capture_console = (): string[] => {
	const lines: string[] = [];
	vi.spyOn(console, 'log').mockImplementation((...a: unknown[]) => void lines.push(a.join(' ')));
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void lines.push(a.join(' ')));
	return lines;
};

afterEach(() => {
	vi.restoreAllMocks();
});

/** A stdout that keeps what it's given. */
const memory_stdout = () => {
	const written: string[] = [];
	return { written, stdout: { write: async (data: string) => written.push(data) } };
};

test('the base path expands `~` from the given HOME, and `--all` is `HOME/dev`', () => {
	strictEqual(resolve_compare_base_path('~/x', false, '/h'), '/h/x');
	strictEqual(resolve_compare_base_path('../x', false, '/h'), '../x');
	strictEqual(resolve_compare_base_path(undefined, true, '/h'), '/h/dev');
	strictEqual(resolve_compare_base_path(undefined, true, undefined), '/dev');
});

test('an unknown language filter is refused with the languages listed', () => {
	const lines = capture_console();
	deepStrictEqual(parse_language_filter(undefined), { ok: true, language: undefined });
	deepStrictEqual(parse_language_filter('css'), { ok: true, language: 'css' });
	deepStrictEqual(parse_language_filter('cobol'), { ok: false });
	deepStrictEqual(lines, [
		'Error: Invalid filter "cobol". Must be one of: svelte, typescript, css'
	]);
});

test('the panic gate trips on a `panic: ` failure only, and lists it relative to the base', () => {
	const lines = capture_console();
	strictEqual(
		gate_on_panics([{ path: '/b/a.ts', error: 'Unexpected token' }, { path: '/b/c.ts' }], '/b'),
		false
	);
	strictEqual(lines.length, 0);
	strictEqual(
		gate_on_panics([{ path: '/b/a.ts', error: 'panic: index out of bounds' }], '/b'),
		true
	);
	strictEqual(lines.length, 2);
	strictEqual(lines[1], '  a.ts: panic: index out of bounds');
});

test('a failed run releases the engines and exits 1', () => {
	let disposed = 0;
	const impls: CompareImplementations = {
		canonical: {} as CompareImplementations['canonical'],
		native: {} as CompareImplementations['native'],
		dispose: () => void disposed++
	};
	strictEqual(fail_compare(impls), 1);
	strictEqual(disposed, 1);
});

const Args = s.strip_object({ json: s.with_default(s.boolean, false) });

test('a tool that throws exits 1, with an error report on stdout only under --json', async () => {
	const lines = capture_console();
	const fail = () => Promise.reject(new Error('sidecar down'));
	const plain = memory_stdout();
	const build_error_report = (message: string) => ({ error: message });
	strictEqual(
		await run_compare_main(fail, {
			argv: [],
			schema: Args,
			build_error_report,
			stdout: plain.stdout
		}),
		1
	);
	deepStrictEqual(plain.written, []);
	const json = memory_stdout();
	strictEqual(
		await run_compare_main(fail, {
			argv: ['--json'],
			schema: Args,
			build_error_report,
			stdout: json.stdout
		}),
		1
	);
	deepStrictEqual(json.written, [JSON.stringify({ error: 'sidecar down' }, null, '\t') + '\n']);
	deepStrictEqual(lines, ['sidecar down', 'sidecar down']);
	strictEqual(
		await run_compare_main(() => Promise.resolve(3), {
			argv: [],
			schema: Args,
			build_error_report,
			stdout: plain.stdout
		}),
		3
	);
});
