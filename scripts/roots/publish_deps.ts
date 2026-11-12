/**
 * The deps `publish`'s entry (`scripts/roots/publish.ts`) hands `run_publish`
 * (`scripts/publish.ts`), built here so a suite reaches each binding without running the entry
 * (`scripts/roots/publish_deps_test.ts`). Its reach:
 *
 * - the checkout, read and written, the working directory of every program
 * - whether a path read against the checkout exists, climbing to the oracle checkouts beside it
 * - each program found by gro's `spawn.find`, which walks `PATH`'s absolute entries only, when
 *   the deps are built (`node_test`'s when Step 6 asks for it), `null` when it isn't there, each
 *   with the environment named in `PUBLISH_PROGRAM_ENV`
 * - the pause between the registry reads that confirm a publish
 *
 * @module
 */

import { basename, dirname, resolve } from 'node:path';

import { GIT_LOCAL_PASS_ENV } from '@fuzdev/gro/git.ts';
import { fs, type FsDir } from '@fuzdev/gro/fs.ts';
import { is_fs_error } from '@fuzdev/gro/fs_error.ts';
import { spawn, type SpawnProgramOptions } from '@fuzdev/gro/spawn.ts';

import { CORPORA_COLLECTIONS, CORPORA_ROOT } from '../../benches/js/lib/corpora.ts';
import { CARGO_PASS_ENV } from '../cargo_env.ts';
import type { PublishDeps, PublishPrograms } from '../publish.ts';

/**
 * Each program's environment, decided one by one:
 *
 * - `deno`: the whole environment. A `deno task` gate or build is an arbitrary subtree (cargo,
 *   rustup, node, npm, bun, wasm-pack, the sidecar) that needs the owner's whole development
 *   setup; `deno_build` adds `TSV_BUILD_FORCE=1`.
 * - `node_test` (`node_test_env`): the whole environment, and `PKG_DIR`: the package suites
 *   are a test subtree of the same kind, steered by the owner's variables (`BENCH_STALE_OK`,
 *   `NODE_OPTIONS`).
 * - `npm`: the whole environment: npm's config, auth, and registry come from it and from the
 *   user's npm config.
 * - `git_user`: the whole environment: `fetch`, `ls-remote`, and `push` reach `origin` through
 *   the owner's credentials, SSH agent, askpass, and proxies, and `commit` and `tag` run their
 *   signing and hooks.
 * - `git_local`: git's local variables (`GIT_LOCAL_PASS_ENV`), for reads of the repository.
 * - `cargo`: cargo's variables (`CARGO_PASS_ENV`), for an offline lockfile update.
 * - `node` (its version and the loader staging, which reads no variable), `wasm-pack` and
 *   `bun` (their versions), `gzip`: gro's base environment alone.
 */
export const PUBLISH_PROGRAM_ENV = {
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
} as const satisfies Record<string, SpawnProgramOptions>;

/** `node_test`'s environment for the package staged at `pkg_dir` (`PUBLISH_PROGRAM_ENV`). */
export const node_test_env = (pkg_dir: string): SpawnProgramOptions => ({
	pass_env: 'all',
	env: { PKG_DIR: pkg_dir }
});

/** The name each program is found by on `PATH`. */
export const PUBLISH_PROGRAM_NAMES = {
	deno: 'deno',
	node: 'node',
	npm: 'npm',
	git: 'git',
	cargo: 'cargo',
	wasm_pack: 'wasm-pack',
	bun: 'bun',
	gzip: 'gzip'
};

/**
 * Whether `path`, read against `checkout_path` and allowed to climb above it, exists. The
 * directory holding it is opened by its absolute path, so a symlink on the way, such as a
 * sibling checkout linked from elsewhere, is followed; the last segment is the handle's own,
 * followed when it is a relative link that stays beneath that directory.
 * @returns `false` when it or a directory above it is missing
 * @throws `FsError` - for any other failure, such as a path through a file
 */
export const exists_beside = async (checkout_path: string, path: string): Promise<boolean> => {
	const absolute = resolve(checkout_path, path);
	let dir: FsDir;
	try {
		dir = await fs.open_dir(dirname(absolute));
	} catch (error) {
		if (is_fs_error(error, 'not_found')) return false;
		throw error;
	}
	return dir.exists(basename(absolute));
};

/**
 * The deps `run_publish` takes, over `checkout`.
 * @param names - the name each program is found by, `PUBLISH_PROGRAM_NAMES` but in a test
 */
export const create_publish_deps = (
	checkout: FsDir,
	names: typeof PUBLISH_PROGRAM_NAMES = PUBLISH_PROGRAM_NAMES
): PublishDeps => {
	const env = PUBLISH_PROGRAM_ENV;
	const programs: PublishPrograms = {
		deno: spawn.find(names.deno, env.deno),
		deno_build: spawn.find(names.deno, env.deno_build),
		node: spawn.find(names.node, env.node),
		node_test: (pkg_dir) => spawn.find(names.node, node_test_env(pkg_dir)),
		npm: spawn.find(names.npm, env.npm),
		git: {
			local: spawn.find(names.git, env.git_local),
			user: spawn.find(names.git, env.git_user)
		},
		cargo: spawn.find(names.cargo, env.cargo),
		wasm_pack: spawn.find(names.wasm_pack, env.wasm_pack),
		bun: spawn.find(names.bun, env.bun),
		gzip: spawn.find(names.gzip, env.gzip)
	};
	return {
		checkout,
		exists: (path) => exists_beside(checkout.path, path),
		corpora: { root: CORPORA_ROOT, collections: CORPORA_COLLECTIONS },
		programs,
		print: (line) => console.log(line),
		print_error: (line) => console.error(line),
		sleep: (ms) => new Promise((resolve) => setTimeout(resolve, ms))
	};
};
