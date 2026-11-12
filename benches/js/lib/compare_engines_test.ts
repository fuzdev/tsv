/**
 * Tests for `compare_engines.ts` and the roots' binding of it (`roots/compare_deps.ts`) over
 * fake engines: the addon loaded and the addon guarded are the one profile's, the guard runs
 * before anything loads, and the roots name the `corpus` profile the tools' tasks build.
 */

import { deepStrictEqual, ok, rejects, strictEqual } from 'node:assert';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, test, vi } from 'vitest';

import { type ArtifactCheck, native_artifact_check } from './check_artifact_freshness.ts';
import { init_compare_implementations } from './compare_engines.ts';
import { native_library_filename } from './runtime.ts';
import { napi_library_path } from './tsv_artifacts.ts';
import { init_implementations } from '../roots/compare_deps.ts';

/** What the fakes saw, in order, and how the guard answers. */
const world = vi.hoisted(() => ({
	calls: [] as string[],
	guarded: [] as ArtifactCheck[],
	stale: false,
	native_fails: false
}));

vi.mock('./check_node_modules.ts', () => ({
	check_node_modules: async () => void world.calls.push('node_modules')
}));

vi.mock('./versions.ts', async (original) => ({
	...(await original<typeof import('./versions.ts')>()),
	load_all_versions: async () => ({ canonical: {} })
}));

vi.mock('./check_artifact_freshness.ts', async (original) => ({
	...(await original<typeof import('./check_artifact_freshness.ts')>()),
	check_artifact_freshness: async (checks: readonly ArtifactCheck[]) => {
		world.calls.push('guard');
		world.guarded.push(...checks);
		// the real guard exits the process on a stale artifact
		if (world.stale) throw new Error('stale');
	}
}));

vi.mock('./canonical.ts', () => ({
	CanonicalImplementation: class {
		async init(): Promise<void> {
			world.calls.push('canonical init');
		}
		dispose(): void {
			world.calls.push('canonical dispose');
		}
	}
}));

vi.mock('./napi.ts', () => ({
	NapiImplementation: class {
		constructor(path: string) {
			world.calls.push(`native load ${path}`);
		}
		async init(): Promise<void> {
			world.calls.push('native init');
			if (world.native_fails) throw new Error('no such addon');
		}
		dispose(): void {
			world.calls.push('native dispose');
		}
	}
}));

const ROOT = fileURLToPath(new URL('../../..', import.meta.url)).replace(/\/$/, '');
const CORPUS_ADDON = `${ROOT}/target/corpus/${native_library_filename('tsv_napi')}`;

beforeEach(() => {
	world.calls = [];
	world.guarded = [];
	world.stale = false;
	world.native_fails = false;
});

afterEach(() => {
	vi.restoreAllMocks();
});

test("the guard checks the named profile's addon before the engines load, and that addon loads", async () => {
	const impls = await init_compare_implementations({ profile: 'release' });
	ok(impls);
	deepStrictEqual(world.guarded, [native_artifact_check('release')]);
	deepStrictEqual(world.calls, [
		'node_modules',
		'guard',
		`native load ${napi_library_path('release')}`,
		'canonical init',
		'native init'
	]);
	impls.dispose();
	deepStrictEqual(world.calls.slice(-2), ['canonical dispose', 'native dispose']);
});

test('a stale addon loads nothing', async () => {
	world.stale = true;
	await rejects(init_compare_implementations({ profile: 'release' }), /stale/);
	deepStrictEqual(world.calls, ['node_modules', 'guard']);
});

test("an addon that fails to load names the profile's rebuild task and frees the oracles", async () => {
	world.native_fails = true;
	const errors: string[] = [];
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void errors.push(a.join(' ')));
	strictEqual(await init_compare_implementations({ profile: 'release' }), null);
	ok(errors.includes('Run: deno task build:napi:release'), errors.join('\n'));
	strictEqual(world.calls.at(-1), 'canonical dispose');
});

test('the roots load and guard the corpus addon, the one their tasks build', async () => {
	const impls = await init_implementations();
	ok(impls);
	deepStrictEqual(
		world.guarded.map((c) => c.path),
		[CORPUS_ADDON]
	);
	strictEqual(world.guarded[0]!.rebuild, 'deno task build:napi:corpus');
	ok(world.calls.includes(`native load ${CORPUS_ADDON}`), world.calls.join('\n'));
});
