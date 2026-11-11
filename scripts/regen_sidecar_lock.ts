/**
 * Regenerate the sidecar's lock (`crates/tsv_debug/sidecar/package-lock.json`), `pins:lock`.
 *
 * The lock is what pins the oracle's own transitive dependencies — above all `esrap`, the
 * printer that emits the JS `svelte.compile()` returns and therefore the effective oracle for
 * every compile fixture. Nothing else in the repo pins it: svelte depends on it by a caret
 * range. `npm ci --prefix crates/tsv_debug/sidecar` installs exactly the lock, and the actor
 * refuses an install that differs from it, so the lock changes only deliberately, here.
 *
 * This runs `npm install --package-lock-only` in the sidecar's directory, which resolves what
 * `package.json` names and the lock doesn't hold yet and keeps every locked entry as it is: run
 * it after bumping a canonical pin in `crates/tsv_debug/sidecar/package.json`, then
 *
 *   1. diff the lock — a canonical bump should move the bumped package and what it newly
 *      needs, and any OTHER package that moved with it is the oracle shifting under you;
 *   2. update `LOCKED_TRANSITIVE` in `scripts/check_canonical_pins.ts` if a pinned
 *      transitive version changed;
 *   3. re-run `deno task compile:fixtures:validate` (oracle freshness) and
 *      `deno task pins:audit`.
 *
 * Re-resolving a locked transitive (taking a newer `esrap` under svelte's range) is the lock's
 * deletion before this runs, its own deliberate oracle move.
 *
 * - `--check` asks whether the lock is what npm resolves from `package.json` and the lock as
 *   they stand, in a scratch copy of the three files, offline (`--offline`): a lock that holds
 *   everything `package.json` names needs no registry, and one that doesn't fails, naming this
 *   task. Exit 1 when the lock would change or npm can't answer.
 * - `--allow-fresh` resolves versions published inside the root `.npmrc`'s `min-release-age`
 *   window (`--min-release-age=0`). That window is a supply-chain guard — it exists so a
 *   compromised release cannot be pulled in the hour it lands — so this is opt-in, for a
 *   just-published upstream version we've decided to take now rather than tomorrow; without
 *   it, npm silently resolves the previous version. A lock made with it is not flag-dependent:
 *   once the version ages past the window, a plain run keeps it.
 *
 * Its reach (`RegenSidecarLockDeps`, built by `scripts/roots/regen_sidecar_lock.ts`): the
 * checkout, where npm writes the lock; a scratch directory for `--check`; and `npm`.
 *
 * @module
 */

import type { FsDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';

import { SIDECAR_DIR, SIDECAR_INSTALL_COMMAND, SIDECAR_LOCK_PATH } from './sidecar_install.ts';

/** The files npm resolves the lock from, copied into the scratch directory for `--check`. */
const RESOLUTION_FILES = ['package.json', 'package-lock.json', '.npmrc'] as const;

const FLAGS = ['--check', '--allow-fresh'] as const;

/** What `run_regen_sidecar_lock` reaches. */
export interface RegenSidecarLockDeps {
	/** The checkout, where the sidecar's package is. */
	checkout: FsDir;
	/** An empty directory for `--check`'s copy, or `null` for a run that writes the lock. */
	scratch: FsDir | null;
	/** `npm`, run in the sidecar's directory or the scratch copy. */
	npm: SpawnProgram;
}

/**
 * The `npm` arguments that regenerate a lock in the working directory. Install scripts are
 * off by the `.npmrc` beside the lock and again here, and the audit and funding requests,
 * which would reach the registry for nothing the lock needs, are off.
 */
export const to_npm_lock_args = (options: {
	offline: boolean;
	allow_fresh: boolean;
}): Array<string> => [
	'install',
	'--package-lock-only',
	'--ignore-scripts',
	'--no-audit',
	'--no-fund',
	...(options.offline ? ['--offline'] : []),
	...(options.allow_fresh ? ['--min-release-age=0'] : [])
];

/**
 * `pins:lock` with `args` (module doc).
 * @returns the exit code: `1` when `--check` finds the lock would change or npm fails, else `0`
 */
export async function run_regen_sidecar_lock(
	deps: RegenSidecarLockDeps,
	args: ReadonlyArray<string>
): Promise<number> {
	const unknown_args = args.filter((a) => !FLAGS.some((f) => f === a));
	if (unknown_args.length > 0) {
		console.error(
			`FAIL: unknown argument(s) ${unknown_args.join(', ')} — expected ${FLAGS.join(' and/or ')}`
		);
		return 1;
	}
	const check = args.includes('--check');
	const allow_fresh = args.includes('--allow-fresh');
	const committed = await deps.checkout.read_text(SIDECAR_LOCK_PATH);

	let cwd: FsDir;
	if (check) {
		if (deps.scratch === null) throw new Error('--check resolves in a scratch directory');
		cwd = deps.scratch;
		for (const file of RESOLUTION_FILES) {
			await cwd.write(file, await deps.checkout.read(`${SIDECAR_DIR}/${file}`));
		}
	} else {
		cwd = deps.checkout.dir(SIDECAR_DIR);
	}

	const result = await deps.npm.run(to_npm_lock_args({ offline: check, allow_fresh }), {
		cwd: cwd.readonly()
	});
	if (result.kind !== 'exited' || result.code !== 0) {
		console.error(
			check
				? 'FAIL: npm could not resolve the sidecar lock offline — the lock lacks what ' +
						'package.json names; run `deno task pins:lock` and review the diff'
				: 'FAIL: `npm install --package-lock-only` could not resolve the sidecar packages'
		);
		return 1;
	}

	const generated = await cwd.read_text('package-lock.json');
	if (generated === committed) {
		console.log(`sidecar lock already up to date (${SIDECAR_LOCK_PATH})`);
		return 0;
	}
	if (check) {
		console.error(
			`FAIL: ${SIDECAR_LOCK_PATH} is not what npm resolves from package.json and the lock — ` +
				'run `deno task pins:lock` and review the diff'
		);
		return 1;
	}
	console.log(
		`wrote ${SIDECAR_LOCK_PATH} — diff it, then re-run \`deno task pins:audit\` and ` +
			`\`deno task compile:fixtures:validate\`, and reinstall the sidecar (\`${SIDECAR_INSTALL_COMMAND}\`)`
	);
	return 0;
}
