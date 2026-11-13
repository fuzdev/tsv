/**
 * `run_engine_parity` over a temp checkout and fake programs: which native binary it grades,
 * what it hands each program and where, the in-process half's consumer project, every finding
 * a disagreement prints, and the refusals before anything is graded (no `node`, an engine
 * missing, the staged N-API packages not one, a stale artifact, an extension set it can't
 * read), each with its exit code.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

import { fs, type FsDir, type FsReadDir } from '@fuzdev/gro/fs.ts';
import type { SpawnOutput, SpawnProgram } from '@fuzdev/gro/spawn.ts';
import { test } from 'vitest';

import { CORE_CRATES } from '../benches/js/lib/crate_lists.ts';
import {
	napi_addon_check,
	napi_loader_checks,
	NAPI_STAGING_SOURCES,
	wasm_package_checks
} from './check_staged_freshness.ts';
import { find_native_cli, run_engine_parity, type EngineParityDeps } from './engine_parity.ts';

/** Minutes apart, so no comparison ties. */
const T_SOURCE = Date.UTC(2026, 0, 1);
const T_STAGED = T_SOURCE + 60_000;
const T_LATER = T_STAGED + 60_000;

const TRIPLE = 'linux-x64-gnu';
const NATIVE = `crates/tsv_napi/pkg/${TRIPLE}/tsv`;
const WASM_CLI = 'crates/tsv_wasm/pkg/all/npm/cli.js';
const REFUSAL = 'No files to format — no unignored .ts/.css files in scope';

/** A fixture corpus: two files in the format extensions, one in none, one the walk must skip
 * under `.git`, and one under `node_modules`, which only the strip legs skip. */
const CORPUS: Record<string, string> = {
	'corpus/a.ts': 'let a',
	'corpus/sub/b.css': 'b {}',
	'corpus/sub/c.TS': 'let c',
	'corpus/notes.txt': 'notes',
	'corpus/.git/x.ts': 'skipped',
	'corpus/node_modules/m/y.ts': 'let y'
};

/** A checkout with every source the freshness checks name dated `T_SOURCE`, every staged
 * artifact dated `T_STAGED` (the native binary, the `all` package, the loader and the
 * platform package), and `CORPUS`. */
const make_checkout = async (): Promise<FsDir & AsyncDisposable> => {
	const root = await fs.temp_dir('tsv_engine_parity_test_');
	const checks = [...wasm_package_checks('all'), ...napi_loader_checks(), napi_addon_check(TRIPLE)];
	const sources = new Set<string>(['Cargo.toml', 'Cargo.lock', ...NAPI_STAGING_SOURCES]);
	for (const crate of [...CORE_CRATES, 'tsv_cli', 'tsv_ignore', 'tsv_discover', 'tsv_napi']) {
		sources.add(`crates/${crate}/Cargo.toml`);
		sources.add(`crates/${crate}/src/lib.rs`);
	}
	for (const check of checks) for (const file of check.files) sources.add(file);
	for (const crate of check_crates(checks)) sources.add(`crates/${crate}/src/lib.rs`);
	for (const path of sources) await write(root, path, path, T_SOURCE);
	for (const check of checks) await write(root, check.staged, check.staged, T_STAGED);
	await write(root, NATIVE, 'native', T_STAGED);
	for (const [path, content] of Object.entries(CORPUS)) await write(root, path, content, T_SOURCE);
	return root;
};

const check_crates = (checks: ReadonlyArray<{ crates: Array<string> }>): Set<string> =>
	new Set(checks.flatMap((check) => check.crates));

const write = async (root: FsDir, path: string, content: string, ms: number): Promise<void> => {
	const dir = path.split('/').slice(0, -1).join('/');
	if (dir !== '') await root.mkdir(dir, { recursive: true });
	await root.write(path, content);
	await root.set_times(path, { mtime_ms: ms });
};

/** One call a fake program took. */
interface Call {
	args: Array<string>;
	/** The working directory's path, with the checkout's and each temp root's replaced. */
	cwd: string;
}

const result = (code: number, stdout = '', stderr = ''): SpawnOutput => ({
	kind: 'exited',
	code,
	stdout,
	stderr,
	truncated: false
});

/** What a fake CLI answers for `args` run in `cwd`: the empty-scope refusal, a `--list` of
 * the tree, a strip of the file, and a format that changes nothing. */
const cli_answer = async (args: ReadonlyArray<string>, cwd: FsReadDir): Promise<SpawnOutput> => {
	if (args[0] === 'strip') return result(0, `stripped ${args[1]}\n`);
	if (args[0] === 'format' && args[1] === '--list') return result(0, 'a.ts\nsub/b.css\n');
	let empty = true;
	for await (const _ of cwd.list()) empty = false;
	return empty ? result(2, '', `${REFUSAL}\n`) : result(0, '', 'checked 3 files\n');
};

interface Fakes {
	/** Overrides the native CLI's answer; `undefined` falls back to `cli_answer`. */
	native?: (args: ReadonlyArray<string>, cwd: FsReadDir) => Promise<SpawnOutput | undefined>;
	/** Overrides the WASM CLI's answer, as `native`. */
	wasm?: (args: ReadonlyArray<string>, cwd: FsReadDir) => Promise<SpawnOutput | undefined>;
	/** Overrides the in-process child's answer, given its file list. */
	engines?: (files: Array<[string, string]>) => SpawnOutput;
	/** `node --version`'s answer. */
	version?: SpawnOutput;
	node?: false;
	host?: string;
}

/** The deps over `checkout`, the fakes' calls, and what the run printed. */
const make_deps = (checkout: FsDir, fakes: Fakes = {}) => {
	const temp_roots: Array<string> = [];
	const calls: Record<'native' | 'node', Array<Call>> = { native: [], node: [] };
	const consumers: Array<{ files: Record<string, string> }> = [];
	const where = (cwd: FsReadDir | undefined): string => {
		const path = cwd?.path ?? '(none)';
		if (path === checkout.path) return '<checkout>';
		const temp = temp_roots.findIndex((root) => path.startsWith(root));
		return temp === -1 ? path : `<temp ${temp}>${path.slice(temp_roots[temp]!.length)}`;
	};
	const unused = (): never => {
		throw new Error('only out is faked');
	};
	const fake = (
		name: 'native' | 'node',
		answer: (args: ReadonlyArray<string>, cwd: FsReadDir) => Promise<SpawnOutput>
	): SpawnProgram => ({
		name,
		path: `/fake/${name}`,
		out: async (args, options) => {
			calls[name].push({ args: [...args], cwd: where(options?.cwd) });
			return answer(args, options?.cwd as FsReadDir);
		},
		run: unused,
		out_bytes: unused,
		start: unused
	});
	const native_programs: Array<string> = [];
	const out: Array<string> = [];
	const err: Array<string> = [];
	const deps: EngineParityDeps = {
		checkout: checkout.readonly(),
		// a root beside the checkout (the snapshot's) is absent here
		open_root: async (root) =>
			!root.startsWith('../') && (await checkout.exists(root))
				? checkout.dir(root).readonly()
				: null,
		temp_dir: async () => {
			const dir = await fs.temp_dir('tsv_engine_parity_test_temp_');
			temp_roots.push(dir.path);
			return dir;
		},
		host_triple: fakes.host ?? TRIPLE,
		native_program: (path) => {
			native_programs.push(path);
			return fake(
				'native',
				async (args, cwd) => (await fakes.native?.(args, cwd)) ?? cli_answer(args, cwd)
			);
		},
		node:
			fakes.node === false
				? null
				: fake('node', async (args, cwd) => {
						if (args[0] === '--version') return fakes.version ?? result(0, 'v24.14.1\n');
						if (args[0] === join(checkout.path, WASM_CLI)) {
							const rest = args.slice(1);
							return (await fakes.wasm?.(rest, cwd)) ?? cli_answer(rest, cwd);
						}
						// the in-process child: its script, the wasm entry, and the file list
						const consumer: Record<string, string> = {};
						for (const path of [
							'engine_parity.mjs',
							'files.json',
							'node_modules/@fuzdev/tsv/index.js',
							'node_modules/@fuzdev/tsv/cli.js',
							`node_modules/@fuzdev/tsv-${TRIPLE}/tsv_napi.node`
						]) {
							consumer[path] = (await cwd.read_text_if_exists(path)) ?? '(missing)';
						}
						consumers.push({ files: consumer });
						const files = JSON.parse(consumer['files.json']!) as Array<[string, string]>;
						return (
							fakes.engines?.(files) ??
							result(0, JSON.stringify({ files: files.length, differing: [] }))
						);
					}),
		stale_ok: false,
		print: (line) => out.push(line),
		print_error: (line) => err.push(line)
	};
	return { deps, calls, consumers, native_programs, out, err, temp_roots };
};

test('agreeing engines: every leg graded, each program handed its arguments where they run', async () => {
	await using checkout = await make_checkout();
	const { deps, calls, consumers, native_programs, out, err, temp_roots } = make_deps(checkout);
	strictEqual(await run_engine_parity(deps, ['corpus']), 0);
	deepStrictEqual(err, []);
	deepStrictEqual(out, [
		`engines: native = staged platform package (${TRIPLE}), wasm = ${WASM_CLI}`,
		'  ✓ corpus: 4 files, byte-identical — checked 3 files',
		'  ✓ corpus (tsv strip): 2 files, byte-identical',
		'  ✓ corpus (strip_typescript): 2 files, byte-identical',
		'  ✓ corpus (parse_imports_*): 2 files, byte-identical',
		'✓ the two engines agree on every byte across 10 files'
	]);
	deepStrictEqual(native_programs, [NATIVE]);
	const wasm_cli = join(checkout.path, WASM_CLI);
	deepStrictEqual(calls.native, [
		{ args: ['format', '.'], cwd: '<temp 0>' },
		{ args: ['format', '.'], cwd: '<temp 1>/native' },
		{ args: ['format', '--list', './/', './', '.'], cwd: '<temp 1>/native' },
		{ args: ['strip', 'corpus/a.ts'], cwd: '<checkout>' },
		{ args: ['strip', 'corpus/sub/c.TS'], cwd: '<checkout>' }
	]);
	const index_url = pathToFileURL(join(checkout.path, 'crates/tsv_wasm/pkg/all/npm/index.js')).href;
	// the temp roots by the order they were made: the empty scope, the two trees, then one
	// consumer per in-process leg
	const temp = (n: number, file: string): string => join(temp_roots[n]!, file);
	deepStrictEqual(calls.node, [
		{ args: ['--version'], cwd: '(none)' },
		{ args: [wasm_cli, 'format', '.'], cwd: '<temp 1>/wasm' },
		{ args: [wasm_cli, 'format', '--list', './/', './', '.'], cwd: '<temp 1>/wasm' },
		{ args: [wasm_cli, 'strip', 'corpus/a.ts'], cwd: '<checkout>' },
		{ args: [wasm_cli, 'strip', 'corpus/sub/c.TS'], cwd: '<checkout>' },
		{
			args: [temp(2, 'engine_parity.mjs'), index_url, temp(2, 'files.json')],
			cwd: '<temp 2>'
		},
		{
			args: [temp(3, 'engine_parity.mjs'), index_url, temp(3, 'files.json')],
			cwd: '<temp 3>'
		}
	]);
	// the consumer project: the script, the list (absolute paths, each with its export), and
	// the staged loader and platform package under `node_modules/@fuzdev/`
	const absolute = (file: string): string => join(checkout.path, 'corpus', file);
	deepStrictEqual(
		consumers.map(({ files }) => JSON.parse(files['files.json']!)),
		[
			[
				[absolute('a.ts'), 'strip_typescript'],
				[absolute('sub/c.TS'), 'strip_typescript']
			],
			[
				[absolute('a.ts'), 'parse_imports_typescript'],
				[absolute('sub/c.TS'), 'parse_imports_typescript']
			]
		]
	);
	for (const { files } of consumers) {
		ok(files['engine_parity.mjs']!.includes("import * as native from '@fuzdev/tsv';"));
		strictEqual(files['node_modules/@fuzdev/tsv/index.js'], 'crates/tsv_napi/pkg/napi/index.js');
		strictEqual(files['node_modules/@fuzdev/tsv/cli.js'], 'crates/tsv_napi/pkg/napi/cli.js');
		strictEqual(
			files[`node_modules/@fuzdev/tsv-${TRIPLE}/tsv_napi.node`],
			`crates/tsv_napi/pkg/${TRIPLE}/tsv_napi.node`
		);
	}
});

test('the trees each engine formats hold exactly the files in its extensions, and nothing outlives the run', async () => {
	await using checkout = await make_checkout();
	const seen: Array<Record<string, string>> = [];
	const snapshot = async (dir: FsReadDir, path = '', into: Record<string, string> = {}) => {
		for await (const entry of dir.list(path === '' ? undefined : path)) {
			const rel = path === '' ? entry.name : `${path}/${entry.name}`;
			if (entry.kind === 'directory') await snapshot(dir, rel, into);
			else into[rel] = await dir.read_text(rel);
		}
		return into;
	};
	const record = async (args: ReadonlyArray<string>, cwd: FsReadDir) => {
		if (args[0] === 'format' && args[1] === '.' && cwd.path.match(/\/(native|wasm)$/)) {
			seen.push(await snapshot(cwd));
		}
		return undefined;
	};
	const { deps, temp_roots } = make_deps(checkout, { native: record, wasm: record });
	strictEqual(await run_engine_parity(deps, ['corpus']), 0);
	const tree = {
		'a.ts': 'let a',
		'sub/b.css': 'b {}',
		'sub/c.TS': 'let c',
		'node_modules/m/y.ts': 'let y'
	};
	deepStrictEqual(seen, [tree, tree]);
	for (const root of temp_roots) {
		const gone = await fs.open_dir(root).then(
			() => false,
			() => true
		);
		strictEqual(gone, true, root);
	}
});

test('each way the format leg can disagree is a finding of its own', async () => {
	await using checkout = await make_checkout();
	const { deps, out, err } = make_deps(checkout, {
		wasm: async (args, cwd) => {
			if (args[0] === 'format' && args[1] === '--list') return result(0, 'a.ts\n');
			if (args[0] === 'format' && cwd.path.endsWith('/wasm')) {
				await (cwd as FsDir).write('sub/b.css', 'b{}');
				return result(1, 'sub/b.css\n', 'other\n');
			}
			return undefined;
		}
	});
	strictEqual(await run_engine_parity(deps, ['corpus']), 1);
	deepStrictEqual(out.slice(1, 7), [
		'  ✗ corpus: 4 files',
		'      exit code: native 0, wasm 1',
		'      the changed-path lists differ (stdout)',
		'      the diagnostics differ (stderr)',
		'      the --list over spelled, overlapping roots differs',
		'      bytes differ: sub/b.css'
	]);
	deepStrictEqual(err, [
		'\n✗ the WASM and native engines disagree on 1 root(s). One Rust source compiled two ways ' +
			'must format to the same bytes; a difference here ships as a formatter whose output ' +
			'depends on which package the user installed.'
	]);
});

/** A run a signal ended, with what it wrote before. */
const signaled = (signal: string, stdout = '', stderr = ''): SpawnOutput => ({
	kind: 'signaled',
	signal,
	stdout,
	stderr,
	truncated: false
});

test('an engine a signal ended is a finding, even when the other ended alike', async () => {
	await using checkout = await make_checkout();
	// both killed by the same signal, with the same output and the files equally untouched: every
	// other comparison agrees
	const killed = async (args: ReadonlyArray<string>, cwd: FsReadDir) => {
		if (args[0] !== 'format') return undefined;
		// the empty-scope refusal the extension set is read off answers as ever
		for await (const _ of cwd.list()) return signaled('SIGKILL');
		return undefined;
	};
	const { deps, out } = make_deps(checkout, { native: killed, wasm: killed });
	strictEqual(await run_engine_parity(deps, ['corpus']), 1);
	deepStrictEqual(out.slice(1, 6), [
		'  ✗ corpus: 4 files',
		'      native killed by SIGKILL',
		'      wasm killed by SIGKILL',
		'      the --list over spelled, overlapping roots: native killed by SIGKILL',
		'      the --list over spelled, overlapping roots: wasm killed by SIGKILL'
	]);
});

test('a strip one engine was killed on is a finding naming it, whatever the other did', async () => {
	await using checkout = await make_checkout();
	const { deps, out } = make_deps(checkout, {
		native: async (args) =>
			args[0] === 'strip' && args[1] === 'corpus/a.ts' ? signaled('SIGABRT') : undefined,
		wasm: async (args) =>
			args[0] === 'strip' && args[1] === 'corpus/a.ts' ? signaled('SIGABRT') : undefined
	});
	strictEqual(await run_engine_parity(deps, ['corpus']), 1);
	deepStrictEqual(out.slice(2, 4), [
		'  ✗ corpus (tsv strip): 2 files',
		'      tsv strip corpus/a.ts: exit native 134 / wasm 134, native killed by SIGABRT, wasm killed by SIGABRT'
	]);
});

test('past ten differing files the rest are counted, in the order the walk lists them', async () => {
	await using checkout = await make_checkout();
	const names = Array.from({ length: 13 }, (_, i) => `f${String(i).padStart(2, '0')}.ts`);
	// written out of order, so a listing in creation order would differ from the sorted one
	for (const name of [...names].reverse()) await write(checkout, `many/${name}`, 'x', T_SOURCE);
	const { deps, out } = make_deps(checkout, {
		wasm: async (args, cwd) => {
			if (args[0] === 'format' && args[1] === '.' && cwd.path.endsWith('/wasm')) {
				for (const name of names) await (cwd as FsDir).write(name, 'y');
				return result(0, '', 'checked 3 files\n');
			}
			return undefined;
		}
	});
	strictEqual(await run_engine_parity(deps, ['many']), 1);
	deepStrictEqual(out.slice(1, 13), [
		'  ✗ many: 13 files',
		...names.slice(0, 10).map((name) => `      bytes differ: ${name}`),
		'      … and 3 more file(s) with differing bytes'
	]);
});

test('a strip the CLIs disagree on names the file and what differs', async () => {
	await using checkout = await make_checkout();
	const { deps, out } = make_deps(checkout, {
		wasm: async (args) =>
			args[0] === 'strip' && args[1] === 'corpus/sub/c.TS'
				? result(1, 'other\n', 'why\n')
				: undefined
	});
	strictEqual(await run_engine_parity(deps, ['corpus']), 1);
	deepStrictEqual(out.slice(2, 4), [
		'  ✗ corpus (tsv strip): 2 files',
		'      tsv strip corpus/sub/c.TS: exit native 0 / wasm 1, stdout differs, stderr differs'
	]);
});

test('the in-process half reports what its child found, and a child that failed', async () => {
	await using checkout = await make_checkout();
	let call = 0;
	const { deps, out } = make_deps(checkout, {
		engines: (files) => {
			call++;
			return call === 1
				? result(0, JSON.stringify({ files: files.length - 1, differing: [files[1]![0]] }))
				: result(3, '', ' boom \n');
		}
	});
	strictEqual(await run_engine_parity(deps, ['corpus']), 1);
	deepStrictEqual(out.slice(3), [
		'  ✗ corpus (strip_typescript): 2 files',
		`      strip_typescript differs: ${join(checkout.path, 'corpus/sub/c.TS')}`,
		'      compared 1 of 2 files',
		'  ✗ corpus (parse_imports_*): 2 files',
		'      the comparison child failed (exit 3): boom'
	]);
});

test('a svelte file reads through the svelte export; the strip half takes no svelte', async () => {
	await using checkout = await make_checkout();
	await write(checkout, 'corpus/c.svelte', '<p>x</p>', T_SOURCE);
	const { deps, consumers } = make_deps(checkout);
	strictEqual(await run_engine_parity(deps, ['corpus']), 0);
	deepStrictEqual(
		consumers.map(({ files }) =>
			(JSON.parse(files['files.json']!) as Array<[string, string]>).map(
				([file, name]) => `${file.slice(checkout.path.length + 1)} ${name}`
			)
		),
		[
			['corpus/a.ts strip_typescript', 'corpus/sub/c.TS strip_typescript'],
			[
				'corpus/a.ts parse_imports_typescript',
				'corpus/c.svelte parse_imports_svelte',
				'corpus/sub/c.TS parse_imports_typescript'
			]
		]
	);
});

test('an absent root is skipped, and nothing gradable fails', async () => {
	await using checkout = await make_checkout();
	const { deps, out, err } = make_deps(checkout);
	strictEqual(await run_engine_parity(deps, ['nowhere']), 1);
	deepStrictEqual(out.slice(1), [
		'  ⚠ nowhere — skipped (not present)',
		'  ⚠ nowhere — skipped (not present)',
		'  ⚠ nowhere (strip_typescript) — skipped (not present)',
		'  ⚠ nowhere (parse_imports_*) — skipped (not present)'
	]);
	deepStrictEqual(err, ['✗ no corpus root was gradable — nothing was compared']);
});

test('the default roots, when none is named', async () => {
	await using checkout = await make_checkout();
	const { deps, out } = make_deps(checkout);
	strictEqual(await run_engine_parity(deps, []), 1);
	deepStrictEqual(out.slice(1), [
		'  ⚠ tests/fixtures — skipped (not present)',
		'  ⚠ tests/fixtures_compile — skipped (not present)',
		'  ⚠ ../corpora/collections — skipped (not present)',
		'  ⚠ tests/fixtures_strip — skipped (not present)',
		'  ⚠ tests/fixtures — skipped (not present)',
		'  ⚠ ../corpora/collections (strip_typescript) — skipped (not present)',
		'  ⚠ tests/fixtures_imports (parse_imports_*) — skipped (not present)',
		'  ⚠ ../corpora/collections (parse_imports_*) — skipped (not present)'
	]);
});

test('--json prints the native label and every report, the runs included', async () => {
	await using checkout = await make_checkout();
	const { deps, out } = make_deps(checkout);
	strictEqual(await run_engine_parity(deps, ['--json', 'corpus']), 0);
	strictEqual(out.length, 2);
	const report = JSON.parse(out[0]!) as {
		native: string;
		reports: Array<{ root: string; native?: unknown }>;
	};
	strictEqual(report.native, `staged platform package (${TRIPLE})`);
	deepStrictEqual(
		report.reports.map((r) => r.root),
		['corpus', 'corpus (tsv strip)', 'corpus (strip_typescript)', 'corpus (parse_imports_*)']
	);
	deepStrictEqual(report.reports[0]!.native, {
		status: 0,
		stdout: '',
		stderr: 'checked 3 files\n'
	});
	strictEqual(out[1], '✓ the two engines agree on every byte across 10 files');
});

test('no node, or one that fails its version probe, is refused before anything runs', async () => {
	for (const fakes of [{ node: false as const }, { version: result(1) }]) {
		await using checkout = await make_checkout();
		const { deps, calls, out, err } = make_deps(checkout, fakes);
		strictEqual(await run_engine_parity(deps, ['corpus']), 1);
		deepStrictEqual(out, []);
		deepStrictEqual(err, ['✗ engine parity needs `node` — the host the WASM CLI ships for']);
		deepStrictEqual(calls.native, []);
	}
});

test('a missing engine is refused, naming each and its build', async () => {
	await using checkout = await make_checkout();
	await checkout.remove(NATIVE);
	await checkout.remove(WASM_CLI);
	const { deps, native_programs, err } = make_deps(checkout);
	strictEqual(await run_engine_parity(deps, ['corpus']), 1);
	deepStrictEqual(native_programs, []);
	deepStrictEqual(err, [
		'✗ engine parity needs both engines built — missing: the native tsv_cli binary (npm run build:napi:packages), the wasm package (npm run build:npm:all)'
	]);
});

test('the staged N-API packages must be the loader and one platform package', async () => {
	await using checkout = await make_checkout();
	await checkout.mkdir('crates/tsv_napi/pkg/linux-arm64-gnu');
	const two = make_deps(checkout);
	strictEqual(await run_engine_parity(two.deps, ['corpus']), 1);
	deepStrictEqual(two.err, [
		'expected exactly one staged platform package under crates/tsv_napi/pkg, got: linux-arm64-gnu, linux-x64-gnu'
	]);
	await checkout.remove('crates/tsv_napi/pkg/napi', { recursive: true });
	const none = make_deps(checkout);
	strictEqual(await run_engine_parity(none.deps, ['corpus']), 1);
	deepStrictEqual(none.err, [
		"crates/tsv_napi/pkg/napi not staged. Run 'npm run build:napi:packages' first."
	]);
});

test('a stale binary is refused before anything is graded, and only warns under `stale_ok`', async () => {
	await using checkout = await make_checkout();
	await checkout.set_times('crates/tsv_cli/src/lib.rs', { mtime_ms: T_LATER });
	const stale = make_deps(checkout);
	strictEqual(await run_engine_parity(stale.deps, ['corpus']), 1);
	deepStrictEqual(stale.out, []);
	deepStrictEqual(stale.calls.native, []);
	ok(stale.err[0]!.includes('  • native tsv_cli binary: staged '));
	ok(stale.err[0]!.includes('but tsv_cli/src/lib.rs changed'));
	const warned = make_deps(checkout);
	warned.deps.stale_ok = true;
	strictEqual(await run_engine_parity(warned.deps, ['corpus']), 0);
	strictEqual(
		warned.err[0]!.split('\n')[1],
		'⚠ Stale staged package artifacts (BENCH_STALE_OK=1 — grading anyway).'
	);
	// a staging source dates a staged binary; the platform package's addon is dated too
	await checkout.set_times('crates/tsv_cli/src/lib.rs', { mtime_ms: T_SOURCE });
	await checkout.set_times('scripts/roots/build_napi_packages.ts', { mtime_ms: T_LATER });
	const staging = make_deps(checkout);
	strictEqual(await run_engine_parity(staging.deps, ['corpus']), 1);
	ok(staging.err[0]!.includes('  • native tsv_cli binary: staged '));
	ok(staging.err[0]!.includes('  • staged N-API addon: staged '));
});

test('an extension set the native binary does not state, or states with another exit, is refused', async () => {
	for (const [answer, stderr] of [
		[result(2, '', 'odd\n'), 'odd\n'],
		// the set, but not from the refusal: a format that found files exits 0 or 1, never 2
		[result(0, '', `${REFUSAL}\n`), `${REFUSAL}\n`]
	] as const) {
		await using checkout = await make_checkout();
		const { deps, out, err } = make_deps(checkout, {
			native: async (args, cwd) =>
				args[0] === 'format' && !cwd.path.match(/\/(native|wasm)$/) ? answer : undefined
		});
		strictEqual(await run_engine_parity(deps, ['corpus']), 1);
		deepStrictEqual(out, []);
		deepStrictEqual(err, [
			`✗ could not read the extension set off the native binary's empty-scope refusal:\n${stderr}`
		]);
	}
});

test('find_native_cli: the host package first, then the others in listed order, then target/release', async () => {
	await using checkout = await fs.temp_dir('tsv_engine_parity_find_test_');
	strictEqual(await find_native_cli(checkout, TRIPLE), undefined);
	await write(checkout, 'target/release/tsv', 'x', T_SOURCE);
	deepStrictEqual(await find_native_cli(checkout, TRIPLE), {
		path: 'target/release/tsv',
		label: 'target/release'
	});
	await write(checkout, 'crates/tsv_napi/pkg/napi/tsv', 'loader dir', T_SOURCE);
	strictEqual((await find_native_cli(checkout, TRIPLE))!.label, 'target/release');
	// the last by name written last: a listing newest first (a tmpfs) puts it first, and only the
	// sort restores the order
	await write(checkout, 'crates/tsv_napi/pkg/b-other/tsv', 'x', T_SOURCE);
	await write(checkout, 'crates/tsv_napi/pkg/z-other/tsv', 'x', T_SOURCE);
	deepStrictEqual(await find_native_cli(checkout, TRIPLE), {
		path: 'crates/tsv_napi/pkg/b-other/tsv',
		label: 'staged platform package (b-other)'
	});
	await write(checkout, `crates/tsv_napi/pkg/${TRIPLE}/tsv`, 'x', T_SOURCE);
	deepStrictEqual(await find_native_cli(checkout, TRIPLE), {
		path: `crates/tsv_napi/pkg/${TRIPLE}/tsv`,
		label: `staged platform package (${TRIPLE})`
	});
	// a Windows host's binary is `tsv.exe`
	await write(checkout, 'crates/tsv_napi/pkg/win32-x64/tsv.exe', 'x', T_SOURCE);
	deepStrictEqual(await find_native_cli(checkout, 'win32-x64'), {
		path: 'crates/tsv_napi/pkg/win32-x64/tsv.exe',
		label: 'staged platform package (win32-x64)'
	});
});
