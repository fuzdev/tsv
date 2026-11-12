/**
 * Tests that every command the comparison tools print or document names a task that runs:
 * an `npm run <x>` needs `<x>` in `package.json`'s scripts, and a `deno task <x>` needs it in
 * `deno.json`'s tasks or in `package.json`'s, which `deno task` falls through to. A task
 * that chains a Deno-only step (`corpus:compare:parse` chains `build:check`) lives in
 * `deno.json` alone, so `npm run` cannot spell it.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { readFileSync } from 'node:fs';
import { afterEach, test, vi } from 'vitest';

import { run_corpus_compare_format } from './corpus_compare_format.ts';
import { run_corpus_compare_parse } from './corpus_compare_parse.ts';

const CHECKOUT = new URL('../../', import.meta.url);

const read_json = (path: string): Record<string, unknown> =>
	JSON.parse(readFileSync(new URL(path, CHECKOUT), 'utf8')) as Record<string, unknown>;

const NPM_SCRIPTS = new Set(Object.keys(read_json('package.json').scripts as object));
const DENO_TASKS = new Set(Object.keys(read_json('deno.json').tasks as object));

/** The modules of the comparison tools and their roots, whose text names their tasks. */
const SOURCES = [
	'benches/js/conformance.ts',
	'benches/js/corpus_compare_format.ts',
	'benches/js/corpus_compare_parse.ts',
	'benches/js/diagnostics/svelte_fixtures_compare.ts',
	'benches/js/diagnostics/ts_fixtures_compare.ts',
	'benches/js/diagnostics/ts_repo_compare.ts',
	'benches/js/lib/compare_cli.ts',
	'benches/js/lib/compare_engines.ts',
	'benches/js/lib/corpus_pins.ts',
	'benches/js/lib/fixtures_gate.ts',
	'benches/js/roots/compare_deps.ts',
	'benches/js/roots/conformance.ts',
	'benches/js/roots/corpus_compare_format.ts',
	'benches/js/roots/corpus_compare_parse.ts',
	'benches/js/roots/svelte_fixtures_compare.ts',
	'benches/js/roots/ts_fixtures_compare.ts',
	'benches/js/roots/ts_repo_compare.ts'
];

/** Each `npm run <x>` or `deno task <x>` in `text` whose `<x>` that runner cannot run. */
const unrunnable = (text: string): string[] =>
	[...text.matchAll(/\b(npm run(?: -s)?|deno task(?: -q)?) ([\w:.-]+)/g)]
		.filter(([, runner, name]) =>
			runner!.startsWith('npm')
				? !NPM_SCRIPTS.has(name!)
				: !DENO_TASKS.has(name!) && !NPM_SCRIPTS.has(name!)
		)
		.map(([command]) => command);

afterEach(() => {
	vi.restoreAllMocks();
});

const capture_console = (): string[] => {
	const lines: string[] = [];
	vi.spyOn(console, 'log').mockImplementation((...a: unknown[]) => void lines.push(a.join(' ')));
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void lines.push(a.join(' ')));
	return lines;
};

/** Deps a `--help` run never reaches. */
const unreached = () => Promise.reject(new Error('a --help run loads nothing'));
const HELP_DEPS = {
	init_implementations: unreached,
	open_tsv_debug: () => {
		throw new Error('a --help run starts nothing');
	},
	home: '/home/nobody',
	stdout: { write: async () => true },
	pins: { read: unreached, write: unreached }
};

test("each runner the tools' modules and roots name runs the task it names", () => {
	for (const path of SOURCES) {
		deepStrictEqual(unrunnable(readFileSync(new URL(path, CHECKOUT), 'utf8')), [], path);
	}
});

test("the parse tool's usage names `deno task`, where its build-first task lives", async () => {
	const lines = capture_console();
	strictEqual(await run_corpus_compare_parse(HELP_DEPS, ['--help']), 0);
	const usage = lines.join('\n');
	ok(usage.includes('Usage: deno task corpus:compare:parse <path> [options]'), usage);
	deepStrictEqual(unrunnable(usage), []);
});

test("the format tool's usage names runners that run its tasks", async () => {
	const lines = capture_console();
	strictEqual(await run_corpus_compare_format(HELP_DEPS, ['--help']), 0);
	const usage = lines.join('\n');
	ok(usage.includes('Usage: npm run corpus:compare:format -- <path> [options]'), usage);
	deepStrictEqual(unrunnable(usage), []);
});
