/**
 * `run_validate_artifacts` over a temp checkout: every line a fresh, fully built tree prints;
 * the size bounds and the deltas between variants; the third-party notice; an unbuilt variant
 * skipped, a half-built npm staging failed, and nothing built refused; and the staleness guard,
 * which dates each deno bundle by what wasm-pack builds it from and each npm staging by the
 * shared staged-package checks, refuses a stale or missing artifact, and only warns under
 * `stale_ok`.
 */

import { deepStrictEqual, strictEqual } from 'node:assert';

import { fs, type FsDir, type FsReadDir } from '@fuzdev/gro/fs.ts';
import { test } from 'vitest';

import { fmt_mtime as fmt } from '../benches/js/lib/check_artifact_freshness.ts';
import { CORE_CRATES, WASM_CRATES, type WasmVariant } from '../benches/js/lib/tsv_artifacts.ts';
import { wasm_package_checks } from './check_staged_freshness.ts';
import { THIRD_PARTY_NOTICE } from './npm_metadata.ts';
import {
	artifact_freshness_checks,
	BOUNDS,
	DELTAS,
	run_validate_artifacts
} from './validate_artifacts.ts';

/** Minutes apart, so no comparison ties. */
const T_SOURCE = Date.UTC(2026, 0, 1);
const T_BUILT = T_SOURCE + 60_000;
const T_LATER = T_BUILT + 60_000;

const VARIANTS: ReadonlyArray<WasmVariant> = ['format', 'parse', 'all'];
const NOTICE = 'the WHATWG notice\n';

const npm_dir = (variant: WasmVariant): string => `crates/tsv_wasm/pkg/${variant}/npm`;
const deno_bundle = (variant: WasmVariant): string =>
	`crates/tsv_wasm/pkg/${variant}/deno/tsv_wasm_bg.wasm`;

/** Each variant's bundle size: its band's center, so `all` minus a subset sits inside its delta
 * band too (the centers the bounds were cut from differ by the measured deltas). */
const CENTER: Record<WasmVariant, number> = {
	format: 2_720_874,
	parse: 1_073_871,
	all: 3_048_129
};

/**
 * A checkout with every source the freshness checks name dated `T_SOURCE`, and for each
 * variant in `built`, its deno bundle and its npm staging (each staged file the checks name,
 * a `package.json` declaring `NOTICE`, and the notice) dated `T_BUILT`.
 */
const make_checkout = async (
	built: ReadonlyArray<WasmVariant> = VARIANTS
): Promise<FsDir & AsyncDisposable> => {
	const root = await fs.temp_dir('tsv_validate_artifacts_test_');
	const write = async (path: string, content: string, ms: number): Promise<void> => {
		const dir = path.split('/').slice(0, -1).join('/');
		if (dir !== '') await root.mkdir(dir, { recursive: true });
		await root.write(path, content);
		await root.set_times(path, { mtime_ms: ms });
	};
	const sources = new Set<string>(['Cargo.toml', 'Cargo.lock', 'package.json']);
	for (const crate of [...CORE_CRATES, ...WASM_CRATES]) {
		sources.add(`crates/${crate}/Cargo.toml`);
		sources.add(`crates/${crate}/src/lib.rs`);
	}
	for (const variant of VARIANTS) {
		for (const check of wasm_package_checks(variant))
			for (const file of check.files) sources.add(file);
	}
	for (const path of sources) {
		await write(path, path === THIRD_PARTY_NOTICE.source ? NOTICE : path, T_SOURCE);
	}
	for (const variant of built) {
		await write(deno_bundle(variant), 'wasm', T_BUILT);
		for (const check of wasm_package_checks(variant))
			await write(check.staged, check.staged, T_BUILT);
		await write(
			`${npm_dir(variant)}/package.json`,
			JSON.stringify({ files: ['index.js', THIRD_PARTY_NOTICE.published] }),
			T_BUILT
		);
		await write(`${npm_dir(variant)}/${THIRD_PARTY_NOTICE.published}`, NOTICE, T_BUILT);
	}
	return root;
};

/**
 * The checkout as `run_validate_artifacts` reads it, each bundle reporting the size `sizes`
 * gives it (the variant's center by default) instead of the bytes written, so a suite needs
 * no multi-megabyte files.
 */
const sized = (dir: FsReadDir, sizes: Partial<Record<string, number>> = {}): FsReadDir => {
	const size_of = (path: string): number | undefined => {
		if (path in sizes) return sizes[path];
		const match = /^crates\/tsv_wasm\/pkg\/(\w+)\/(?:npm|deno)\/tsv_wasm_bg\.wasm$/.exec(path);
		return match ? CENTER[match[1] as WasmVariant] : undefined;
	};
	const unused = (): never => {
		throw new Error('only the async reads are faked');
	};
	return {
		path: dir.path,
		dir: unused,
		stat: async (path, options) => {
			const info = await dir.stat(path, options);
			const size = size_of(path);
			return info === null || size === undefined ? info : { ...info, size };
		},
		exists: (path) => dir.exists(path),
		read: (path) => dir.read(path),
		read_text: (path) => dir.read_text(path),
		read_if_exists: (path) => dir.read_if_exists(path),
		read_text_if_exists: (path) => dir.read_text_if_exists(path),
		list: (path) => dir.list(path),
		read_link: (path) => dir.read_link(path),
		stat_sync: unused,
		exists_sync: unused,
		read_text_sync: unused
	};
};

/** A run over `checkout`, with what it printed to each stream. */
const run = async (
	checkout: FsReadDir,
	stale_ok = false
): Promise<{ code: number; out: Array<string>; err: Array<string> }> => {
	const out: Array<string> = [];
	const err: Array<string> = [];
	const code = await run_validate_artifacts({
		checkout,
		stale_ok,
		print: (line) => out.push(line),
		print_error: (line) => err.push(line)
	});
	return { code, out, err };
};

const touch = (root: FsDir, path: string, ms = T_LATER): Promise<void> =>
	root.set_times(path, { mtime_ms: ms });

/** What a fresh, fully built checkout prints, each line. */
const FULL_OUT = [
	'=== WASM binary sizes ===',
	'  PASS: format/npm: 2.72 MB (2720874 B)',
	'  PASS: parse/npm: 1.07 MB (1073871 B)',
	'  PASS: all/npm: 3.05 MB (3048129 B)',
	'  PASS: format/deno: 2.72 MB (2720874 B)',
	'  PASS: parse/deno: 1.07 MB (1073871 B)',
	'  PASS: all/deno: 3.05 MB (3048129 B)',
	'  PASS: all - format (npm) = 327.3 KB',
	'  PASS: all - parse (npm) = 1.97 MB',
	'  PASS: all - format (deno) = 327.3 KB',
	'  PASS: all - parse (deno) = 1.97 MB',
	'\n=== Third-party notice ===',
	'  PASS: format/npm — package.json files declares NOTICE',
	'  PASS: format/npm — NOTICE is crates/tsv_html/NOTICE verbatim',
	'  PASS: parse/npm — package.json files declares NOTICE',
	'  PASS: parse/npm — NOTICE is crates/tsv_html/NOTICE verbatim',
	'  PASS: all/npm — package.json files declares NOTICE',
	'  PASS: all/npm — NOTICE is crates/tsv_html/NOTICE verbatim',
	'\n=== Artifact validation: 16 passed, 0 failed, 0 skipped ==='
];

test('a fresh, fully built checkout passes every check and prints each one', async () => {
	await using root = await make_checkout();
	const { code, out, err } = await run(sized(root.readonly()));
	deepStrictEqual(out, FULL_OUT);
	deepStrictEqual(err, []);
	strictEqual(code, 0);
});

test('a bundle outside its band fails, naming the size and the bound', async () => {
	await using root = await make_checkout();
	const small = await run(sized(root.readonly(), { [deno_bundle('parse')]: BOUNDS.parse.min - 1 }));
	strictEqual(small.code, 1);
	strictEqual(
		small.out.find((line) => line.includes('parse/deno:')),
		'  FAIL: parse/deno: 988.0 KB (987999 B) < min 988.0 KB — suspiciously small'
	);
	const big = await run(
		sized(root.readonly(), { [`${npm_dir('format')}/tsv_wasm_bg.wasm`]: BOUNDS.format.max + 1 })
	);
	strictEqual(big.code, 1);
	strictEqual(
		big.out.find((line) => line.includes('format/npm:')),
		'  FAIL: format/npm: 2.94 MB (2938001 B) > max 2.94 MB — size regression'
	);
	// the bounds are inclusive (the deltas this moves are the next test's)
	const edges = await run(
		sized(root.readonly(), {
			[deno_bundle('parse')]: BOUNDS.parse.min,
			[deno_bundle('format')]: BOUNDS.format.max
		})
	);
	deepStrictEqual(
		edges.out.filter((line) => / B\)$/.test(line)),
		[
			'  PASS: format/npm: 2.72 MB (2720874 B)',
			'  PASS: parse/npm: 1.07 MB (1073871 B)',
			'  PASS: all/npm: 3.05 MB (3048129 B)',
			'  PASS: format/deno: 2.94 MB (2938000 B)',
			'  PASS: parse/deno: 988.0 KB (988000 B)',
			'  PASS: all/deno: 3.05 MB (3048129 B)'
		]
	);
});

test("a subset's distance below `all` outside its band fails, per target", async () => {
	await using root = await make_checkout();
	// `all` moved alone, to just under the format delta's floor: on the deno target the format
	// delta leaves its band and the parse delta stays inside, and the npm target's both stay
	const narrow = await run(
		sized(root.readonly(), { [deno_bundle('all')]: CENTER.format + DELTAS.format.min - 1 })
	);
	strictEqual(narrow.code, 1);
	deepStrictEqual(
		narrow.out.filter((line) => line.includes('(deno) =')),
		[
			'  FAIL: all - format (deno) = 301.0 KB — expected ≥301.0 KB (feature gate broken?)',
			'  PASS: all - parse (deno) = 1.95 MB'
		]
	);
	deepStrictEqual(
		narrow.out.filter((line) => line.includes('(npm) =')),
		['  PASS: all - format (npm) = 327.3 KB', '  PASS: all - parse (npm) = 1.97 MB']
	);
	const wide = await run(
		sized(root.readonly(), { [deno_bundle('parse')]: CENTER.all - DELTAS.parse.max - 1 })
	);
	strictEqual(
		wide.out.find((line) => line.includes('all - parse (deno)')),
		'  FAIL: all - parse (deno) = 2.13 MB — expected ≤2.13 MB (unexpected bloat)'
	);
	strictEqual(wide.code, 1);
});

test('a package whose NOTICE is missing, differs, or is undeclared fails that check', async () => {
	await using root = await make_checkout();
	await root.write(`${npm_dir('format')}/${THIRD_PARTY_NOTICE.published}`, 'another notice\n');
	await touch(root, `${npm_dir('format')}/${THIRD_PARTY_NOTICE.published}`, T_BUILT);
	await root.remove(`${npm_dir('parse')}/${THIRD_PARTY_NOTICE.published}`);
	await root.write(`${npm_dir('all')}/package.json`, JSON.stringify({ files: ['index.js'] }));
	await touch(root, `${npm_dir('all')}/package.json`, T_BUILT);
	const { code, out } = await run(sized(root.readonly()));
	strictEqual(code, 1);
	deepStrictEqual(
		out.filter((line) => line.startsWith('  FAIL')),
		[
			'  FAIL: format/npm — NOTICE is crates/tsv_html/NOTICE verbatim returned wrong output',
			'  FAIL: parse/npm — NOTICE is crates/tsv_html/NOTICE verbatim returned wrong output',
			'  FAIL: all/npm — package.json files declares NOTICE returned wrong output'
		]
	);
	strictEqual(out.at(-1), '\n=== Artifact validation: 13 passed, 3 failed, 0 skipped ===');
});

test('an unbuilt variant is skipped, and its deltas with it', async () => {
	await using root = await make_checkout(['format', 'all']);
	// an empty staging directory is no staging
	await root.mkdir(npm_dir('parse'), { recursive: true });
	const { code, out, err } = await run(sized(root.readonly()));
	strictEqual(code, 0);
	deepStrictEqual(err, []);
	deepStrictEqual(
		out.filter((line) => line.includes('SKIP') || line.includes('parse')),
		[
			'  SKIP: parse/npm — not built',
			'  SKIP: parse/deno — not built',
			'  SKIP: parse/npm NOTICE — not built'
		]
	);
	strictEqual(out.at(-1), '\n=== Artifact validation: 10 passed, 0 failed, 3 skipped ===');
});

test('nothing built fails, saying so on stderr', async () => {
	await using root = await make_checkout([]);
	const { code, out, err } = await run(sized(root.readonly()));
	strictEqual(code, 1);
	strictEqual(out.at(-1), '\n=== Artifact validation: 0 passed, 0 failed, 9 skipped ===');
	deepStrictEqual(err, [
		'FAIL: no artifacts found to validate — run `deno task build:npm:format` etc. first'
	]);
});

test('the freshness checks: each deno bundle by its wasm-pack inputs, each npm staging by the shared list', () => {
	const checks = artifact_freshness_checks({
		deno: new Set<WasmVariant>(['parse']),
		npm: new Set<WasmVariant>(['all'])
	});
	deepStrictEqual(checks, [
		{
			label: 'parse/deno WASM bundle',
			staged: deno_bundle('parse'),
			crates: [...CORE_CRATES, ...WASM_CRATES],
			files: ['package.json'],
			rebuild: 'deno task build:wasm:parse:deno'
		},
		...wasm_package_checks('all')
	]);
	deepStrictEqual(
		artifact_freshness_checks({ deno: new Set(VARIANTS), npm: new Set() }).map(
			(check) => check.rebuild
		),
		[
			'deno task build:wasm:deno',
			'deno task build:wasm:parse:deno',
			'deno task build:wasm:all:deno'
		]
	);
});

/** Every input that must stale a deno bundle: its crates' sources, the workspace files, and the
 * task file the wasm-pack flags are written in. */
const DENO_INPUTS = [
	...[...CORE_CRATES, ...WASM_CRATES].flatMap((crate) => [
		`crates/${crate}/Cargo.toml`,
		`crates/${crate}/src/lib.rs`
	]),
	'Cargo.toml',
	'Cargo.lock',
	'package.json'
];

test('a deno bundle older than any of its inputs is refused before anything is graded', async () => {
	await using root = await make_checkout(['parse']);
	// the npm staging's own inputs are graded by the shared list; drop it so only the deno
	// bundle's check reads these
	await root.remove(npm_dir('parse'), { recursive: true });
	for (const input of DENO_INPUTS) {
		await touch(root, input);
		const { code, out, err } = await run(sized(root.readonly()));
		strictEqual(code, 1, input);
		deepStrictEqual(out, [], input);
		strictEqual(err.length, 1, input);
		strictEqual(
			err[0]!.split('\n').slice(0, 4).join('\n'),
			'\n✗ Stale staged package artifacts — refusing to grade outdated code.\n' +
				`  • parse/deno WASM bundle: staged ${fmt(T_BUILT)}, but ` +
				`${input.startsWith('crates/') ? input.slice('crates/'.length) : input} changed ${fmt(T_LATER)}\n` +
				'      restage: deno task build:wasm:parse:deno',
			input
		);
		await touch(root, input, T_SOURCE);
	}
	const fresh = await run(sized(root.readonly()));
	strictEqual(fresh.code, 0);
	deepStrictEqual(fresh.err, []);
});

test('an npm staging older than an input of the shared list is refused', async () => {
	await using root = await make_checkout(['format']);
	await touch(root, 'scripts/roots/patch_npm_package.ts');
	const { code, out, err } = await run(sized(root.readonly()));
	strictEqual(code, 1);
	deepStrictEqual(out, []);
	strictEqual(
		err[0]!.split('\n')[2],
		`  • staged package entries (format): staged ${fmt(T_BUILT)}, but scripts/roots/patch_npm_package.ts changed ${fmt(T_LATER)}`
	);
});

test('under `stale_ok` a stale artifact warns and the run grades it', async () => {
	await using root = await make_checkout();
	await touch(root, 'crates/tsv_wasm/src/lib.rs');
	const { code, out, err } = await run(sized(root.readonly()), true);
	strictEqual(code, 0);
	deepStrictEqual(out, FULL_OUT);
	strictEqual(err.length, 1);
	strictEqual(
		err[0]!.split('\n')[1],
		'⚠ Stale staged package artifacts (BENCH_STALE_OK=1 — grading anyway).'
	);
	strictEqual(err[0]!.includes('restage: deno task build:wasm:deno'), true);
	strictEqual(err[0]!.includes('restage: deno task build:npm:all'), true);
});

test('an npm staging that lacks its bundle is refused as missing, not skipped as unbuilt', async () => {
	await using root = await make_checkout();
	await root.remove(`${npm_dir('parse')}/tsv_wasm_bg.wasm`);
	const { code, out, err } = await run(sized(root.readonly()), true);
	strictEqual(code, 1);
	deepStrictEqual(out, []);
	deepStrictEqual(err[0]!.split('\n').slice(1, 4), [
		'✗ Stale staged package artifacts — refusing to grade outdated code.',
		`  • staged WASM bundle (parse): not staged — ${npm_dir('parse')}/tsv_wasm_bg.wasm`,
		'      restage: deno task build:npm:parse'
	]);
});

test('an npm staging holding its entries without its bundle or manifest is refused as missing', async () => {
	await using root = await make_checkout();
	await root.remove(`${npm_dir('parse')}/tsv_wasm_bg.wasm`);
	await root.remove(`${npm_dir('parse')}/package.json`);
	const entries = await run(sized(root.readonly()), true);
	strictEqual(entries.code, 1);
	deepStrictEqual(entries.out, []);
	strictEqual(
		entries.err[0]!.split('\n')[2],
		`  • staged WASM bundle (parse): not staged — ${npm_dir('parse')}/tsv_wasm_bg.wasm`
	);
	// the worker entry alone, which no shared check names
	await using lone = await make_checkout(['format', 'all']);
	await lone.mkdir(npm_dir('parse'), { recursive: true });
	await lone.write(`${npm_dir('parse')}/worker.js`, "export * from './browser.js';\n");
	const worker = await run(sized(lone.readonly()), true);
	strictEqual(worker.code, 1);
	deepStrictEqual(worker.out, []);
	strictEqual(
		worker.err[0]!.split('\n')[2],
		`  • staged WASM bundle (parse): not staged — ${npm_dir('parse')}/tsv_wasm_bg.wasm`
	);
});

test('an npm staging holding its bundle without its manifest fails, whatever else it holds', async () => {
	await using root = await make_checkout();
	// the bundle alone, as wasm-pack leaves it partway: refused for the entries it lacks
	for (const check of wasm_package_checks('format').slice(1)) await root.remove(check.staged);
	await root.remove(`${npm_dir('format')}/package.json`);
	const bare = await run(sized(root.readonly()));
	strictEqual(bare.code, 1);
	deepStrictEqual(bare.out, []);
	strictEqual(
		bare.err[0]!.split('\n')[2],
		`  • staged package entries (format): not staged — ${npm_dir('format')}/index.js`
	);
	// everything but the manifest: it declares no NOTICE
	await using full = await make_checkout();
	await full.remove(`${npm_dir('all')}/package.json`);
	const { code, out, err } = await run(sized(full.readonly()));
	strictEqual(code, 1);
	deepStrictEqual(err, []);
	deepStrictEqual(
		out.filter((line) => !line.startsWith('  PASS')),
		[
			'=== WASM binary sizes ===',
			'\n=== Third-party notice ===',
			'  FAIL: all/npm — package.json files declares NOTICE returned wrong output',
			'\n=== Artifact validation: 15 passed, 1 failed, 0 skipped ==='
		]
	);
});

test('a staged manifest that does not parse fails its check, and the run grades the rest', async () => {
	await using root = await make_checkout();
	await root.write(`${npm_dir('parse')}/package.json`, '{"files": ');
	await touch(root, `${npm_dir('parse')}/package.json`, T_BUILT);
	const { code, out, err } = await run(sized(root.readonly()));
	strictEqual(code, 1);
	deepStrictEqual(err, []);
	const failed = out.filter((line) => line.startsWith('  FAIL'));
	strictEqual(failed.length, 1);
	strictEqual(
		failed[0]!.startsWith(
			'  FAIL: parse/npm — package.json files declares NOTICE threw: SyntaxError'
		),
		true
	);
	strictEqual(out.at(-1), '\n=== Artifact validation: 15 passed, 1 failed, 0 skipped ===');
});

test('a staged file the checks name that is missing is fatal under `stale_ok` too', async () => {
	await using root = await make_checkout();
	await root.remove(`${npm_dir('all')}/cli.js`);
	const { code, out, err } = await run(sized(root.readonly()), true);
	strictEqual(code, 1);
	deepStrictEqual(out, []);
	deepStrictEqual(err[0]!.split('\n').slice(1, 4), [
		'✗ Stale staged package artifacts — refusing to grade outdated code.',
		`  • staged cli.js (all): not staged — ${npm_dir('all')}/cli.js`,
		'      restage: deno task build:npm:all'
	]);
});
