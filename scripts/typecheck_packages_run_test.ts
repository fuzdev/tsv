/**
 * `run_typecheck_packages` over a temp checkout holding a fixture staging of the format
 * package, with a fake TypeScript that records what each program was asked to compile: the
 * install check, the package selection, the staleness guard, the consumer project as a
 * package install lays it out (the declared `files`, a directory recursively, and nothing
 * else), the programs and their options, and how a diagnostic, an entry point no program
 * loaded, and a declared file the staging lacks fail the run.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';
// test plumbing: the fake compiler reads the consumer project as TypeScript does, by path
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

import { fs, type FsDir } from '@fuzdev/gro/fs.ts';
import { test } from 'vitest';

import { CORE_CRATES, WASM_CRATES } from '../benches/js/lib/crate_lists.ts';
import { napi_loader_checks, wasm_package_checks } from './check_staged_freshness.ts';
import { TYPECHECK_INSTALL, TYPESCRIPT_MODULE, TYPES_ROOT } from './typecheck_install.ts';
import { run_typecheck_packages, type TypeScriptApi } from './typecheck_packages.ts';

/** Minutes apart, so no comparison ties. */
const T_SOURCE = Date.UTC(2026, 0, 1);
const T_STAGED = T_SOURCE + 60_000;
const T_LATER = T_STAGED + 60_000;

const PKG = 'crates/tsv_wasm/pkg/format/npm';
const NAME = '@fuzdev/tsv-format-wasm';

const README = [
	'# tsv-format-wasm',
	'',
	'```ts',
	"import {format_css} from '@fuzdev/tsv-format-wasm';",
	"format_css('a {}');",
	'```',
	'',
	'<!-- typecheck: node -->',
	'```js',
	"import {readFileSync} from 'node:fs';",
	'```',
	''
].join('\n');

/** The fixture package's manifest: two entries, a `files` list naming a directory, and a file
 * on disk the list leaves out. */
const MANIFEST = {
	name: NAME,
	files: ['index.js', 'index.d.ts', 'types', 'README.md'],
	exports: {
		'.': { types: './index.d.ts', default: './index.js' },
		'./worker': { types: './types/worker.d.ts', default: './index.js' }
	}
};

/**
 * A checkout with every source the format package's checks name dated `T_SOURCE`, the
 * `benches/js` install, and the format package staged, its files dated `T_STAGED`.
 */
const make_checkout = async (): Promise<FsDir & AsyncDisposable> => {
	const root = await fs.temp_dir('tsv_typecheck_packages_test_');
	const write = async (path: string, content: string, ms: number): Promise<void> => {
		const dir = path.split('/').slice(0, -1).join('/');
		if (dir !== '') await root.mkdir(dir, { recursive: true });
		await root.write(path, content);
		// the umask's bits stay out of what the suite compares
		await root.set_mode(path, 0o644);
		await root.set_times(path, { mtime_ms: ms });
	};
	const sources = new Set<string>([
		'Cargo.toml',
		'Cargo.lock',
		'package.json',
		'crates/tsv_wasm/README_format.md'
	]);
	for (const crate of [...CORE_CRATES, ...WASM_CRATES]) {
		sources.add(`crates/${crate}/Cargo.toml`);
		sources.add(`crates/${crate}/src/lib.rs`);
	}
	for (const check of wasm_package_checks('format'))
		for (const file of check.files) sources.add(file);
	for (const path of sources) await write(path, path, T_SOURCE);
	for (const { path } of TYPECHECK_INSTALL) await write(path, path, T_SOURCE);
	for (const check of wasm_package_checks('format'))
		await write(check.staged, check.staged, T_STAGED);
	await write(`${PKG}/package.json`, JSON.stringify(MANIFEST), T_STAGED);
	await write(`${PKG}/README.md`, README, T_STAGED);
	await write(`${PKG}/index.d.ts`, 'export declare const a: 1;\n', T_STAGED);
	await write(`${PKG}/types/worker.d.ts`, 'export {};\n', T_STAGED);
	await write(`${PKG}/types/deep/more.d.ts`, 'export {};\n', T_STAGED);
	await root.set_mode(`${PKG}/types/deep/more.d.ts`, 0o600);
	await write(`${PKG}/unlisted.js`, 'unlisted', T_STAGED);
	return root;
};

/** One `createProgram` call: its roots and options, consumer-relative where they're paths. */
interface ProgramCall {
	roots: Array<string>;
	options: Record<string, unknown>;
}

/**
 * A fake compiler. Each program "loads" its roots and, unless `unloaded` names it, every
 * declaration the fixture's exports map names; `diagnose` gives a program's diagnostics by
 * its roots. It snapshots the consumer project (every file and its mode) at the first program,
 * while the project exists.
 */
const fake_typescript = (
	options: { diagnose?: (roots: Array<string>) => Array<string>; unloaded?: Array<string> } = {}
) => {
	const calls: Array<ProgramCall> = [];
	let consumer = '';
	const tree: Record<string, string> = {};
	const snapshot = (dir: string): void => {
		for (const entry of readdirSync(dir, { withFileTypes: true })) {
			const path = join(dir, entry.name);
			if (entry.isDirectory()) snapshot(path);
			else tree[relative(consumer, path)] = (statSync(path).mode & 0o777).toString(8);
		}
	};
	const ts: TypeScriptApi = {
		version: '0.0.0-fake',
		convertCompilerOptionsFromJson: (json) => ({ options: json, errors: [] }),
		createProgram: (roots, program_options) => {
			if (consumer === '') {
				consumer = roots[0]!.split('/format/')[0]!;
				ok(existsSync(join(consumer, 'package.json')));
				snapshot(consumer);
			}
			calls.push({
				roots: roots.map((root) => relative(consumer, root)),
				options: program_options as Record<string, unknown>
			});
			const installed = join(consumer, 'node_modules', NAME);
			const declarations = ['index.d.ts', 'types/worker.d.ts']
				.filter((d) => !(options.unloaded ?? []).includes(d))
				.map((d) => join(installed, d));
			const diagnostics = options.diagnose?.(roots.map((root) => relative(consumer, root))) ?? [];
			return {
				getSourceFiles: () => [...roots, ...declarations].map((fileName) => ({ fileName })),
				diagnostics
			} as never;
		},
		getPreEmitDiagnostics: (program) =>
			(program as unknown as { diagnostics: Array<string> }).diagnostics,
		formatDiagnostics: (diagnostics) => diagnostics.map((d) => `${String(d)}\n`).join('')
	};
	return { ts, calls, tree, consumer: () => consumer };
};

/** A run over `checkout` with `ts`, what it printed, and the paths it loaded TypeScript from. */
const run = async (
	checkout: FsDir,
	ts: TypeScriptApi,
	argv: Array<string>,
	stale_ok = false
): Promise<{ code: number; out: Array<string>; err: Array<string>; loaded: Array<string> }> => {
	const out: Array<string> = [];
	const err: Array<string> = [];
	const loaded: Array<string> = [];
	const code = await run_typecheck_packages(
		{
			checkout: checkout.readonly(),
			temp_dir: () => fs.temp_dir('tsv_typecheck_packages_consumer_'),
			load_typescript: (path) => {
				loaded.push(path);
				return ts;
			},
			stale_ok,
			print: (line) => out.push(line),
			print_error: (line) => err.push(line)
		},
		argv
	);
	return { code, out, err, loaded };
};

test('a staged package installs into the consumer as npm lays it out, and grades clean', async () => {
	await using root = await make_checkout();
	const fake = fake_typescript();
	const { code, out, err, loaded } = await run(root, fake.ts, ['format']);
	deepStrictEqual(err, []);
	deepStrictEqual(loaded, [TYPESCRIPT_MODULE]);
	deepStrictEqual(out, [
		"Type-checking the staged packages' declarations with TypeScript 0.0.0-fake",
		`  PASS: ${NAME} · nodenext · es2022+dom (5 modules)`,
		`  PASS: ${NAME} · bundler · es2022+dom (5 modules)`,
		`  PASS: ${NAME} · nodenext · es2022+dom · @types/node (1 modules)`,
		`\n✓ ${NAME}: declarations type-check`
	]);
	strictEqual(code, 0);
	// the manifest, its declared files (the directory whole, modes kept), and the consumer
	// modules; nothing the files list leaves out
	const installed = `node_modules/${NAME}`;
	deepStrictEqual(
		Object.fromEntries(Object.entries(fake.tree).filter(([path]) => path.startsWith(installed))),
		{
			[`${installed}/package.json`]: '644',
			[`${installed}/index.js`]: '644',
			[`${installed}/index.d.ts`]: '644',
			[`${installed}/types/worker.d.ts`]: '644',
			[`${installed}/types/deep/more.d.ts`]: '600',
			[`${installed}/README.md`]: '644'
		}
	);
	deepStrictEqual(
		Object.keys(fake.tree)
			.filter((path) => !path.startsWith(installed))
			.sort(),
		[
			'format/absent.ts',
			'format/entry_bundler.ts',
			'format/entry_node.ts',
			'format/format.ts',
			'format/readme_line_3.ts',
			'format/readme_line_9.js',
			'format/worker.ts',
			'package.json'
		]
	);
	// the consumer project is gone once the run ends
	strictEqual(existsSync(fake.consumer()), false);
	// each program's roots and the options that differ between them
	deepStrictEqual(
		fake.calls.map(({ roots, options }) => ({
			roots: roots.sort(),
			lib: options.lib,
			moduleResolution: options.moduleResolution,
			types: options.types,
			typeRoots: options.typeRoots
		})),
		[
			{
				roots: [
					'format/absent.ts',
					'format/entry_node.ts',
					'format/format.ts',
					'format/readme_line_3.ts',
					'format/worker.ts'
				],
				lib: ['es2022', 'dom'],
				moduleResolution: 'nodenext',
				types: [],
				typeRoots: undefined
			},
			{
				roots: [
					'format/absent.ts',
					'format/entry_bundler.ts',
					'format/format.ts',
					'format/readme_line_3.ts',
					'format/worker.ts'
				],
				lib: ['es2022', 'dom'],
				moduleResolution: 'bundler',
				types: [],
				typeRoots: undefined
			},
			{
				roots: ['format/readme_line_9.js'],
				lib: ['es2022', 'dom'],
				moduleResolution: 'nodenext',
				types: ['node'],
				typeRoots: [join(root.path, TYPES_ROOT)]
			}
		]
	);
	for (const { options } of fake.calls) {
		strictEqual(options.strict, true);
		strictEqual(options.exactOptionalPropertyTypes, true);
		strictEqual(options.noUncheckedIndexedAccess, true);
		strictEqual(options.skipLibCheck, false);
	}
});

test('a program with diagnostics fails the run, each printed under its label', async () => {
	await using root = await make_checkout();
	const fake = fake_typescript({
		diagnose: (roots) => (roots.includes('format/entry_bundler.ts') ? ['TS0000: broken'] : [])
	});
	const { code, out, err } = await run(root, fake.ts, ['format']);
	strictEqual(code, 1);
	deepStrictEqual(err, [
		`  FAIL: ${NAME} · bundler · es2022+dom — 1 diagnostics`,
		'    TS0000: broken\n    ',
		`\n✗ ${NAME}: declarations do not type-check`
	]);
	strictEqual(out.filter((line) => line.startsWith('  PASS')).length, 2);
});

test('an exports entry no program loaded fails the run, naming it', async () => {
	await using root = await make_checkout();
	const { code, err } = await run(root, fake_typescript({ unloaded: ['types/worker.d.ts'] }).ts, [
		'format'
	]);
	strictEqual(code, 1);
	deepStrictEqual(err, [
		`  FAIL: declaration entry points no program loaded:\n    node_modules/${NAME}/types/worker.d.ts`,
		`\n✗ ${NAME}: declarations do not type-check`
	]);
});

test('a file the files list declares that the staging lacks fails the run, naming the restage', async () => {
	await using root = await make_checkout();
	await root.remove(`${PKG}/types`, { recursive: true });
	const { code, err } = await run(root, fake_typescript().ts, ['format']);
	strictEqual(code, 1);
	deepStrictEqual(err, [
		`  FAIL: ${NAME}: the staging lacks ${PKG}/types, which its files list declares — restage: deno task build:npm:format`,
		`\n✗ ${NAME}: declarations do not type-check`
	]);
});

test('a missing install fails before TypeScript loads, naming the install', async () => {
	for (const { what, path } of TYPECHECK_INSTALL) {
		await using root = await make_checkout();
		await root.remove(path);
		const { code, out, err, loaded } = await run(root, fake_typescript().ts, []);
		strictEqual(code, 1);
		deepStrictEqual(out, []);
		deepStrictEqual(loaded, []);
		deepStrictEqual(err, [
			`✗ ${what} is not installed at ${path} — run \`deno task bench:install\``
		]);
	}
});

test('no arguments grade every staged package and skip the rest', async () => {
	await using root = await make_checkout();
	const { code, out } = await run(root, fake_typescript().ts, []);
	strictEqual(code, 0);
	deepStrictEqual(out.slice(0, 4), [
		'  skip: @fuzdev/tsv-parse-wasm is not staged (crates/tsv_wasm/pkg/parse/npm)',
		'  skip: @fuzdev/tsv-wasm is not staged (crates/tsv_wasm/pkg/all/npm)',
		'  skip: @fuzdev/tsv is not staged (crates/tsv_napi/pkg/napi)',
		"Type-checking the staged packages' declarations with TypeScript 0.0.0-fake"
	]);
	await root.remove(`${PKG}/package.json`);
	const none = await run(root, fake_typescript().ts, []);
	strictEqual(none.code, 1);
	deepStrictEqual(none.err, [
		'✗ no package is staged — run `deno task build:packages` and/or `deno task build:napi:packages`'
	]);
});

test('a package named twice is graded once', async () => {
	await using root = await make_checkout();
	const once = await run(root, fake_typescript().ts, ['format']);
	const fake = fake_typescript();
	const { code, out, err } = await run(root, fake.ts, ['format', 'format']);
	strictEqual(code, 0);
	deepStrictEqual(err, []);
	deepStrictEqual(out, once.out);
	strictEqual(fake.calls.length, 3);
});

test('an unknown package name fails the run, naming it', async () => {
	await using root = await make_checkout();
	const { code, out, err } = await run(root, fake_typescript().ts, ['format', 'bogus', 'napo']);
	strictEqual(code, 1);
	deepStrictEqual(out, []);
	deepStrictEqual(err, ['✗ unknown package bogus, napo (expected format, parse, all or napi)']);
});

test('a named package that is not staged is refused as missing, under `stale_ok` too', async () => {
	await using root = await make_checkout();
	const { code, out, err } = await run(root, fake_typescript().ts, ['napi'], true);
	strictEqual(code, 1);
	deepStrictEqual(out, []);
	strictEqual(err.length, 1);
	const lines = err[0]!.split('\n');
	strictEqual(lines[1], '✗ Stale staged package artifacts — refusing to grade outdated code.');
	strictEqual(lines[2], `  • staged loader: not staged — ${napi_loader_checks()[0]!.staged}`);
});

test('a staging older than its README source is refused, and only warns under `stale_ok`', async () => {
	await using root = await make_checkout();
	await root.set_times('crates/tsv_wasm/README_format.md', { mtime_ms: T_LATER });
	const stale = await run(root, fake_typescript().ts, ['format']);
	strictEqual(stale.code, 1);
	deepStrictEqual(stale.out, []);
	ok(stale.err[0]!.includes('  • staged README (format): staged '));
	ok(stale.err[0]!.includes('but crates/tsv_wasm/README_format.md changed'));
	const ok_run = await run(root, fake_typescript().ts, ['format'], true);
	strictEqual(ok_run.code, 0);
	strictEqual(
		ok_run.err[0]!.split('\n')[1],
		'⚠ Stale staged package artifacts (BENCH_STALE_OK=1 — grading anyway).'
	);
	strictEqual(ok_run.out.at(-1), `\n✓ ${NAME}: declarations type-check`);
});
