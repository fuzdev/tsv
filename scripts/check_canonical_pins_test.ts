/**
 * `check_canonical_pins.ts`' mode flags; what `--checkouts` says when the run has no git (the
 * presence of the `git` handle is what decides whether the drift half runs); a present checkout
 * graded, one whose version file can't be read failing the run and one git fails in warning,
 * neither reading as absent; and `--pins` over fake pin sites.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';
import { SpawnError } from '@fuzdev/gro/spawn_error.ts';
import { afterEach, describe, test, vi } from 'vitest';

import { GATE_CHECKOUT_IDS } from '../benches/js/lib/gate_counts.ts';
import { parse_pins_args, run_check_canonical_pins } from './check_canonical_pins.ts';

test('neither flag or both select both modes; one flag selects its mode', () => {
	deepStrictEqual(parse_pins_args([]), { run_pins: true, run_checkouts: true });
	deepStrictEqual(parse_pins_args(['--pins', '--checkouts']), {
		run_pins: true,
		run_checkouts: true
	});
	deepStrictEqual(parse_pins_args(['--pins']), { run_pins: true, run_checkouts: false });
	deepStrictEqual(parse_pins_args(['--checkouts']), { run_pins: false, run_checkouts: true });
});

test('an argument that is neither flag is a usage error naming it', () => {
	deepStrictEqual(parse_pins_args(['--pins', '--bogus', 'x']), {
		error:
			'FAIL: unknown argument(s) --bogus, x — expected --pins and/or --checkouts (neither = both modes)'
	});
});

/** A read-only handle over `files`; every other path is absent. */
const fake_dir = (files: Record<string, string>): FsReadDir =>
	({
		read_text: async (path: string) => {
			const text = files[path];
			if (text === undefined) throw new Error(`not_found: ${path}`);
			return text;
		}
	}) as unknown as FsReadDir;

/** No sibling checkout present. */
const no_siblings = { open: async () => null };

/** The one pin site every run reads, empty: `--checkouts` alone grades none of it. */
const PIN_SITES = {
	'benches/js/package.json': '{}'
};

const logged: string[] = [];
afterEach(() => {
	vi.restoreAllMocks();
	logged.length = 0;
});
const capture_log = () =>
	vi.spyOn(console, 'log').mockImplementation((line: string) => void logged.push(line));

test('--checkouts without git skips the drift half and says so', async () => {
	capture_log();
	const code = await run_check_canonical_pins(
		{ checkout: fake_dir(PIN_SITES), siblings: no_siblings, git: null },
		{ run_pins: false, run_checkouts: true }
	);
	strictEqual(code, 0);
	deepStrictEqual(logged, [
		'pins:audit:checkouts OK — checkouts aligned (checkout drift not checked — git is not on PATH; ' +
			'svelte checkout absent — alignment not checked; ' +
			'@sveltejs/acorn-typescript checkout absent — alignment not checked)'
	]);
});

test('--checkouts with git asks it for each sibling checkout that exists, and only those', async () => {
	capture_log();
	const asked: unknown[] = [];
	const git = {
		out: async (args: ReadonlyArray<string>, options: unknown) => {
			asked.push({ args, options });
			return { kind: 'exited', code: 128, stdout: '', stderr: '', truncated: false };
		}
	} as unknown as SpawnProgram;
	const code = await run_check_canonical_pins(
		{ checkout: fake_dir(PIN_SITES), siblings: no_siblings, git },
		{ run_pins: false, run_checkouts: true }
	);
	strictEqual(code, 0);
	// no sibling directory exists in the fake, so git is never run
	deepStrictEqual(asked, []);
	strictEqual(logged.length, 1);
	strictEqual(logged[0]!.includes('git is not on PATH'), false);
	strictEqual(logged[0]!.includes('../corpora absent — drift not checked'), true);
});

test('--checkouts runs git in each present sibling checkout, asking for its pinned object', async () => {
	capture_log();
	// the fake checkouts hold no version file, which fails the run; its lines aren't this test's
	vi.spyOn(console, 'error').mockImplementation(() => {});
	const asked: Array<{ args: ReadonlyArray<string>; cwd: unknown }> = [];
	// a handle per sibling, so the test sees which directory each git call ran in
	const handles = new Map<string, FsReadDir>();
	const siblings = {
		open: async (path: string) => {
			let handle = handles.get(path);
			if (handle === undefined) {
				handle = { ...fake_dir({}), path: `/siblings/${path}` } as unknown as FsReadDir;
				handles.set(path, handle);
			}
			return handle;
		}
	};
	const git = {
		out: async (args: ReadonlyArray<string>, options: { cwd: unknown }) => {
			asked.push({ args, cwd: options.cwd });
			return { kind: 'exited', code: 128, stdout: '', stderr: '', truncated: false };
		}
	} as unknown as SpawnProgram;
	await run_check_canonical_pins(
		{ checkout: fake_dir(PIN_SITES), siblings, git },
		{ run_pins: false, run_checkouts: true }
	);
	const corpora = asked.find((a) => a.args.join(' ') === 'rev-parse HEAD:collections');
	strictEqual(corpora?.cwd, handles.get('../corpora'));
	const svelte = asked.find((a) => a.cwd === handles.get('../svelte'));
	deepStrictEqual(svelte?.args, ['rev-parse', 'HEAD']);
});

/** What an `--checkouts` run printed, by stream, and its exit code. */
const run_checkouts = async (
	siblings: NonNullable<Parameters<typeof run_check_canonical_pins>[0]['siblings']>,
	git: SpawnProgram,
	checkout: FsReadDir = fake_dir(PIN_SITES)
): Promise<{ code: number; out: string[]; err: string[] }> => {
	const out: string[] = [];
	const err: string[] = [];
	vi.spyOn(console, 'log').mockImplementation((line: string) => void out.push(line));
	vi.spyOn(console, 'warn').mockImplementation((line: string) => void out.push(line));
	vi.spyOn(console, 'error').mockImplementation((line: string) => void err.push(line));
	const code = await run_check_canonical_pins(
		{ checkout, siblings, git },
		{ run_pins: false, run_checkouts: true }
	);
	return { code, out, err };
};

/** Siblings where only the checkouts in `present` open, each a handle over its `files`. */
const siblings_of = (present: Record<string, Record<string, string>>) => ({
	open: async (path: string) =>
		present[path] === undefined
			? null
			: ({ ...fake_dir(present[path]), path: `/siblings/${path}` } as unknown as FsReadDir)
});

/** A git answering `answer(args, the sibling's path)`. */
const fake_git = (
	answer: (
		args: ReadonlyArray<string>,
		path: string
	) => { code: number; stdout?: string; stderr?: string }
): SpawnProgram =>
	({
		out: async (args: ReadonlyArray<string>, options: { cwd: { path: string } }) => {
			const { code, stdout = '', stderr = '' } = answer(args, options.cwd.path);
			return { kind: 'exited', code, stdout, stderr, truncated: false };
		}
	}) as unknown as SpawnProgram;

describe('--checkouts over a present checkout', () => {
	const CORPORA = GATE_CHECKOUT_IDS['../corpora']!;

	test('one git reads at its pinned ids passes, with no note for it', async () => {
		const git = fake_git((args) => ({ code: 0, stdout: `${CORPORA.hash}0000\n` }));
		const run = await run_checkouts(siblings_of({ '../corpora': {} }), git);
		strictEqual(run.code, 0, run.err.join('\n'));
		deepStrictEqual(run.err, []);
		strictEqual(run.out.length, 1);
		ok(!run.out[0]!.includes('../corpora'), run.out[0]);
	});

	test('a clone without the recorded commit is noted, not failed', async () => {
		const commit = CORPORA.commit!;
		const git = fake_git((args) =>
			args[1] === `${commit}:${CORPORA.tree}`
				? { code: 128, stderr: `fatal: invalid object name '${commit}'\n` }
				: { code: 0, stdout: `${CORPORA.hash}\n` }
		);
		const run = await run_checkouts(siblings_of({ '../corpora': {} }), git);
		strictEqual(run.code, 0, run.err.join('\n'));
		deepStrictEqual(run.err, []);
		ok(
			run.out[0]!.includes(`../corpora lacks the recorded commit ${commit.slice(0, 12)}`),
			run.out[0]
		);
	});

	test('git failing in it warns, naming what git said, never reading it as absent', async () => {
		const git = fake_git((args) =>
			args[1] === `HEAD:${CORPORA.tree}`
				? { code: 128, stderr: `fatal: path '${CORPORA.tree}' does not exist in 'HEAD'\n` }
				: { code: 0, stdout: `${CORPORA.hash}\n` }
		);
		const run = await run_checkouts(siblings_of({ '../corpora': {} }), git);
		strictEqual(run.code, 0, run.err.join('\n'));
		deepStrictEqual(run.out.slice(0, 2), [
			"⚠ pins:audit:checkouts — drift not checked in a present checkout git can't read:",
			`  · ../corpora:${CORPORA.tree}: ../corpora is present but git rev-parse HEAD:${CORPORA.tree} exited 128: ` +
				`fatal: path '${CORPORA.tree}' does not exist in 'HEAD'`
		]);
		ok(!run.out.some((line) => line.includes('../corpora absent')));
	});

	test('a git that fails to start in it warns, naming why', async () => {
		const git = {
			out: async () => {
				throw new Error('not_found: no program git');
			}
		} as unknown as SpawnProgram;
		const run = await run_checkouts(siblings_of({ '../test262': {} }), git);
		strictEqual(run.code, 0, run.err.join('\n'));
		ok(
			run.out.includes(
				'  · ../test262: ../test262 is present but git rev-parse HEAD failed: not_found: no program git'
			),
			run.out.join('\n')
		);
	});

	test('a git that can not start in it warns naming the directory first, not the binary', async () => {
		const git = {
			out: async () => {
				throw new SpawnError(
					'launch_failed',
					'git',
					'git (/usr/bin/git): spawn /usr/bin/git EACCES'
				);
			}
		} as unknown as SpawnProgram;
		const run = await run_checkouts(siblings_of({ '../test262': {} }), git);
		strictEqual(run.code, 0, run.err.join('\n'));
		deepStrictEqual(run.out.slice(0, 3), [
			"⚠ pins:audit:checkouts — drift not checked in a present checkout git can't read:",
			"  · ../test262: ../test262 is present but git rev-parse HEAD couldn't start in it, a directory it can't enter or a git it can't run (git (/usr/bin/git): spawn /usr/bin/git EACCES)",
			'  Each is a broken checkout, not a machine without one; repair it, or remove it.'
		]);
	});

	test('a version file missing in it fails the run as a broken checkout, no pin remedy printed', async () => {
		const git = fake_git(() => ({ code: 0, stdout: 'f'.repeat(40) }));
		const run = await run_checkouts(siblings_of({ '../svelte': {} }), git);
		strictEqual(run.code, 1);
		deepStrictEqual(run.err, [
			"FAIL: a present checkout's version file can't be read:",
			"  · svelte: ../svelte is present but ../svelte/packages/svelte/package.json can't be read (not_found: packages/svelte/package.json)",
			'  Each is a broken checkout, not a machine without one; repair it, or remove it.'
		]);
	});

	test('a checkout at another version fails with the bump remedy and not the pin sites', async () => {
		const git = fake_git(() => ({ code: 0, stdout: 'f'.repeat(40) }));
		const run = await run_checkouts(
			siblings_of({ '../svelte': { 'packages/svelte/package.json': '{"version":"1.0.0"}' } }),
			git,
			fake_dir({ 'benches/js/package.json': JSON.stringify({ dependencies: { svelte: '5.0.0' } }) })
		);
		strictEqual(run.code, 1);
		deepStrictEqual(run.err, [
			'FAIL: canonical version sync broken:',
			'  · svelte: checkout ../svelte/packages/svelte/package.json is v1.0.0 but the oracle pin is v5.0.0 — ' +
				'align the checkout to the pinned tag, or bump the canonical pins deliberately (a fixture re-baseline)',
			'  ⚠ Bumping a canonical pin re-baselines the fixture corpus — see docs/benchmarks.md §"Canonical baseline is coupled".'
		]);
	});

	test('a recorded commit holding another tree fails with neither pin remedy', async () => {
		const git = fake_git((args) => ({
			code: 0,
			stdout: args[1] === `${CORPORA.commit}:${CORPORA.tree}` ? '0'.repeat(40) : `${CORPORA.hash}\n`
		}));
		const run = await run_checkouts(siblings_of({ '../corpora': {} }), git);
		strictEqual(run.code, 1);
		deepStrictEqual(run.err.length, 2, run.err.join('\n'));
		ok(run.err[1]!.includes('re-record GATE_CHECKOUT_IDS'), run.err[1]);
	});

	test('a broken version file and a skewed one each print their block; git failing adds no warning to a failed run', async () => {
		const git = fake_git((args, path) =>
			path === '/siblings/../test262'
				? { code: 128, stderr: 'fatal: not a git repository\n' }
				: { code: 0, stdout: 'f'.repeat(40) }
		);
		const run = await run_checkouts(
			siblings_of({
				'../svelte': {},
				'../acorn-typescript': { 'package.json': '{"version":"1.0.0"}' },
				'../test262': {}
			}),
			git,
			fake_dir({
				'benches/js/package.json': JSON.stringify({
					dependencies: { '@sveltejs/acorn-typescript': '2.0.0' }
				})
			})
		);
		strictEqual(run.code, 1);
		deepStrictEqual(run.err, [
			'FAIL: canonical version sync broken:',
			'  · @sveltejs/acorn-typescript: checkout ../acorn-typescript/package.json is v1.0.0 but the oracle pin is v2.0.0 — ' +
				'align the checkout to the pinned tag, or bump the canonical pins deliberately (a fixture re-baseline)',
			'  ⚠ Bumping a canonical pin re-baselines the fixture corpus — see docs/benchmarks.md §"Canonical baseline is coupled".',
			"FAIL: a present checkout's version file can't be read:",
			"  · svelte: ../svelte is present but ../svelte/packages/svelte/package.json can't be read (not_found: packages/svelte/package.json)",
			'  Each is a broken checkout, not a machine without one; repair it, or remove it.'
		]);
		deepStrictEqual(run.out, []);
	});

	test('a git the spawn module refuses for another reason is named as failing, not as unable to start', async () => {
		const git = {
			out: async () => {
				throw new SpawnError('not_found', 'git', 'git: not found');
			}
		} as unknown as SpawnProgram;
		const run = await run_checkouts(siblings_of({ '../test262': {} }), git);
		strictEqual(run.code, 0, run.err.join('\n'));
		ok(
			run.out.includes(
				'  · ../test262: ../test262 is present but git rev-parse HEAD failed: git: not found'
			),
			run.out.join('\n')
		);
	});

	test('a version file missing or malformed in it fails the run, naming the cause', async () => {
		const git = fake_git(() => ({ code: 0, stdout: 'f'.repeat(40) }));
		for (const [files, cause] of [
			[{}, 'not_found: packages/svelte/package.json'],
			// the parser's own words, whatever they are
			[{ 'packages/svelte/package.json': '{' }, '']
		] as const) {
			const run = await run_checkouts(siblings_of({ '../svelte': files }), git);
			vi.restoreAllMocks();
			strictEqual(run.code, 1);
			ok(
				run.err.some((line) =>
					line.startsWith(
						`  · svelte: ../svelte is present but ../svelte/packages/svelte/package.json can't be read (${cause}`
					)
				),
				run.err.join('\n')
			);
		}
	});
});

const PINS = {
	prettier: '3.9.10',
	'prettier-plugin-svelte': '4.1.1',
	svelte: '5.57.3',
	acorn: '8.19.0',
	'@sveltejs/acorn-typescript': '1.0.13'
};

const OPTIONS = `
	useTabs: true,
	printWidth: 100,
	singleQuote: true,
	trailingComma: 'none',`;

/** A lock installing `extra` beside the five canonical packages and esrap, all flat. */
const sidecar_lock = (
	pins: Record<string, string>,
	extra: Record<string, { version: string }> = {}
): string =>
	JSON.stringify({
		packages: {
			'': { dependencies: pins },
			...Object.fromEntries(
				Object.entries({ ...pins, esrap: '2.4.0' }).map(([name, version]) => [
					`node_modules/${name}`,
					{ version }
				])
			),
			...extra
		}
	});

/** A checkout whose pin sites agree; `edits` replaces files in it. */
const pin_checkout = (edits: Record<string, string> = {}): FsReadDir =>
	fake_dir({
		'crates/tsv_debug/sidecar/sidecar.ts': `return await prettier.format(content, {${OPTIONS}
				parser: options?.parser
			});`,
		'crates/tsv_debug/sidecar/package.json': JSON.stringify({
			engines: { node: '>=24.14' },
			dependencies: PINS
		}),
		'crates/tsv_debug/sidecar/package-lock.json': sidecar_lock(PINS),
		'crates/tsv_debug/sidecar/.npmrc': 'ignore-scripts=true\n',
		'.npmrc': 'ignore-scripts=true\n',
		'package.json': JSON.stringify({ engines: { node: '>=24.14' } }),
		'benches/js/package.json': JSON.stringify({ dependencies: PINS }),
		'benches/js/lib/canonical.ts': `const PRETTIER_OPTIONS = {${OPTIONS}
} as const;`,
		...edits
	});

/** `--pins` over `checkout`: its exit code and what it printed. */
const run_pins = async (checkout: FsReadDir): Promise<{ code: number; text: string }> => {
	const lines: string[] = [];
	vi.spyOn(console, 'log').mockImplementation((line: string) => void lines.push(line));
	vi.spyOn(console, 'error').mockImplementation((line: string) => void lines.push(line));
	const code = await run_check_canonical_pins(
		{ checkout, siblings: null, git: null },
		{ run_pins: true, run_checkouts: false }
	);
	return { code, text: lines.join('\n') };
};

test('--pins passes when the manifests, the lock, and the posture agree', async () => {
	const { code, text } = await run_pins(pin_checkout());
	strictEqual(code, 0, text);
	strictEqual(text.includes('lockfile-only pins hold (esrap@2.4.0)'), true, text);
});

describe('--pins refuses', () => {
	const off_by_a_patch = { ...PINS, svelte: '5.57.4' };
	const cases: Array<[string, Record<string, string>, string]> = [
		[
			"a pin off by a patch in the sidecar's manifest",
			{
				'crates/tsv_debug/sidecar/package.json': JSON.stringify({
					engines: { node: '>=24.14' },
					dependencies: off_by_a_patch
				})
			},
			'svelte: pins disagree'
		],
		[
			'a lock that installs another version than every manifest names',
			{
				'crates/tsv_debug/sidecar/package-lock.json': sidecar_lock(PINS, {
					'node_modules/svelte': { version: '5.57.4' }
				})
			},
			'svelte: crates/tsv_debug/sidecar/package-lock.json installs 5.57.4'
		],
		[
			'a range where an exact pin belongs',
			{
				'crates/tsv_debug/sidecar/package.json': JSON.stringify({
					engines: { node: '>=24.14' },
					dependencies: { ...PINS, acorn: '^8.19.0' }
				}),
				'crates/tsv_debug/sidecar/package-lock.json': sidecar_lock({ ...PINS, acorn: '^8.19.0' }),
				'benches/js/package.json': JSON.stringify({ dependencies: { ...PINS, acorn: '^8.19.0' } })
			},
			'acorn: pinned as ^8.19.0, not an exact version'
		],
		[
			'a nested node_modules key',
			{
				'crates/tsv_debug/sidecar/package-lock.json': sidecar_lock(PINS, {
					'node_modules/magic-string/node_modules/@jridgewell/sourcemap-codec': {
						version: '1.5.0'
					}
				})
			},
			'nests node_modules/magic-string/node_modules/@jridgewell/sourcemap-codec'
		],
		[
			'a duplicated esrap',
			{
				'crates/tsv_debug/sidecar/package-lock.json': sidecar_lock(PINS, {
					'node_modules/svelte/node_modules/esrap': { version: '2.3.1' }
				})
			},
			'esrap: crates/tsv_debug/sidecar/package-lock.json installs 2 copies'
		],
		[
			'a dependency that is not a canonical package',
			{
				'crates/tsv_debug/sidecar/package.json': JSON.stringify({
					engines: { node: '>=24.14' },
					dependencies: { ...PINS, esrap: '2.4.0' }
				})
			},
			'esrap: crates/tsv_debug/sidecar/package.json depends on it'
		],
		[
			"a lock root entry naming what the manifest doesn't",
			{
				'crates/tsv_debug/sidecar/package-lock.json': sidecar_lock({ ...PINS, 'left-pad': '1.3.0' })
			},
			'left-pad: the root entry of crates/tsv_debug/sidecar/package-lock.json depends on it'
		],
		[
			"an .npmrc that isn't the root's",
			{ 'crates/tsv_debug/sidecar/.npmrc': '' },
			'crates/tsv_debug/sidecar/.npmrc differs from .npmrc'
		],
		[
			"an engines floor that isn't the root's",
			{ 'package.json': JSON.stringify({ engines: { node: '>=26' } }) },
			'engines.node is >=24.14'
		]
	];
	for (const [what, edits, expected] of cases) {
		test(what, async () => {
			const { code, text } = await run_pins(pin_checkout(edits));
			strictEqual(code, 1, text);
			strictEqual(text.includes(expected), true, text);
			// a pin site's failure gets both remedies
			strictEqual(text.includes('  Pin sites: '), true, text);
			strictEqual(text.includes('  ⚠ Bumping a canonical pin'), true, text);
		});
	}
});
