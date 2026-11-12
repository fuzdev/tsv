/**
 * The deps `publish`'s entry builds (`create_publish_deps`), over a temp checkout and harmless
 * programs standing in for every one it finds (`sh`, never `npm`, `git`, or `deno`): each
 * handle's environment, its name's lookup, the existence probe beside the checkout, the
 * snapshot's paths, the pause, the print streams, the entry left as its one call, and the task
 * that starts it, which no npm lifecycle can.
 *
 * @module
 */

import { deepStrictEqual, ok, rejects, strictEqual } from 'node:assert';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import process from 'node:process';

import { fs } from '@fuzdev/gro/fs.ts';
import { is_fs_error } from '@fuzdev/gro/fs_error.ts';
import { GIT_LOCAL_PASS_ENV } from '@fuzdev/gro/git.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';
import { afterEach, beforeEach, describe, test } from 'vitest';

import { CORPORA_COLLECTIONS, CORPORA_ROOT } from '../../benches/js/lib/corpora.ts';
import { CARGO_PASS_ENV } from '../cargo_env.ts';
import {
	create_publish_deps,
	exists_beside,
	node_test_env,
	PUBLISH_PROGRAM_ENV,
	PUBLISH_PROGRAM_NAMES
} from './publish_deps.ts';

const CHECKOUT = new URL('../../', import.meta.url).pathname.replace(/\/$/, '');

/** Every program found as `sh`. */
const SH = Object.fromEntries(
	Object.keys(PUBLISH_PROGRAM_NAMES).map((key) => [key, 'sh'])
) as typeof PUBLISH_PROGRAM_NAMES;

/** Variables set in the host for the deps to see or drop, restored after each test. */
const PROBE = {
	TSV_PUBLISH_PROBE: 'host',
	GIT_PUBLISH_PROBE: 'git',
	CARGO_PUBLISH_PROBE: 'cargo',
	LC_ALL: 'en_US.UTF-8'
};
let saved: Record<string, string | undefined>;
beforeEach(() => {
	saved = Object.fromEntries(Object.keys(PROBE).map((name) => [name, process.env[name]]));
	Object.assign(process.env, PROBE);
});
afterEach(() => {
	for (const [name, value] of Object.entries(saved)) {
		if (value === undefined) delete process.env[name];
		else process.env[name] = value;
	}
});

/** What `program` sees: its working directory and the probed variables. */
const seen = async (program: SpawnProgram | null, cwd: Parameters<SpawnProgram['out']>[1]) => {
	ok(program !== null);
	const result = await program.out(
		[
			'-c',
			'pwd -P; for n in TSV_PUBLISH_PROBE GIT_PUBLISH_PROBE CARGO_PUBLISH_PROBE LC_ALL TSV_BUILD_FORCE PKG_DIR; do printf "%s=%s\\n" "$n" "$(printenv $n)"; done'
		],
		cwd
	);
	strictEqual(result.kind, 'exited');
	return result.stdout.trim().split('\n');
};

describe('the programs', () => {
	test('each is found by its name on PATH, and is null when it is not there', () => {
		deepStrictEqual(PUBLISH_PROGRAM_NAMES, {
			deno: 'deno',
			node: 'node',
			npm: 'npm',
			git: 'git',
			cargo: 'cargo',
			wasm_pack: 'wasm-pack',
			bun: 'bun',
			gzip: 'gzip'
		});
		const none = Object.fromEntries(
			Object.keys(PUBLISH_PROGRAM_NAMES).map((key) => [key, 'tsv_no_such_program'])
		) as typeof PUBLISH_PROGRAM_NAMES;
		return (async () => {
			await using dir = await fs.temp_dir('tsv_publish_deps_test_');
			const { programs } = create_publish_deps(dir, none);
			for (const program of [
				programs.deno,
				programs.deno_build,
				programs.node,
				programs.node_test('x'),
				programs.npm,
				programs.git.local,
				programs.git.user,
				programs.cargo,
				programs.wasm_pack,
				programs.bun,
				programs.gzip
			]) {
				strictEqual(program, null);
			}
		})();
	});

	test('the environments, as decided', () => {
		deepStrictEqual(PUBLISH_PROGRAM_ENV, {
			deno: { pass_env: 'all' },
			deno_build: { pass_env: 'all', env: { TSV_BUILD_FORCE: '1' } },
			node: {},
			npm: { pass_env: 'all' },
			git_local: { pass_env: GIT_LOCAL_PASS_ENV },
			git_user: { pass_env: 'all' },
			cargo: { pass_env: CARGO_PASS_ENV },
			wasm_pack: {},
			bun: {},
			gzip: {}
		});
		deepStrictEqual(node_test_env('crates/x/npm'), {
			pass_env: 'all',
			env: { PKG_DIR: 'crates/x/npm' }
		});
	});

	test('each handle runs in the cwd it is given, with its own environment', async () => {
		await using dir = await fs.temp_dir('tsv_publish_deps_test_');
		const real = (await fs.open_dir(dir.path)).path;
		const { programs } = create_publish_deps(dir, SH);
		const whole = ['TSV_PUBLISH_PROBE=host', 'GIT_PUBLISH_PROBE=git', 'CARGO_PUBLISH_PROBE=cargo'];
		const base = ['TSV_PUBLISH_PROBE=', 'GIT_PUBLISH_PROBE=', 'CARGO_PUBLISH_PROBE=', 'LC_ALL=C'];
		const cases: Array<[string, SpawnProgram | null, Array<string>]> = [
			['deno', programs.deno, [...whole, 'LC_ALL=en_US.UTF-8', 'TSV_BUILD_FORCE=', 'PKG_DIR=']],
			[
				'deno_build',
				programs.deno_build,
				[...whole, 'LC_ALL=en_US.UTF-8', 'TSV_BUILD_FORCE=1', 'PKG_DIR=']
			],
			[
				'node_test',
				programs.node_test('crates/tsv_wasm/pkg/all/npm'),
				[...whole, 'LC_ALL=en_US.UTF-8', 'TSV_BUILD_FORCE=', 'PKG_DIR=crates/tsv_wasm/pkg/all/npm']
			],
			['npm', programs.npm, [...whole, 'LC_ALL=en_US.UTF-8', 'TSV_BUILD_FORCE=', 'PKG_DIR=']],
			[
				'git user',
				programs.git.user,
				[...whole, 'LC_ALL=en_US.UTF-8', 'TSV_BUILD_FORCE=', 'PKG_DIR=']
			],
			[
				'git local',
				programs.git.local,
				[
					'TSV_PUBLISH_PROBE=',
					'GIT_PUBLISH_PROBE=git',
					'CARGO_PUBLISH_PROBE=',
					'LC_ALL=C',
					'TSV_BUILD_FORCE=',
					'PKG_DIR='
				]
			],
			[
				'cargo',
				programs.cargo,
				[
					'TSV_PUBLISH_PROBE=',
					'GIT_PUBLISH_PROBE=',
					'CARGO_PUBLISH_PROBE=cargo',
					'LC_ALL=C',
					'TSV_BUILD_FORCE=',
					'PKG_DIR='
				]
			],
			...(
				[
					['node', programs.node],
					['wasm-pack', programs.wasm_pack],
					['bun', programs.bun],
					['gzip', programs.gzip]
				] as const
			).map(([name, program]): [string, SpawnProgram | null, Array<string>] => [
				name,
				program,
				[...base, 'TSV_BUILD_FORCE=', 'PKG_DIR=']
			])
		];
		for (const [name, program, expected] of cases) {
			deepStrictEqual(await seen(program, { cwd: dir }), [real, ...expected], name);
		}
	});
});

describe('the existence probe', () => {
	test('reads against the checkout, climbs beside it, and follows a linked sibling', async () => {
		await using root = await fs.temp_dir('tsv_publish_deps_test_');
		await root.mkdir('checkout/benches/js/node_modules', { recursive: true });
		await root.mkdir('elsewhere/svelte/packages/svelte/tests', { recursive: true });
		await root.write('checkout/file.txt', 'x');
		await root.symlink('svelte', `${root.path}/elsewhere/svelte`);
		const checkout = `${root.path}/checkout`;
		strictEqual(await exists_beside(checkout, 'benches/js/node_modules'), true);
		strictEqual(await exists_beside(checkout, 'file.txt'), true);
		strictEqual(await exists_beside(checkout, '../svelte/packages/svelte/tests'), true);
		strictEqual(await exists_beside(checkout, '../svelte/packages/svelte/nope'), false);
		strictEqual(await exists_beside(checkout, '../acorn-typescript/test'), false);
		strictEqual(
			await exists_beside(checkout, 'benches/js/node_modules/typescript/lib/x.js'),
			false
		);
		await rejects(exists_beside(checkout, 'file.txt/x'), (error) =>
			is_fs_error(error, 'not_a_directory')
		);
		const deps = create_publish_deps(await fs.open_dir(checkout), SH);
		strictEqual(await deps.exists('../svelte/packages/svelte/tests'), true);
		strictEqual(await deps.exists('../test262/test'), false);
	});
});

describe('the rest of the deps', () => {
	test('the checkout, the snapshot paths, the pause, and the print streams', async () => {
		await using dir = await fs.temp_dir('tsv_publish_deps_test_');
		const deps = create_publish_deps(dir, SH);
		strictEqual(deps.checkout, dir);
		deepStrictEqual(deps.corpora, { root: CORPORA_ROOT, collections: CORPORA_COLLECTIONS });
		const start = performance.now();
		await deps.sleep(50);
		ok(performance.now() - start >= 45, 'the pause waits its milliseconds');
		const module = new URL('./publish_deps.ts', import.meta.url).href;
		const run = spawnSync(
			process.execPath,
			[
				'--input-type=module',
				'-e',
				`import {create_publish_deps} from ${JSON.stringify(module)};
import {fs} from '@fuzdev/gro/fs.ts';
const deps = create_publish_deps(await fs.open_dir('/'));
deps.print('out %s');
deps.print_error('err %s');`
			],
			{ cwd: CHECKOUT, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }
		);
		deepStrictEqual([run.status, run.stdout, run.stderr], [0, 'out %s\n', 'err %s\n']);
	});

	test('the entry is the checkout, the deps, one call, and the exit code from it', () => {
		const source = readFileSync(`${CHECKOUT}/scripts/roots/publish.ts`, 'utf8');
		const code = source
			.replace(/^\/\*\*[\s\S]*?\*\/\n/, '')
			.replace(/^import [^;]*;\n/gm, '')
			.replace(/\s+/g, ' ')
			.trim();
		strictEqual(
			code,
			'process.set_exit_code(await run_publish(create_publish_deps(await open_checkout()), process.args));'
		);
		deepStrictEqual(source.match(/^import [^;]*;$/gm), [
			"import { process } from '@fuzdev/gro/process.ts';",
			"import { run_publish } from '../publish.ts';",
			"import { open_checkout } from './checkout.ts';",
			"import { create_publish_deps } from './publish_deps.ts';"
		]);
	});
});

/**
 * The scripts npm runs by name on its own, as another command's side effect (npm 11's
 * `install`, `ci`, `rebuild`, `pack`, `publish`, `version`, and its reify), never typed. A
 * `pre<name>` or `post<name>` beside a script `<name>` is one too: `npm run <name>` and
 * `deno task <name>` run both around it.
 */
const NPM_LIFECYCLE_SCRIPTS = [
	'preinstall',
	'install',
	'postinstall',
	'prepublish',
	'preprepare',
	'prepare',
	'postprepare',
	'predependencies',
	'dependencies',
	'postdependencies',
	'prepack',
	'postpack',
	'prepublishOnly',
	'publish',
	'postpublish',
	'preversion',
	'version',
	'postversion'
];

describe('how the entry is started', () => {
	test('by its deno.json task alone, which npm never reads', () => {
		const tasks = JSON.parse(readFileSync(`${CHECKOUT}/deno.json`, 'utf8')).tasks;
		strictEqual(tasks.publish, 'node scripts/roots/publish.ts');
	});

	test('the root package.json has no script npm or deno task starts on its own', () => {
		const scripts: Record<string, string> = JSON.parse(
			readFileSync(`${CHECKOUT}/package.json`, 'utf8')
		).scripts;
		const names = Object.keys(scripts);
		deepStrictEqual(
			names.filter((name) => NPM_LIFECYCLE_SCRIPTS.includes(name)),
			[]
		);
		deepStrictEqual(
			names.filter((name) => {
				const around = /^(?:pre|post)(.+)$/.exec(name)?.[1];
				return around !== undefined && around in scripts;
			}),
			[]
		);
	});
});
