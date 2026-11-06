/**
 * Where the PGO pipeline's pieces live: the working directory's layout (one spelling,
 * shared by `pgo.ts`, its Node children and — for the target dirs and profiles —
 * `build.sh`), the cargo artifacts per triple, and a file's parse language.
 *
 * The working directory (`.pgo/` by default) sits OUTSIDE `target/`: every release
 * row caches `target/`, and the profile-use builds must never be restored from a
 * cache — cargo fingerprints the RUSTFLAGS text, not the profile's bytes, so a cached
 * use build would ship an earlier profile's codegen under this one's flag string.
 *
 * Node builtins only, so the pipeline runs under Node as well as Deno.
 *
 * @module
 */

import { join } from 'node:path';

import { cli_binary_name } from '../napi_host.ts';

/** The two training worlds — the abort-profile CLI and the unwind-profile addon train apart,
 * since a profile does not transfer across the panic strategy's CFG changes. */
export const WORLDS = ['cli', 'napi'] as const;
export type World = (typeof WORLDS)[number];

/** A build phase of `build.sh`, as its target dir is named. */
export type Phase = 'instrument' | 'use';

/** The working directory's layout, every path under `dir`. */
export const pgo_paths = (dir: string) => ({
	target: (phase: Phase): string => join(dir, `target-${phase}`),
	logs: join(dir, 'logs'),
	build_log: (phase: Phase, world: World): string =>
		join(dir, 'logs', `build-${phase}-${world}.log`),
	/** A staged `@fuzdev/tsv` loader + platform package, laid out as npm installs them. */
	loader: (phase: Phase): string => join(dir, `loader-${phase}`, 'node_modules'),
	/** The corpus copy one world trains on (training formats it in place). */
	train: (world: World): string => join(dir, 'train', world),
	profraw: (world: World): string => join(dir, 'profraw', world),
	/** Where the training-time `--list` run's own profile goes — never merged. */
	profraw_discard: join(dir, 'profraw', 'discard'),
	profdata: (world: World): string => join(dir, `${world}.profdata`),
	/** What one world's training ran (`TrainedCounts`), the floors the merge's gate reads. */
	trained: (world: World): string => join(dir, 'train', `${world}.counts.json`),
	/** The training corpus as `pgo.ts corpora` resolved it from the pinned commit (the own
	 * collections, the snapshot's `.gitignore`), so the steps that run without git — inside
	 * the musl row's Node container — read the pinned answer rather than a working tree. */
	corpus: join(dir, 'corpus.json'),
	/** The instrumenting rustc's `rustc -vV` (`build.sh instrument`). */
	rustc_instrument: join(dir, 'logs', 'rustc-instrument.txt'),
	byteid: join(dir, 'byteid'),
	/** The depth gate's shapes (`write_shape_limits`) and its scratch. */
	depth: join(dir, 'depth'),
	depth_shapes: join(dir, 'depth', 'shapes.tsv')
});

/** The cdylib filename cargo writes for `tsv_napi` on the triple's OS. */
export const addon_library_name = (triple: string): string =>
	triple.startsWith('win32-')
		? 'tsv_napi.dll'
		: triple.startsWith('darwin-')
			? 'libtsv_napi.dylib'
			: 'libtsv_napi.so';

/**
 * The two shipped artifacts of one cargo target dir: the `release` CLI and the
 * `napi`-profile addon, under `<target>/` for a cross build (`rust_target` set).
 */
export const cargo_artifacts = (
	target_dir: string,
	triple: string,
	rust_target?: string
): { cli: string; addon: string } => {
	const base = rust_target ? join(target_dir, rust_target) : target_dir;
	return {
		cli: join(base, 'release', cli_binary_name(triple)),
		addon: join(base, 'napi', addon_library_name(triple))
	};
};

/** The loader's `index.js` and `cli.js` inside a `pgo_paths().loader` tree. */
export const loader_entries = (node_modules: string): { index: string; cli: string } => ({
	index: join(node_modules, '@fuzdev', 'tsv', 'index.js'),
	cli: join(node_modules, '@fuzdev', 'tsv', 'cli.js')
});

/** The parse language `tsv parse` reads a path as — by extension, as the CLI detects it. */
export const parse_language = (path: string): 'svelte' | 'css' | 'typescript' => {
	const lower = path.toLowerCase();
	if (lower.endsWith('.svelte')) return 'svelte';
	if (lower.endsWith('.css')) return 'css';
	return 'typescript';
};
