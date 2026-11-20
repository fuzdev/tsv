/**
 * `run_doctor` over in-memory deps: a provisioned machine passes clean, and each check's pass,
 * warn, and fail line and what it does to the exit code, with each probed program present,
 * absent, at another version, printing garbage, or exiting non-zero; a present sibling that can't
 * say what it holds. Also what each program is run with.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';

import { FsError } from '@fuzdev/gro/fs_error.ts';
import { afterEach, beforeEach, describe, test, vi } from 'vitest';

import { GATE_CHECKOUT_IDS } from '../benches/js/lib/gate_counts.ts';
import { HARVEST_STAMPS } from '../benches/js/lib/harvest_stamp_table.ts';
import { native_library_filename } from '../benches/js/lib/runtime.ts';
import type { AllVersions } from '../benches/js/lib/versions.ts';
import {
	DENO_MISSING,
	DOCTOR_SIBLINGS,
	type DoctorCorpus,
	type DoctorDeps,
	type DoctorDir,
	type DoctorOutput,
	type DoctorProgram,
	type DoctorPrograms,
	type DoctorSibling,
	run_doctor
} from './doctor.ts';

/** How a fake program answers each call, by its argv joined with spaces, or `'*'`. */
type Answers = Record<string, DoctorOutput | Error>;

/** A run that exited `code` printing `stdout`. */
const exited = (stdout: string, code = 0, stderr = ''): DoctorOutput => ({
	kind: 'exited',
	code,
	stdout,
	stderr
});

/** A fake program recording each call's argv. */
interface FakeProgram extends DoctorProgram {
	calls: string[][];
}

const fake_program = (answers: Answers): FakeProgram => {
	const calls: string[][] = [];
	return {
		calls,
		out: (args) => {
			calls.push([...args]);
			const answer = answers[args.join(' ')] ?? answers['*'];
			if (answer === undefined) return Promise.reject(new Error(`unexpected ${args.join(' ')}`));
			return answer instanceof Error ? Promise.reject(answer) : Promise.resolve(answer);
		}
	};
};

/** A directory over `{path: text}`; every parent of a file exists. `null` text is a directory. */
const fake_dir = (files: Record<string, string | null>): DoctorDir => {
	const has = (path: string): boolean =>
		path in files || Object.keys(files).some((f) => f.startsWith(`${path}/`));
	const read = (path: string): string => {
		const text = files[path];
		if (typeof text !== 'string') throw new FsError('not_found', path);
		return text;
	};
	return {
		exists: (path) => Promise.resolve(has(path)),
		read_text: (path) => {
			try {
				return Promise.resolve(read(path));
			} catch (error) {
				return Promise.reject(error);
			}
		},
		read_text_sync: read,
		list: async function* (path = '') {
			if (!has(path)) throw new FsError('not_found', path);
			if (typeof files[path] === 'string') throw new FsError('not_a_directory', path);
			const prefix = path === '' ? '' : `${path}/`;
			const seen = new Set<string>();
			for (const f of Object.keys(files)) {
				if (!f.startsWith(prefix)) continue;
				const [name, ...rest] = f.slice(prefix.length).split('/');
				if (seen.has(name!)) continue;
				seen.add(name!);
				yield { name: name!, kind: rest.length > 0 || files[f] === null ? 'directory' : 'file' };
			}
		}
	};
};

/** A directory every read of which fails, as an unreadable one does. */
const failing_dir: DoctorDir = {
	exists: () => Promise.reject(new FsError('permission_denied', 'x')),
	read_text: () => Promise.reject(new FsError('permission_denied', 'x')),
	read_text_sync: () => {
		throw new FsError('permission_denied', 'x');
	},
	list: async function* () {
		yield* [];
		throw new FsError('permission_denied', 'x');
	}
};

const FFI = native_library_filename('tsv_ffi');
const NAPI = native_library_filename('tsv_napi');

const CORPORA_HASH = `${GATE_CHECKOUT_IDS['../corpora']!.hash}0000000000000000000000000000000`;

/** The commit each checkout's git reports, by the path `git -C` names. */
const HEADS: Record<string, string> = {
	'../wpt/css': 'a'.repeat(40),
	'../test262': 'b'.repeat(40),
	'../typescript': 'c'.repeat(40),
	'../svelte': 'd'.repeat(40),
	'../prettier': 'e'.repeat(40),
	'../prettier-plugin-svelte': 'f'.repeat(40),
	'../wpt': '1'.repeat(40)
};

/** Each stamp recording exactly what the fake git reports. */
const fresh_stamps = (): Record<string, string> =>
	Object.fromEntries(
		Object.values(HARVEST_STAMPS).map(({ path, checkouts }) => [
			path,
			JSON.stringify(
				Object.fromEntries(
					Object.entries(checkouts).map(([key, input]) => [
						key,
						typeof input === 'string' ? HEADS[input] : CORPORA_HASH
					])
				)
			)
		])
	);

const SIDECAR_LOCK = JSON.stringify({
	packages: { '': { name: 'sidecar' }, 'node_modules/svelte': { version: '5.0.0' } }
});

/** Everything a fully provisioned machine has, each part replaceable by a case. */
interface World {
	checkout: Record<string, string | null>;
	target: Record<string, string | null> | null;
	siblings: Record<DoctorSibling, Record<string, string | null> | null>;
	/** A sibling's own directory, in place of one over its files. */
	sibling_dirs: Partial<Record<DoctorSibling, DoctorDir>>;
	programs: Record<keyof DoctorPrograms, Answers | null>;
	probe_node_modules: DoctorDeps['probe_node_modules'];
	load_all_versions: DoctorDeps['load_all_versions'];
	load_corpus: DoctorDeps['load_corpus'];
}

const healthy = (): World => ({
	checkout: {
		'.github/workflows/check.yml':
			"jobs:\n  a:\n    node-version: '24.14.1'\n  b:\n    node-version: '24.14.1'\n",
		'.github/workflows/release.yml': "jobs:\n  a:\n    node-version: '24.14.1'\n",
		'.github/workflows/notes.txt': "node-version: '1.0.0'\n",
		'package.json': JSON.stringify({ engines: { node: '>=24.14' } }),
		'crates/tsv_debug/sidecar/package.json': JSON.stringify({ engines: { node: '>=24.14' } }),
		'benches/js/package.json': JSON.stringify({ engines: { node: '>=22.18' } }),
		'crates/tsv_debug/sidecar/package-lock.json': SIDECAR_LOCK,
		'crates/tsv_debug/sidecar/node_modules/.package-lock.json': SIDECAR_LOCK,
		'crates/tsv_debug/sidecar/node_modules/svelte/package.json': JSON.stringify({
			version: '5.0.0'
		}),
		...fresh_stamps(),
		[`.pgo/bench/${FFI}`]: '',
		[`.pgo/bench/${NAPI}`]: '',
		'crates/tsv_wasm/pkg/all/deno': null,
		'crates/tsv_wasm/pkg/all/nodejs': null,
		'crates/tsv_wasm/pkg/all/npm': null
	},
	target: { [`corpus/${NAPI}`]: '', [`napi/${NAPI}`]: '' },
	siblings: {
		svelte: {
			'packages/svelte/tests': null,
			'packages/svelte/package.json': JSON.stringify({ version: '5.0.0' })
		},
		'acorn-typescript': { test: null, 'package.json': JSON.stringify({ version: '1.0.0' }) },
		typescript: { 'tests/baselines/reference': null },
		prettier: { 'package.json': JSON.stringify({ version: '3.0.0-dev' }) },
		'prettier-plugin-svelte': { 'package.json': JSON.stringify({ version: '3.0.0' }) },
		test262: { test: null },
		wpt: { css: null },
		'typescript-go': {
			'testdata/baselines/reference/submodule': null,
			'_submodules/TypeScript/tests/cases': null,
			'internal/bundled/libs': null
		},
		corpora: { 'collections/a/a.ts': '' }
	},
	sibling_dirs: {},
	programs: {
		deno: { '--version': exited('deno 2.9.7 (stable, release, x86_64-unknown-linux-gnu)\nv8 1\n') },
		node: {
			'--version': exited('v24.14.1\n'),
			'scripts/roots/check_canonical_pins.ts': exited('canonical pins OK — all agree\n')
		},
		bun: { '--version': exited('1.4.4\nrevision 1\n') },
		cargo: { '--version': exited('cargo 1.94.1 (29ea6fb6a 2026-03-24)\n') },
		wasm_pack: { '--version': exited('wasm-pack 0.15.0\n') },
		npm: { '--version': exited('11.20.0\n') },
		git: {
			...Object.fromEntries(
				Object.entries(HEADS).map(([repo, head]) => [
					`-C ${repo} rev-parse HEAD`,
					exited(`${head}\n`)
				])
			),
			'-C ../corpora rev-parse HEAD:collections': exited(`${CORPORA_HASH}\n`),
			'--no-optional-locks -C ../corpora status --porcelain -- collections': exited('')
		}
	},
	probe_node_modules: () => Promise.resolve({ status: 'ok' }),
	load_all_versions: () =>
		Promise.resolve({
			canonical: {
				svelte: '5.0.0',
				'@sveltejs/acorn-typescript': '1.0.0',
				prettier: '3.0.0',
				'prettier-plugin-svelte': '1.0.0',
				acorn: '8.0.0'
			}
		} as AllVersions),
	load_corpus: () =>
		Promise.resolve({
			corpus_missing_entries: () =>
				Promise.resolve({ missing: [], optional_missing: [], total: 3 }),
			corpus_unplaced_collections: () => Promise.resolve([])
		} as DoctorCorpus)
});

interface Run {
	code: number;
	lines: string[];
	programs: Record<keyof DoctorPrograms, FakeProgram | null>;
}

let out: string[];

beforeEach(() => {
	out = [];
	vi.spyOn(console, 'log').mockImplementation((...a: unknown[]) => void out.push(a.join(' ')));
});

afterEach(() => {
	vi.restoreAllMocks();
});

const run = async (world: World, argv: string[] = []): Promise<Run> => {
	out = [];
	const programs = Object.fromEntries(
		Object.entries(world.programs).map(([name, answers]) => [
			name,
			answers === null ? null : fake_program(answers)
		])
	) as Record<keyof DoctorPrograms, FakeProgram | null>;
	const code = await run_doctor(
		{
			checkout: fake_dir(world.checkout),
			target: world.target === null ? null : fake_dir(world.target),
			siblings: Object.fromEntries(
				DOCTOR_SIBLINGS.map((name) => {
					const files = world.siblings[name];
					return [name, world.sibling_dirs[name] ?? (files === null ? null : fake_dir(files))];
				})
			) as Record<DoctorSibling, DoctorDir | null>,
			programs,
			probe_node_modules: world.probe_node_modules,
			load_all_versions: world.load_all_versions,
			load_corpus: world.load_corpus
		},
		argv
	);
	return { code, lines: [...out], programs };
};

/** The printed lines that start with `mark` (`✓`, `⚠`, `✗`, `·`), indented as a check's
 * are, each without it; a line is one `console.log`, so a check's continuation lines are in it. */
const marked = (lines: string[], mark: string): string[] =>
	lines.filter((l) => l.startsWith(`  ${mark} `)).map((l) => l.slice(4));

/** The verdict, the last line. */
const verdict = (r: Run): string => r.lines.at(-1)!;

describe('a provisioned machine', () => {
	test('passes clean, every check a ✓ or an info line', async () => {
		const r = await run(healthy());
		strictEqual(r.code, 0);
		deepStrictEqual(marked(r.lines, '⚠'), []);
		deepStrictEqual(marked(r.lines, '✗'), []);
		strictEqual(verdict(r), '✓ doctor: everything present and consistent.');
		deepStrictEqual(marked(r.lines, '✓').slice(0, 8), [
			'deno 2.9.7',
			'node v24.14.1 (≥ 22.18)',
			'node v24.14.1 meets engines.node >=24.14 (package.json + crates/tsv_debug/sidecar/package.json)',
			'bun 1.4.4',
			'cargo 1.94.1 (29ea6fb6a 2026-03-24)',
			'wasm-pack 0.15.0 (matches pin)',
			'npm 11.20.0',
			'canonical pins OK — all agree'
		]);
		deepStrictEqual(marked(r.lines, '·'), [
			'freshness (artifact vs source mtimes) is enforced at run time by lib/check_artifact_freshness.ts (bench/corpus) and scripts/check_staged_freshness.ts (the staged npm/napi packages)',
			'the Node that answers: `cargo run -p tsv_debug check`'
		]);
	});

	test('the header names the mode', async () => {
		strictEqual(
			(await run(healthy())).lines[0],
			'tsv doctor — diagnostic-toolchain setup check (read-only)'
		);
		strictEqual(
			(await run(healthy(), ['--strict'])).lines[0],
			'tsv doctor — diagnostic-toolchain setup check (read-only, --strict: warnings fail)'
		);
	});

	test('runs each tool for its version alone, node for the pin audit, git for reads', async () => {
		const r = await run(healthy());
		for (const name of ['deno', 'bun', 'cargo', 'wasm_pack', 'npm'] as const) {
			deepStrictEqual(r.programs[name]!.calls, [['--version']], name);
		}
		deepStrictEqual(r.programs.node!.calls, [
			['--version'],
			['scripts/roots/check_canonical_pins.ts']
		]);
		for (const call of r.programs.git!.calls) {
			ok(
				(call[2] === 'rev-parse' && call.length === 4) ||
					call.join(' ') === '--no-optional-locks -C ../corpora status --porcelain -- collections',
				call.join(' ')
			);
		}
	});
});

/** A case: what it changes, the line it must print under its mark, and the exit code. */
interface Case {
	name: string;
	change: (world: World) => void;
	mark: '✓' | '⚠' | '✗' | '·';
	line: string;
	code: 0 | 1;
}

/** `answers` for a `--version` probe printing `stdout`. */
const version = (stdout: string): Answers => ({ '--version': exited(stdout) });

const CASES: Case[] = [
	// deno
	{
		name: 'deno absent',
		change: (w) => void (w.programs.deno = null),
		mark: '⚠',
		line: DENO_MISSING,
		code: 0
	},
	{
		name: 'deno exiting non-zero',
		change: (w) => void (w.programs.deno = { '--version': exited('deno 2.9.7\n', 1) }),
		mark: '⚠',
		line: DENO_MISSING,
		code: 0
	},
	{
		name: 'deno failing to start',
		change: (w) => void (w.programs.deno = { '--version': new Error('launch_failed') }),
		mark: '⚠',
		line: DENO_MISSING,
		code: 0
	},
	{
		name: 'deno printing garbage',
		change: (w) => void (w.programs.deno = version('hello\n')),
		mark: '⚠',
		line: 'deno — `deno --version` printed "hello", no Deno version',
		code: 0
	},
	{
		name: 'deno at any version, which no workflow installs',
		change: (w) => void (w.programs.deno = version('deno 3.0.0 (stable)\n')),
		mark: '✓',
		line: 'deno 3.0.0',
		code: 0
	},
	// node
	{
		name: 'node absent',
		change: (w) => void (w.programs.node = null),
		mark: '✗',
		line: "node missing — cargo test's fixture oracle (tsv_debug's sidecar) runs on it, and bench:node, bench:install, test:npm, and the publish artifact tests need it",
		code: 1
	},
	{
		name: 'node absent, for the pin audit',
		change: (w) => void (w.programs.node = null),
		mark: '✗',
		line: 'node missing — the canonical pin audit (scripts/roots/check_canonical_pins.ts) runs on it',
		code: 1
	},
	{
		name: 'node below the type-stripping floor',
		change: (w) => {
			w.programs.node!['--version'] = exited('v22.17.0\n');
		},
		mark: '⚠',
		line: 'node v22.17.0 — < 22.18 lacks native TS type-stripping; the harness entries fail to parse',
		code: 1
	},
	{
		name: "node below the bench's own floor, read from its manifest",
		change: (w) => {
			w.programs.node!['--version'] = exited('v22.18.0\n');
			w.checkout['benches/js/package.json'] = JSON.stringify({ engines: { node: '>=22.19' } });
		},
		mark: '⚠',
		line: 'node v22.18.0 — < 22.19 lacks native TS type-stripping; the harness entries fail to parse',
		code: 1
	},
	{
		name: "node below the bench's floor by its patch",
		change: (w) => {
			w.programs.node!['--version'] = exited('v22.18.1\n');
			w.checkout['benches/js/package.json'] = JSON.stringify({ engines: { node: '>=22.18.3' } });
		},
		mark: '⚠',
		line: 'node v22.18.1 — < 22.18.3 lacks native TS type-stripping; the harness entries fail to parse',
		code: 1
	},
	{
		name: "the bench's floor unreadable",
		change: (w) => void (w.checkout['benches/js/package.json'] = null),
		mark: '⚠',
		line: "cannot read engines.node from benches/js/package.json (not_found: benches/js/package.json) — the bench's Node floor is ungraded",
		code: 0
	},
	{
		name: "the bench's floor of another form",
		change: (w) =>
			void (w.checkout['benches/js/package.json'] = JSON.stringify({ engines: { node: '^22' } })),
		mark: '⚠',
		line: "benches/js/package.json states no engines.node floor of the form >=major.minor[.patch] — the bench's Node floor is ungraded",
		code: 0
	},
	{
		name: 'node at the type-stripping floor',
		change: (w) => {
			w.programs.node!['--version'] = exited('v22.18.0\n');
		},
		mark: '✓',
		line: 'node v22.18.0 (≥ 22.18)',
		code: 1
	},
	{
		name: 'deno printing its version past the line start',
		change: (w) => {
			w.programs.deno!['--version'] = exited('wrapper: deno 2.9.7 (stable)\n');
		},
		mark: '⚠',
		line: 'deno — `deno --version` printed "wrapper: deno 2.9.7 (stable)", no Deno version',
		code: 0
	},
	{
		name: 'node printing garbage',
		change: (w) => {
			w.programs.node!['--version'] = exited('hello\n');
		},
		mark: '⚠',
		line: 'node hello — < 22.18 lacks native TS type-stripping; the harness entries fail to parse',
		code: 0
	},
	{
		name: 'node at a version CI does not pin',
		change: (w) => {
			w.programs.node!['--version'] = exited('v26.13.0\n');
		},
		mark: '⚠',
		line: 'node 26.13.0 — differs from the 24.14.1 that check.yml + release.yml install; bump the workflows (node-version) so CI verifies what you develop against',
		code: 0
	},
	{
		name: 'node at a version that only prefixes the pin',
		change: (w) => {
			w.programs.node!['--version'] = exited('v24.14.10\n');
		},
		mark: '⚠',
		line: 'node 24.14.10 — differs from the 24.14.1 that check.yml + release.yml install; bump the workflows (node-version) so CI verifies what you develop against',
		code: 0
	},
	{
		name: 'workflows pinning two node versions',
		change: (w) =>
			void (w.checkout['.github/workflows/release.yml'] += "    node-version: '24.13.0'\n"),
		mark: '⚠',
		line: 'node — the workflows pin more than one version (check.yml: 24.14.1, release.yml: 24.14.1, release.yml: 24.13.0); CI disagrees with itself about which node verifies this repo',
		code: 0
	},
	{
		name: 'one workflow pinning two node versions',
		change: (w) =>
			void (w.checkout['.github/workflows/check.yml'] += "  c:\n    node-version: '24.13.0'\n"),
		mark: '⚠',
		line: 'node — the workflows pin more than one version (check.yml: 24.14.1, check.yml: 24.13.0, release.yml: 24.14.1); CI disagrees with itself about which node verifies this repo',
		code: 0
	},
	{
		name: 'node below the engines floor',
		change: (w) => {
			w.programs.node!['--version'] = exited('v24.13.9\n');
		},
		mark: '✗',
		line: 'node v24.13.9 — below engines.node >=24.14 (package.json + crates/tsv_debug/sidecar/package.json): the npm scripts on gro, this one among them; the fixture oracle `cargo test` runs, which tsv_debug refuses to start below it',
		code: 1
	},
	{
		name: 'node meeting one floor and not the other',
		change: (w) => {
			w.programs.node!['--version'] = exited('v24.20.0\n');
			w.checkout['crates/tsv_debug/sidecar/package.json'] = JSON.stringify({
				engines: { node: '>=25.0' }
			});
		},
		mark: '✗',
		line: 'node v24.20.0 — below engines.node >=25.0 (crates/tsv_debug/sidecar/package.json): the fixture oracle `cargo test` runs, which tsv_debug refuses to start below it',
		code: 1
	},
	{
		name: 'node meeting a floor with a patch',
		change: (w) => {
			w.checkout['package.json'] = JSON.stringify({ engines: { node: '>=24.14.1' } });
		},
		mark: '✓',
		line: 'node v24.14.1 meets engines.node >=24.14.1 (package.json)',
		code: 0
	},
	{
		name: 'node below a floor by its patch',
		change: (w) => {
			w.checkout['package.json'] = JSON.stringify({ engines: { node: '>=24.14.2' } });
		},
		mark: '✗',
		line: 'node v24.14.1 — below engines.node >=24.14.2 (package.json): the npm scripts on gro, this one among them',
		code: 1
	},
	{
		name: 'an unreadable engines floor',
		change: (w) => void delete w.checkout['package.json'],
		mark: '✗',
		line: 'cannot read engines.node from package.json (not_found: package.json) — the Node floor it states is ungraded',
		code: 1
	},
	{
		name: 'an unreadable engines floor, the other still graded',
		change: (w) => void delete w.checkout['package.json'],
		mark: '✓',
		line: 'node v24.14.1 meets engines.node >=24.14 (crates/tsv_debug/sidecar/package.json)',
		code: 1
	},
	{
		name: 'an engines floor of another form',
		change: (w) => {
			w.checkout['crates/tsv_debug/sidecar/package.json'] = JSON.stringify({
				engines: { node: '^24' }
			});
		},
		mark: '✗',
		line: 'crates/tsv_debug/sidecar/package.json states no engines.node floor of the form >=major.minor[.patch] — the Node floor it states is ungraded',
		code: 1
	},
	// bun, cargo, wasm-pack, npm
	{
		name: 'bun absent',
		change: (w) => void (w.programs.bun = null),
		mark: '⚠',
		line: "bun missing — bench:bun unavailable, test:bun and the package suites' Bun legs warn-skip, and a publish --wetrun fails without it (--no-check waives)",
		code: 0
	},
	{
		name: 'bun exiting non-zero',
		change: (w) => void (w.programs.bun = { '--version': exited('1.4.4\n', 2) }),
		mark: '⚠',
		line: "bun missing — bench:bun unavailable, test:bun and the package suites' Bun legs warn-skip, and a publish --wetrun fails without it (--no-check waives)",
		code: 0
	},
	{
		name: 'bun killed',
		change: (w) =>
			void (w.programs.bun = {
				'--version': { kind: 'signaled', stdout: '1.4.4\n', stderr: '' }
			}),
		mark: '⚠',
		line: "bun missing — bench:bun unavailable, test:bun and the package suites' Bun legs warn-skip, and a publish --wetrun fails without it (--no-check waives)",
		code: 0
	},
	{
		name: 'cargo absent',
		change: (w) => void (w.programs.cargo = null),
		mark: '✗',
		line: 'cargo missing — nothing Rust builds without it',
		code: 1
	},
	{
		name: 'cargo exiting non-zero',
		change: (w) => void (w.programs.cargo = { '--version': exited('', 1, 'no toolchain') }),
		mark: '✗',
		line: 'cargo missing — nothing Rust builds without it',
		code: 1
	},
	{
		name: 'wasm-pack absent',
		change: (w) => void (w.programs.wasm_pack = null),
		mark: '⚠',
		line: 'wasm-pack missing — WASM builds (build:wasm:*, bench, publish) unavailable: cargo install wasm-pack',
		code: 0
	},
	{
		name: 'wasm-pack at another version',
		change: (w) => void (w.programs.wasm_pack = version('wasm-pack 0.14.0\n')),
		mark: '⚠',
		line: 'wasm-pack 0.14.0 — differs from the pinned 0.15.0 (check.yml + the validate_artifacts.ts size-bound calibration); sizes may drift into failing bounds at publish',
		code: 0
	},
	{
		name: 'npm absent',
		change: (w) => void (w.programs.npm = null),
		mark: '✗',
		line: 'npm missing — `npm run` runs the package.json scripts (`check` among them) and bench:install',
		code: 1
	},
	// canonical pins
	{
		name: 'the pin audit failing',
		change: (w) => {
			w.programs.node!['scripts/roots/check_canonical_pins.ts'] = exited(
				'ignored\n',
				1,
				'pin drift\nsvelte differs\n'
			);
		},
		mark: '✗',
		line: 'pin drift\n    svelte differs',
		code: 1
	},
	{
		name: 'the pin audit failing to start',
		change: (w) => {
			w.programs.node!['scripts/roots/check_canonical_pins.ts'] = new Error('launch_failed\nmore');
		},
		mark: '✗',
		line: 'cannot run the canonical pin audit (launch_failed)',
		code: 1
	},
	{
		name: 'the pin audit signaled',
		change: (w) => {
			w.programs.node!['scripts/roots/check_canonical_pins.ts'] = {
				kind: 'signaled',
				stdout: 'canonical pins OK — all agree\n',
				stderr: 'killed\n'
			};
		},
		mark: '✗',
		line: 'killed',
		code: 1
	},
	// harness deps
	{
		name: 'no bench install',
		change: (w) =>
			void (w.probe_node_modules = () =>
				Promise.resolve({ status: 'missing', message: 'not installed' })),
		mark: '⚠',
		line: 'not installed',
		code: 0
	},
	{
		name: 'a stale bench install',
		change: (w) =>
			void (w.probe_node_modules = () => Promise.resolve({ status: 'stale', message: 'stale' })),
		mark: '✗',
		line: 'stale — reports would label OLD installed versions with the new pins',
		code: 1
	},
	{
		name: 'an unreadable bench package.json',
		change: (w) => void (w.probe_node_modules = () => Promise.reject(new Error('bad json\nmore'))),
		mark: '✗',
		line: 'cannot read benches/js/package.json (bad json) — the pins are unreadable, so nothing downstream can be graded against them',
		code: 1
	},
	// oracle checkouts
	{
		name: 'unreadable oracle pins',
		change: (w) => void (w.load_all_versions = () => Promise.reject(new Error('bad pins'))),
		mark: '✗',
		line: 'cannot read the oracle pins (bad pins) — checkout-vs-pin skew is ungraded below',
		code: 1
	},
	{
		name: 'unreadable oracle pins, the checkouts listed ungraded',
		change: (w) => void (w.load_all_versions = () => Promise.reject(new Error('bad pins'))),
		mark: '✓',
		line: '../svelte checkout (v5.0.0; pin unreadable)',
		code: 1
	},
	{
		name: '../svelte absent',
		change: (w) => void (w.siblings.svelte = null),
		mark: '⚠',
		line: '../svelte checkout missing — conformance:svelte-fixtures + the corpus suites need it',
		code: 0
	},
	{
		name: '../svelte without its tests',
		change: (w) => void delete w.siblings.svelte!['packages/svelte/tests'],
		mark: '⚠',
		line: '../svelte checkout missing — conformance:svelte-fixtures + the corpus suites need it',
		code: 0
	},
	{
		name: '../svelte skewed from its pin',
		change: (w) => {
			w.siblings.svelte!['packages/svelte/package.json'] = JSON.stringify({ version: '5.1.0' });
		},
		mark: '⚠',
		line: '../svelte checkout is v5.1.0 but the svelte oracle is pinned v5.0.0 — suite inputs and the grading parser disagree (align the checkout or bump the pins deliberately)',
		code: 0
	},
	{
		name: '../svelte with no version file, the broken checkout the pin audit fails',
		change: (w) => void delete w.siblings.svelte!['packages/svelte/package.json'],
		mark: '⚠',
		line: "../svelte is present but packages/svelte/package.json can't be read (not_found: packages/svelte/package.json) — a broken checkout, not a machine without one; repair it, or remove it",
		code: 0
	},
	{
		name: '../svelte stating no version',
		change: (w) => void (w.siblings.svelte!['packages/svelte/package.json'] = '{}'),
		mark: '✓',
		line: '../svelte checkout (v?, matches the pin)',
		code: 0
	},
	{
		name: '../acorn-typescript absent',
		change: (w) => void (w.siblings['acorn-typescript'] = null),
		mark: '⚠',
		line: '../acorn-typescript checkout missing — conformance:ts-fixtures needs it',
		code: 0
	},
	{
		name: '../acorn-typescript skewed',
		change: (w) => {
			w.siblings['acorn-typescript']!['package.json'] = JSON.stringify({ version: '1.1.0' });
		},
		mark: '⚠',
		line: '../acorn-typescript checkout is v1.1.0 but the oracle is pinned v1.0.0 — suite inputs and the grading parser disagree',
		code: 0
	},
	{
		name: '../typescript partial',
		change: (w) => void (w.siblings.typescript = { README: '' }),
		mark: '⚠',
		line: '../typescript present but tests/baselines/reference missing — conformance:ts-repo will FAIL (partial checkout)',
		code: 0
	},
	{
		name: '../typescript absent',
		change: (w) => void (w.siblings.typescript = null),
		mark: '⚠',
		line: '../typescript checkout missing — conformance:ts-repo + bench:harvest:ts-repo need it',
		code: 0
	},
	{
		name: '../prettier absent',
		change: (w) => void (w.siblings.prettier = null),
		mark: '⚠',
		line: '../prettier checkout missing — the corpus prettier suites + layout-reference reading need it',
		code: 0
	},
	{
		name: '../prettier skewed past its -dev suffix',
		change: (w) => {
			w.siblings.prettier!['package.json'] = JSON.stringify({ version: '3.1.0-dev' });
		},
		mark: '⚠',
		line: '../prettier checkout is v3.1.0-dev vs pinned prettier v3.0.0 — its fixture suites (corpus inputs) come from a different version than the live oracle (informational)',
		code: 0
	},
	{
		name: '../prettier at its pin',
		change: () => {},
		mark: '✓',
		line: '../prettier checkout (v3.0.0-dev)',
		code: 0
	},
	{
		name: '../test262 absent',
		change: (w) => void (w.siblings.test262 = null),
		mark: '⚠',
		line: '../test262 checkout missing — conformance:test262 (the release gate, publish Step 3b) + bench:harvest:test262 need it',
		code: 0
	},
	{
		name: '../wpt without css',
		change: (w) => void (w.siblings.wpt = { README: '' }),
		mark: '⚠',
		line: '../wpt/css checkout missing — bench:harvest:wpt unavailable (manual-cadence tool)',
		code: 0
	},
	{
		name: 'a sibling whose version read fails, warned as what it is',
		change: (w) => {
			const dir = fake_dir(w.siblings.prettier!);
			w.sibling_dirs.prettier = { ...dir, read_text: failing_dir.read_text };
		},
		mark: '⚠',
		line: "../prettier is present but package.json can't be read (permission_denied: x) — a broken checkout, not a machine without one; repair it, or remove it",
		code: 0
	},
	{
		name: 'a sibling stating no version, its version unknown',
		change: (w) => void (w.siblings.prettier!['package.json'] = '{}'),
		mark: '✓',
		line: '../prettier checkout (v?)',
		code: 0
	},
	{
		name: 'a workflows directory whose listing fails, graded against nothing',
		change: (w) => {
			w.programs.deno = version('deno 3.0.0\n');
			w.checkout['.github/workflows'] = 'not a directory';
			for (const path of Object.keys(w.checkout)) {
				if (path.startsWith('.github/workflows/')) delete w.checkout[path];
			}
		},
		mark: '✓',
		line: 'deno 3.0.0',
		code: 0
	},
	// the corpus snapshot
	{
		name: '../corpora absent',
		change: (w) => {
			w.siblings.corpora = null;
			w.programs.git!['-C ../corpora rev-parse HEAD:collections'] = exited('', 128);
		},
		mark: '⚠',
		line: '../corpora checkout missing — the bench + corpus:compare gates read the real-code snapshot (git clone https://github.com/fuzdev/corpora ../corpora)',
		code: 0
	},
	{
		name: '../corpora present, git failing in it',
		change: (w) => {
			w.programs.git!['-C ../corpora rev-parse HEAD:collections'] = exited('', 128);
		},
		mark: '⚠',
		line:
			"../corpora is present but git can't read its collections/ tree id (not a git checkout, one git refuses, a corrupt or unreadable one, or the snapshot gone) — the snapshot is ungraded; " +
			"make it a clone git can read (an export has no history; a repository git refuses for its owner is made yours, since listing it in git's safe.directory instead trusts its config to run commands as you; a corrupt one is cloned again), or remove it to skip what reads it",
		code: 0
	},
	{
		name: '../corpora at another tree',
		change: (w) => {
			w.programs.git!['-C ../corpora rev-parse HEAD:collections'] = exited(`${'9'.repeat(40)}\n`);
		},
		mark: '⚠',
		line: `../corpora:collections is at 999999999 but the corpus pins were measured at ${GATE_CHECKOUT_IDS['../corpora']!.hash} — a snapshot refresh re-runs the corpus gates and re-pins; otherwise check out the pinned snapshot`,
		code: 0
	},
	{
		name: '../corpora modified',
		change: (w) => {
			w.programs.git!['--no-optional-locks -C ../corpora status --porcelain -- collections'] =
				exited(' M x\n');
		},
		mark: '⚠',
		line: `../corpora is at pinned ${GATE_CHECKOUT_IDS['../corpora']!.hash} but collections/ has local modifications — the corpus gates would grade bytes that commit does not hold (git -C ../corpora checkout -- collections)`,
		code: 0
	},
	{
		name: '../corpora whose status fails',
		change: (w) => {
			w.programs.git!['--no-optional-locks -C ../corpora status --porcelain -- collections'] =
				exited(' M x\n', 128);
		},
		mark: '✓',
		line: `../corpora snapshot (collections/ at pinned ${GATE_CHECKOUT_IDS['../corpora']!.hash}, clean)`,
		code: 0
	},
	{
		name: 'git absent',
		change: (w) => void (w.programs.git = null),
		mark: '⚠',
		line: '../corpora is present, but no git is on PATH to read its collections/ tree id — the snapshot is ungraded',
		code: 0
	},
	{
		name: 'git absent, a stamp graded against nothing',
		change: (w) => void (w.programs.git = null),
		mark: '·',
		line: 'test262: no checkout to grade against (../test262 present, no git on PATH to read it)',
		code: 0
	},
	// the experimental typechecker
	{
		name: '../typescript-go absent',
		change: (w) => void (w.siblings['typescript-go'] = null),
		mark: '·',
		line: '../typescript-go checkout absent — needed only by the on-demand tsc_conformance tasks',
		code: 0
	},
	{
		name: '../typescript-go partial',
		change: (w) =>
			void delete w.siblings['typescript-go']!['testdata/baselines/reference/submodule'],
		mark: '⚠',
		line: '../typescript-go present but testdata/baselines/reference/submodule missing — conformance:tsc-roundtrip would FAIL (partial checkout)',
		code: 0
	},
	{
		name: '../typescript-go without its corpus inputs',
		change: (w) => void delete w.siblings['typescript-go']!['_submodules/TypeScript/tests/cases'],
		mark: '·',
		line: '../typescript-go corpus inputs absent — only conformance:tsc-check needs them (git submodule update --init in ../typescript-go)',
		code: 0
	},
	{
		name: '../typescript-go without its bundled libs',
		change: (w) => void delete w.siblings['typescript-go']!['internal/bundled/libs'],
		mark: '·',
		line: '../typescript-go bundled libs (internal/bundled/libs) absent — only conformance:tsc-check needs them',
		code: 0
	},
	// harvest stamps
	{
		name: 'a stamp never written',
		change: (w) => void delete w.checkout[HARVEST_STAMPS.test262.path],
		mark: '⚠',
		line: 'test262: never stamped (no benches/js/.cache/test262.stamp.json) — run npm run bench:harvest:test262',
		code: 0
	},
	{
		name: 'a stamp behind its checkout',
		change: (w) => {
			w.checkout[HARVEST_STAMPS.test262.path] = JSON.stringify({ source_commit: '2'.repeat(40) });
		},
		mark: '⚠',
		line: 'test262: stamp is stale for ../test262 (stamped 222222222, now bbbbbbbbb) — run npm run bench:harvest:test262',
		code: 0
	},
	{
		name: 'a stamp missing an input',
		change: (w) => void (w.checkout[HARVEST_STAMPS.test262.path] = '{}'),
		mark: '⚠',
		line: 'test262: stamp is stale for ../test262 (no `source_commit` recorded — the stamp predates that input) — run npm run bench:harvest:test262',
		code: 0
	},
	{
		name: 'a stamp with one of its checkouts absent',
		change: (w) => {
			w.siblings['prettier-plugin-svelte'] = null;
			w.programs.git!['-C ../prettier-plugin-svelte rev-parse HEAD'] = exited('', 128);
		},
		mark: '✓',
		line: 'svelte-rejects: stamp matches ../svelte + ../prettier (../prettier-plugin-svelte not checked out)',
		code: 0
	},
	{
		name: 'a stamp with none of its checkouts',
		change: (w) => {
			w.siblings.test262 = null;
			w.programs.git!['-C ../test262 rev-parse HEAD'] = exited('', 128);
		},
		mark: '·',
		line: 'test262: no checkout to grade against (../test262 not checked out)',
		code: 0
	},
	{
		name: 'a stamp with one of its checkouts present but unread by git',
		change: (w) => {
			w.programs.git!['-C ../prettier-plugin-svelte rev-parse HEAD'] = exited('', 128);
		},
		mark: '✓',
		line: "svelte-rejects: stamp matches ../svelte + ../prettier (../prettier-plugin-svelte present, git can't read it)",
		code: 0
	},
	{
		name: 'a stamp whose directory beneath a present checkout is missing',
		change: (w) => {
			w.siblings.wpt = { README: '' };
			w.programs.git!['-C ../wpt/css rev-parse HEAD'] = exited('', 128);
		},
		mark: '·',
		line: 'wpt-css: no checkout to grade against (../wpt/css not checked out)',
		code: 0
	},
	// corpus entries
	{
		name: 'corpus entries missing',
		change: (w) =>
			void (w.load_corpus = () =>
				Promise.resolve({
					corpus_missing_entries: (view: string) =>
						Promise.resolve(
							view === 'gates'
								? { missing: ['a', 'b'], optional_missing: [], total: 3 }
								: { missing: [], optional_missing: [], total: 3 }
						),
					corpus_unplaced_collections: () => Promise.resolve([])
				} as DoctorCorpus)),
		mark: '⚠',
		line: 'gates view: 2/3 entries missing — corpus:compare/bench on this view fail fast:\n      a\n      b',
		code: 0
	},
	{
		name: 'an optional corpus entry absent',
		change: (w) =>
			void (w.load_corpus = () =>
				Promise.resolve({
					corpus_missing_entries: () =>
						Promise.resolve({ missing: [], optional_missing: ['o'], total: 3 }),
					corpus_unplaced_collections: () => Promise.resolve(['x', 'y'])
				} as DoctorCorpus)),
		mark: '✓',
		line: 'gates view: all 2/3 required entries present',
		code: 0
	},
	{
		name: 'an optional corpus entry absent, disclosed',
		change: (w) =>
			void (w.load_corpus = () =>
				Promise.resolve({
					corpus_missing_entries: () =>
						Promise.resolve({ missing: [], optional_missing: ['o'], total: 3 }),
					corpus_unplaced_collections: () => Promise.resolve([])
				} as DoctorCorpus)),
		mark: '·',
		line: 'conformance view: optional entry absent (fail-open, disclosed): o',
		code: 0
	},
	{
		name: 'unplaced snapshot collections',
		change: (w) =>
			void (w.load_corpus = () =>
				Promise.resolve({
					corpus_missing_entries: () =>
						Promise.resolve({ missing: [], optional_missing: [], total: 3 }),
					corpus_unplaced_collections: () => Promise.resolve(['x', 'y'])
				} as DoctorCorpus)),
		mark: '·',
		line: '2 snapshot collection(s) not placed (whole-snapshot sweeps only): x, y',
		code: 0
	},
	{
		name: 'the corpus loader failing',
		change: (w) => void (w.load_corpus = () => Promise.reject(new Error('no manifest\nmore'))),
		mark: '⚠',
		line: 'cannot load the corpus entry list: no manifest',
		code: 0
	},
	// build artifacts
	{
		name: 'an artifact absent',
		change: (w) => void delete w.checkout[`.pgo/bench/${FFI}`],
		mark: '·',
		line: `.pgo/bench/${FFI} absent — npm run build:bench:pgo (bench:deno, profile-guided)`,
		code: 0
	},
	{
		name: 'no target directory',
		change: (w) => void (w.target = null),
		mark: '·',
		line: `target/napi/${NAPI} absent — npm run build:napi (test:napi)`,
		code: 0
	},
	{
		name: "the conformance gates' addon absent",
		change: (w) => void delete w.target![`corpus/${NAPI}`],
		mark: '·',
		line: `target/corpus/${NAPI} absent — npm run build:napi:corpus (conformance gates, over-acceptance)`,
		code: 0
	},
	{
		name: "the conformance gates' addon present",
		change: () => {},
		mark: '✓',
		line: `target/corpus/${NAPI}`,
		code: 0
	},
	// the sidecar
	{
		name: 'the sidecar not installed',
		change: (w) =>
			void delete w.checkout['crates/tsv_debug/sidecar/node_modules/.package-lock.json'],
		mark: '✗',
		line: 'crates/tsv_debug/sidecar is not installed — cargo test and the fixture tooling refuse to run it; run `npm ci --prefix crates/tsv_debug/sidecar`',
		code: 1
	},
	{
		name: 'the sidecar installed from another lock',
		change: (w) => {
			w.checkout['crates/tsv_debug/sidecar/node_modules/svelte/package.json'] = JSON.stringify({
				version: '5.0.1'
			});
		},
		mark: '✗',
		line: 'crates/tsv_debug/sidecar differs from its lock — cargo test and the fixture tooling refuse to run it; run `npm ci --prefix crates/tsv_debug/sidecar`',
		code: 1
	},
	{
		name: 'the sidecar lock absent',
		change: (w) => void delete w.checkout['crates/tsv_debug/sidecar/package-lock.json'],
		mark: '✗',
		line: "cannot check crates/tsv_debug/sidecar's install (not_found: crates/tsv_debug/sidecar/package-lock.json)",
		code: 1
	}
];

describe('each check', () => {
	for (const c of CASES) {
		test(c.name, async () => {
			const world = healthy();
			c.change(world);
			const r = await run(world);
			ok(marked(r.lines, c.mark).includes(c.line), `no ${c.mark} ${c.line}\n${r.lines.join('\n')}`);
			strictEqual(r.code, c.code, verdict(r));
		});
	}
});

describe('a present sibling that cannot say what it holds', () => {
	test('a read that fails warns as what it is, never as a missing checkout', async () => {
		const world = healthy();
		world.sibling_dirs.test262 = failing_dir;
		const r = await run(world);
		strictEqual(r.code, 0, verdict(r));
		deepStrictEqual(marked(r.lines, '⚠'), [
			"../test262 is present but whether test is in it can't be read (permission_denied: x) — a broken checkout, not a machine without one; repair it, or remove it"
		]);
	});

	test('a path beneath it through a link out of it warns as no tool following it', async () => {
		const world = healthy();
		world.sibling_dirs['typescript-go'] = {
			...fake_dir(world.siblings['typescript-go']!),
			exists: (path) =>
				path === '_submodules/TypeScript/tests/cases'
					? Promise.reject(new FsError('outside_root', path))
					: Promise.resolve(true)
		};
		const r = await run(world);
		strictEqual(r.code, 0, verdict(r));
		deepStrictEqual(marked(r.lines, '⚠'), [
			'../typescript-go/_submodules/TypeScript/tests/cases reaches through a symlink that is absolute or climbs out of ../typescript-go, which is never followed; make it relative within the checkout, or link the checkout itself instead'
		]);
		deepStrictEqual(
			marked(r.lines, '·').filter((line) => line.includes('corpus inputs')),
			[]
		);
	});

	test('each sibling read that fails warns once per path asked, and nothing reads it as missing', async () => {
		for (const name of [
			'svelte',
			'acorn-typescript',
			'typescript',
			'wpt',
			'typescript-go'
		] as const) {
			const world = healthy();
			world.sibling_dirs[name] = failing_dir;
			const r = await run(world);
			strictEqual(r.code, 0, `${name}: ${verdict(r)}`);
			const warned = marked(r.lines, '⚠');
			ok(warned.length > 0, name);
			ok(
				warned.every((line) => line.startsWith(`../${name} is present but whether `)),
				`${name}:\n${warned.join('\n')}`
			);
			ok(
				!marked(r.lines, '·').some(
					(line) => line.includes(`../${name}`) && line.includes('absent')
				),
				name
			);
		}
	});

	test('a version file reached through a link out of it warns as no tool following it, never as matching the pin', async () => {
		const world = healthy();
		world.sibling_dirs.svelte = {
			...fake_dir(world.siblings.svelte!),
			read_text: (path) => Promise.reject(new FsError('outside_root', path))
		};
		const r = await run(world);
		strictEqual(r.code, 0, verdict(r));
		deepStrictEqual(marked(r.lines, '⚠'), [
			'../svelte/packages/svelte/package.json reaches through a symlink that is absolute or climbs out of ../svelte, which is never followed; make it relative within the checkout, or link the checkout itself instead'
		]);
		ok(!marked(r.lines, '✓').some((line) => line.startsWith('../svelte checkout')));
	});

	test('each version file that fails to read warns once, never beside a check of that checkout', async () => {
		for (const [name, path] of [
			['acorn-typescript', 'package.json'],
			['prettier', 'package.json']
		] as const) {
			const world = healthy();
			world.sibling_dirs[name] = {
				...fake_dir(world.siblings[name]!),
				read_text: (p) => Promise.reject(new FsError('permission_denied', p))
			};
			const r = await run(world);
			strictEqual(r.code, 0, `${name}: ${verdict(r)}`);
			deepStrictEqual(marked(r.lines, '⚠'), [
				`../${name} is present but ${path} can't be read (permission_denied: ${path}) — a broken checkout, not a machine without one; repair it, or remove it`
			]);
			ok(!marked(r.lines, '✓').some((line) => line.startsWith(`../${name} checkout`)), name);
		}
	});

	test('a stamp input beneath a present checkout reads as absent only when a file is in the way', async () => {
		for (const [kind, aside] of [
			['not_a_directory', '../wpt/css not checked out'],
			['permission_denied', "../wpt/css present, git can't read it"]
		] as const) {
			const world = healthy();
			world.sibling_dirs.wpt = {
				...fake_dir(world.siblings.wpt!),
				exists: (path) => Promise.reject(new FsError(kind, path))
			};
			world.programs.git!['-C ../wpt/css rev-parse HEAD'] = exited('', 128);
			const r = await run(world);
			ok(
				marked(r.lines, '·').includes(`wpt-css: no checkout to grade against (${aside})`),
				`${kind}:\n${r.lines.join('\n')}`
			);
		}
	});

	test('a path that passes through a file reads as missing', async () => {
		const world = healthy();
		world.sibling_dirs.svelte = {
			...fake_dir(world.siblings.svelte!),
			exists: (path) => Promise.reject(new FsError('not_a_directory', path))
		};
		const r = await run(world);
		deepStrictEqual(marked(r.lines, '⚠'), [
			'../svelte checkout missing — conformance:svelte-fixtures + the corpus suites need it'
		]);
	});
});

test('a missing deno fails no check leg, so it warns, and fails only under --strict', async () => {
	const world = healthy();
	world.programs.deno = null;
	const r = await run(world);
	strictEqual(r.code, 0, verdict(r));
	deepStrictEqual(marked(r.lines, '⚠'), [DENO_MISSING]);
	strictEqual((await run(world, ['--strict'])).code, 1);
});

describe('the CI pins', () => {
	test('a tool no workflow pins is graded against nothing, and warns nothing', async () => {
		const world = healthy();
		world.checkout['.github/workflows/check.yml'] = 'jobs:\n  a:\n';
		world.checkout['.github/workflows/release.yml'] = 'jobs:\n  a:\n';
		world.programs.node!['--version'] = exited('v26.13.0\n');
		const r = await run(world);
		deepStrictEqual(marked(r.lines, '⚠'), []);
		ok(
			marked(r.lines, '·').includes(
				'node 26.13.0 — no workflow pins a node-version, so nothing grades it against CI'
			)
		);
		strictEqual(r.code, 0);
		strictEqual(verdict(r), '✓ doctor: everything present and consistent.');
	});

	test('a deno-version pin grades nothing: no workflow installs Deno', async () => {
		const world = healthy();
		world.checkout['.github/workflows/check.yml'] += '  c:\n    deno-version: 2.9.7\n';
		world.programs.deno = version('deno 2.9.8\n');
		const r = await run(world);
		deepStrictEqual(marked(r.lines, '⚠'), []);
		ok(marked(r.lines, '✓').includes('deno 2.9.8'));
		ok(!r.lines.some((line) => line.includes('deno-version')), r.lines.join('\n'));
		strictEqual(verdict(r), '✓ doctor: everything present and consistent.');
	});

	test('no workflows directory grades nothing', async () => {
		const world = healthy();
		for (const path of Object.keys(world.checkout)) {
			if (path.startsWith('.github/')) delete world.checkout[path];
		}
		world.programs.node!['--version'] = exited('v99.0.0\n');
		const r = await run(world);
		deepStrictEqual(marked(r.lines, '⚠'), []);
		strictEqual(r.code, 0);
	});

	test('a workflow directory entry that is no file is not read', async () => {
		const world = healthy();
		world.checkout['.github/workflows/sub.yml/x.yml'] = "node-version: '1.0.0'\n";
		world.programs.node!['--version'] = exited('v26.13.0\n');
		const r = await run(world);
		deepStrictEqual(marked(r.lines, '⚠'), [
			'node 26.13.0 — differs from the 24.14.1 that check.yml + release.yml install; bump the workflows (node-version) so CI verifies what you develop against'
		]);
	});
});

describe('the verdict', () => {
	test('warnings alone pass, counted', async () => {
		const world = healthy();
		world.programs.bun = null;
		world.programs.wasm_pack = null;
		const r = await run(world);
		strictEqual(r.code, 0);
		strictEqual(
			verdict(r),
			'⚠ doctor: 2 warning(s) — nothing misleading; the ⚠ tools fail loud on their own if used.'
		);
	});

	test('--strict fails on a warning', async () => {
		const world = healthy();
		world.programs.bun = null;
		const r = await run(world, ['--strict']);
		strictEqual(r.code, 1);
		strictEqual(
			verdict(r),
			'✗ doctor --strict: 1 warning(s) — this machine is not fully provisioned.'
		);
	});

	test('--strict passes a clean machine, info lines included', async () => {
		const world = healthy();
		world.siblings['typescript-go'] = null;
		const r = await run(world, ['--strict']);
		strictEqual(r.code, 0);
		strictEqual(verdict(r), '✓ doctor: everything present and consistent.');
	});

	test('a failure fails, counting both', async () => {
		const world = healthy();
		world.programs.cargo = null;
		world.programs.bun = null;
		const r = await run(world);
		strictEqual(r.code, 1);
		strictEqual(
			verdict(r),
			'✗ doctor: 1 error(s), 1 warning(s) — the ✗ items above would mislead a run; fix them first.'
		);
	});

	test('every program absent is reported, never a crash', async () => {
		const world = healthy();
		for (const name of Object.keys(world.programs) as Array<keyof DoctorPrograms>) {
			world.programs[name] = null;
		}
		const r = await run(world);
		strictEqual(r.code, 1);
		strictEqual(
			verdict(r),
			'✗ doctor: 4 error(s), 4 warning(s) — the ✗ items above would mislead a run; fix them first.'
		);
	});
});
