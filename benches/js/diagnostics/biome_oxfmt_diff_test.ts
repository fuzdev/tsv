/**
 * `run_biome_oxfmt_diff` over fake engines and formatters: `--files` batches every path after it
 * (one verdict line each, no bodies), however the argument parser split them, a single source
 * prints each formatter's output and the verdict, and a formatter that can't load is reported
 * and left out. Package-free, so it gates in `npm run test:node`.
 */

import { deepStrictEqual, strictEqual } from 'node:assert';
import { afterEach, beforeEach, test, vi } from 'vitest';

import type { CompareImplementations } from '../lib/compare_cli.ts';
import {
	type BiomeOxfmtDiffDeps,
	type DiffFormatter,
	type DiffFormatterName,
	run_biome_oxfmt_diff
} from './biome_oxfmt_diff.ts';

let err: string[];
beforeEach(() => {
	err = [];
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void err.push(a.join(' ')));
});
afterEach(() => {
	vi.restoreAllMocks();
});

/** A formatter that formats every source as `format` does; `async` answers through a promise. */
const formatter = (format: (source: string) => string, async = false): DiffFormatter => ({
	supports_format_language: (language) => language === 'typescript',
	format: (source) => (async ? Promise.resolve(format(source)) : format(source)),
	dispose: () => {}
});

const fake_deps = (options: { unavailable?: DiffFormatterName } = {}) => {
	const state = { out: '', read: [] as string[], disposed: [] as string[] };
	const engines: CompareImplementations = {
		canonical: {
			format_async: async (source) => source.trim(),
			parse: () => ({}),
			enable_format_cache: () => null
		},
		native: { format: (source) => source.trim(), parse: () => ({}), parse_internal: () => {} },
		dispose: () => void state.disposed.push('engines')
	};
	const deps: BiomeOxfmtDiffDeps = {
		init_implementations: async () => engines,
		load_formatter: async (name) => {
			if (name === options.unavailable) throw new Error(`no ${name}`);
			const f =
				name === 'biome' ? formatter((s) => s.trim()) : formatter((s) => `${s.trim()};`, true);
			return { ...f, dispose: () => void state.disposed.push(name) };
		},
		read_file: async (path) => {
			state.read.push(path);
			return ` ${path} `;
		},
		read_stdin: async () => '',
		stdout: { write: async (data) => void (state.out += data) }
	};
	return { deps, state };
};

const VERDICT = 'tsv==prettier biome[=tsv,=prettier] oxfmt[≠both]';

test('`--files a b c` batches every path after the flag, one verdict line each', async () => {
	const { deps, state } = fake_deps();
	const code = await run_biome_oxfmt_diff(deps, [
		'--parser',
		'typescript',
		'--files',
		'a.ts',
		'b.ts',
		'c.ts'
	]);
	strictEqual(code, 0, err.join('\n'));
	deepStrictEqual(state.read, ['a.ts', 'b.ts', 'c.ts']);
	strictEqual(state.out, `${VERDICT}  a.ts\n${VERDICT}  b.ts\n${VERDICT}  c.ts\n`);
	deepStrictEqual(state.disposed, ['engines', 'biome', 'oxfmt']);
});

test('a repeated `--files`, and one path alone, batch too', async () => {
	const repeated = fake_deps();
	strictEqual(await run_biome_oxfmt_diff(repeated.deps, ['--files', 'a.ts', '--files', 'b.ts']), 0);
	deepStrictEqual(repeated.state.read, ['a.ts', 'b.ts']);
	const one = fake_deps();
	strictEqual(await run_biome_oxfmt_diff(one.deps, ['--files', 'a.ts']), 0);
	strictEqual(one.state.out, `${VERDICT}  a.ts\n`);
});

test('a single source prints each output, then the verdict', async () => {
	const { deps, state } = fake_deps();
	strictEqual(await run_biome_oxfmt_diff(deps, ['--content', ' x ']), 0);
	strictEqual(
		state.out,
		[
			'\n===== tsv =====',
			'x',
			'\n===== prettier =====',
			'x',
			'\n===== biome =====',
			'x',
			'\n===== oxfmt =====',
			'x;',
			`\n===== verdict =====\n${VERDICT}`,
			''
		].join('\n')
	);
});

test('a formatter that cannot load is reported and left out of the verdict', async () => {
	const { deps, state } = fake_deps({ unavailable: 'biome' });
	strictEqual(await run_biome_oxfmt_diff(deps, ['--files', 'a.ts']), 0);
	deepStrictEqual(err, ['biome unavailable: Error: no biome']);
	strictEqual(state.out, 'tsv==prettier biome:- oxfmt[≠both]  a.ts\n');
	deepStrictEqual(state.disposed, ['engines', 'oxfmt']);
});

test('no source fails, releasing what loaded', async () => {
	const { deps, state } = fake_deps();
	strictEqual(await run_biome_oxfmt_diff(deps, ['--parser', 'css']), 1);
	deepStrictEqual(err, ['provide --content, --stdin, --file, or --files']);
	deepStrictEqual(state.disposed, ['engines', 'biome', 'oxfmt']);
});
