/**
 * `check_canonical_pins.ts`' mode flags, and what `--checkouts` says when the run has no git:
 * the presence of the `git` handle is what decides whether the drift half runs.
 */

import { deepStrictEqual, strictEqual } from 'node:assert';
import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';
import { afterEach, describe, test, vi } from 'vitest';

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
		},
		read_text_sync: (path: string) => {
			const text = files[path];
			if (text === undefined) throw new Error(`not_found: ${path}`);
			return text;
		},
		dir: (path: string) => {
			throw new Error(`not_found: ${path}`);
		}
	}) as unknown as FsReadDir;

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
		{ checkout: fake_dir(PIN_SITES), siblings: fake_dir({}), git: null },
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
		{ checkout: fake_dir(PIN_SITES), siblings: fake_dir({}), git },
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
	const asked: Array<{ args: ReadonlyArray<string>; cwd: unknown }> = [];
	// a handle per sibling, so the test sees which directory each git call ran in
	const handles = new Map<string, FsReadDir>();
	const siblings = {
		...fake_dir({}),
		dir: (path: string) => {
			let handle = handles.get(path);
			if (handle === undefined) {
				handle = { path: `/siblings/${path}` } as unknown as FsReadDir;
				handles.set(path, handle);
			}
			return handle;
		}
	} as unknown as FsReadDir;
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
	strictEqual(corpora?.cwd, handles.get('corpora'));
	const svelte = asked.find((a) => a.cwd === handles.get('svelte'));
	deepStrictEqual(svelte?.args, ['rev-parse', 'HEAD']);
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
		});
	}
});
