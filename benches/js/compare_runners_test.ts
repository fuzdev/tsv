/**
 * Tests that every command the task files, the CI workflows, and the comparison tools print or
 * document names a task that runs: an `npm run <x>` needs `<x>` in `package.json`'s scripts,
 * and a `deno task <x>` needs it in `deno.json`'s tasks or in `package.json`'s, which
 * `deno task` falls through to. And the split between the two files: `deno.json` holds the
 * tasks whose body runs Deno itself, `package.json` every other, chained with `npm run`, its
 * one way to a Deno task a `deno task <x>` leg naming a `deno.json` task (docs/tasks.md). And
 * that no CI workflow installs or runs Deno.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { readdirSync, readFileSync } from 'node:fs';
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

test("the parse tool's usage names runners that run its tasks", async () => {
	const lines = capture_console();
	strictEqual(await run_corpus_compare_parse(HELP_DEPS, ['--help']), 0);
	const usage = lines.join('\n');
	ok(usage.includes('Usage: npm run corpus:compare:parse -- <path> [options]'), usage);
	deepStrictEqual(unrunnable(usage), []);
});

test("the format tool's usage names runners that run its tasks", async () => {
	const lines = capture_console();
	strictEqual(await run_corpus_compare_format(HELP_DEPS, ['--help']), 0);
	const usage = lines.join('\n');
	ok(usage.includes('Usage: npm run corpus:compare:format -- <path> [options]'), usage);
	deepStrictEqual(unrunnable(usage), []);
});

/** Each task body of a task file, by name. */
const BODIES = {
	'package.json': read_json('package.json').scripts as Record<string, string>,
	'deno.json': Object.fromEntries(
		Object.entries(read_json('deno.json').tasks as Record<string, string>).filter(
			([name]) => !name.startsWith('//')
		)
	)
};

/** The commands a body runs: split at `&&`, `||`, `;`, and parentheses, each with any leading
 * `NAME=value` assignments dropped. */
const commands_of = (body: string): string[] =>
	body
		.split(/&&|\|\||;|\(|\)/)
		.map((command) => command.trim().replace(/^(?:[A-Z_][A-Z0-9_]*=\S*\s+)*/, ''))
		.filter((command) => command !== '');

/** Whether `command` runs Deno itself, rather than as the runner of a `deno.json` task. */
const runs_deno = (command: string): boolean => /^deno (?!task )/.test(command);

test('each `npm run` in a task body names a package.json script, and each `deno task` a deno.json task', () => {
	const unresolved: string[] = [];
	for (const [file, bodies] of Object.entries(BODIES)) {
		for (const [name, body] of Object.entries(bodies)) {
			for (const [command, runner, target] of body.matchAll(/\b(npm run|deno task) ([\w:.-]+)/g)) {
				if (!(runner === 'npm run' ? NPM_SCRIPTS : DENO_TASKS).has(target!)) {
					unresolved.push(`${file} ${name}: ${command}`);
				}
			}
		}
	}
	deepStrictEqual(unresolved, []);
});

test('deno.json holds exactly the tasks whose body runs Deno itself', () => {
	deepStrictEqual(
		Object.entries(BODIES['deno.json'])
			.filter(([, body]) => !commands_of(body).some(runs_deno))
			.map(([name]) => name),
		[]
	);
	deepStrictEqual(
		Object.entries(BODIES['package.json'])
			.filter(([, body]) => commands_of(body).some(runs_deno))
			.map(([name]) => name),
		[]
	);
	deepStrictEqual(
		[...DENO_TASKS].filter((name) => !name.startsWith('//') && NPM_SCRIPTS.has(name)),
		[]
	);
});

/** The CI workflow files, either extension. */
const WORKFLOW_DIR = new URL('.github/workflows/', CHECKOUT);
const WORKFLOWS = readdirSync(WORKFLOW_DIR).filter((name) => /\.ya?ml$/i.test(name));

/** `text`'s lines with YAML comments dropped: a whole-line `#`, or one after whitespace to the
 * line's end (a shell comment, inside a `run: |` block). */
const uncommented = (text: string): string[] =>
	text.split(/\r?\n/).map((line) => line.replace(/(?:^|\s)#.*$/, ''));

/** The package.json scripts `text` runs: `npm run <x>` and `npm run-script <x>`, with npm's
 * flags before or after the command, npm's shorthands for `test` and `start`, and
 * `node --run <x>`. */
const scripts_run_by = (text: string): string[] => {
	const flags = String.raw`(?:[ \t]+--?[\w-]+(?:(?:=|[ \t]+)(?!-)\S+)?)*`;
	const run = new RegExp(
		[
			String.raw`\bnpm${flags}[ \t]+(?:run|run-script|rum|urn)${flags}[ \t]+([\w:.-]+)`,
			String.raw`\bnpm${flags}[ \t]+(test|t|tst|start|stop|restart)\b`,
			String.raw`\bnode${flags}[ \t]+--run(?:=|[ \t]+)([\w:.-]+)`
		].join('|'),
		'g'
	);
	return [...text.matchAll(run)].map(([, script, shorthand, node_run]) => {
		const name = script ?? node_run ?? shorthand!;
		return name === 't' || name === 'tst' ? 'test' : name;
	});
};

test('each runner a CI workflow names runs the task it names', () => {
	for (const file of WORKFLOWS) {
		deepStrictEqual(unrunnable(readFileSync(new URL(file, WORKFLOW_DIR), 'utf8')), [], file);
	}
});

test("no CI workflow installs or runs Deno, a prerequisite of the bench's Deno row alone", () => {
	// any mention outside a comment: an action, an installer's `tool: deno`, an install
	// script's URL, a path to a `deno` binary, a quoted or bare `deno` command
	for (const file of WORKFLOWS) {
		deepStrictEqual(
			uncommented(readFileSync(new URL(file, WORKFLOW_DIR), 'utf8')).filter((line) =>
				/deno/i.test(line)
			),
			[],
			file
		);
	}
});

test('no package.json script a CI workflow runs, or any script it runs in turn, names Deno', () => {
	const scripts = BODIES['package.json'];
	const naming: string[] = [];
	for (const file of WORKFLOWS) {
		const text = uncommented(readFileSync(new URL(file, WORKFLOW_DIR), 'utf8')).join('\n');
		const pending = scripts_run_by(text).map((name) => [name]);
		const seen = new Set<string>();
		for (let chain = pending.pop(); chain !== undefined; chain = pending.pop()) {
			const name = chain.at(-1)!;
			if (seen.has(name)) continue;
			seen.add(name);
			const body = scripts[name];
			if (body === undefined) {
				naming.push(`${file}: ${chain.join(' > ')}: no such package.json script`);
				continue;
			}
			// a `deno task`, `npx deno`, `deno` by path or quoted, or a script named for Deno
			if (/deno/i.test(body)) naming.push(`${file}: ${chain.join(' > ')}: ${body}`);
			for (const next of scripts_run_by(body)) pending.push([...chain, next]);
		}
	}
	deepStrictEqual(naming, []);
});

test('each `npm run` in a package.json body that passes arguments passes them after a `--`', () => {
	// without the `--`, npm takes a flag as its own config and the script never sees it
	const swallowing: string[] = [];
	for (const [name, body] of Object.entries(BODIES['package.json'])) {
		for (const [command, args] of body.matchAll(
			/\bnpm run (?:-s )?[\w:.-]+((?: +[^\s&|;()]+)*)/g
		)) {
			const first = args!.trim().split(/\s+/)[0];
			if (first !== '' && first !== '--') swallowing.push(`${name}: ${command}`);
		}
	}
	deepStrictEqual(swallowing, []);
});
