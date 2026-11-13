/**
 * Skip an expensive build command when its output artifact is already newer
 * than every source that feeds it.
 *
 * Motivation: `wasm-pack` re-runs `wasm-opt` (~8–27s per bundle) even when
 * cargo itself is a no-op, so a fully-cached `npm run bench` still pays
 * ~90s of unconditional wasm-opt across its four bundles. This wrapper is the
 * build-side sibling of `benches/js/lib/check_artifact_freshness.ts`: the
 * `:run` tasks abort when the artifact is STALE; the wrapped build tasks skip
 * when it is FRESH. Same mtime discipline, opposite ends, one walk
 * (`benches/js/lib/source_mtime.ts`). (The third family
 * member is `scripts/check_staged_freshness.ts`, the same abort posture over
 * the STAGED npm packages the `test:npm:*:run` / `test:napi:npm:run` tasks
 * consume; and `scripts/cargo_build.ts` is what lets all of them read one meaning
 * from a cargo-built artifact's mtime, by dating it at the build.)
 *
 * Freshness inputs (newest mtime wins): every file under `src/`, `build.rs` and
 * `Cargo.toml` of the crates that feed the WASM bundle (`benches/js/lib/crate_lists.ts`'
 * `CORE_CRATES` + `WASM_CRATES` — `tsv_wasm` plus the `tsv_ignore`/`tsv_discover`
 * IgnoreStack crates; that module states the crate set once for every side
 * that must agree on it, so build-side and run-side can't drift; the dev-tooling
 * crates are deliberately out, else every `tsv_debug` fixture-workflow edit would
 * force a pointless wasm rebuild), the workspace `Cargo.toml` + `Cargo.lock`
 * (dependency bumps) + `.cargo/config.toml` (the wasm32 `rustflags`), and `package.json` (the wrapped command's own flags live
 * there, so editing a build task re-triggers it). What the check CANNOT see is a toolchain change —
 * a wasm-pack / wasm-opt / rustc upgrade produces different bytes from
 * identical sources — so after a toolchain update run once with
 * `TSV_BUILD_FORCE=1` (same blind spot as the harvest stamps' `--force`).
 *
 * The published packages never pass through it: `build:packages` chains the
 * `build:npm:*` scripts, plain wasm-pack builds, so released bundles are always
 * freshly built regardless of mtimes.
 *
 * Usage:
 *   node scripts/roots/run_if_stale.ts --target <artifact-path> -- <command> [args...]
 *
 * The target is a path beneath the checkout, and the command runs there. Exits 0 on a fresh
 * skip; otherwise runs the command (stdio inherited) and passes on its exit code, `128 + N` for
 * one a signal `N` ended. A missing target always runs. Exits 2 on a usage error, and 1 for a
 * command that isn't one of `RunIfStaleDeps.programs` or isn't on `PATH`. A source that is there
 * but can't be read (a symlink under a crate's `src/` the checkout handle can't follow, a
 * directory it can't list) throws, naming the path, and runs nothing: a skip needs every source
 * seen.
 *
 * Reach: the checkout read-only, and the programs the root grants (`RunIfStaleDeps`, built by
 * `scripts/roots/run_if_stale.ts`).
 *
 * @module
 */

import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';

import {
	newest_crate_source_mtime,
	newest_workspace_build_mtime,
	type SourceMtime
} from '../benches/js/lib/source_mtime.ts';
import { CORE_CRATES, WASM_CRATES } from '../benches/js/lib/crate_lists.ts';
import { signal_exit_code } from './exit_code.ts';
import { handle_source_tree } from './handle_source_tree.ts';

/** The file the wrapped commands are written in, so editing a build task re-runs it. */
const TASKS_FILE = 'package.json';

/** What `run_if_stale` reaches. */
export interface RunIfStaleDeps {
	/** The checkout: the sources and the target are read beneath it, and the command runs in it. */
	checkout: FsReadDir;
	/** `TSV_BUILD_FORCE=1`: build whatever the mtimes say. */
	force: boolean;
	/** The programs a command may name, by name: `null` for one that isn't on `PATH`. */
	programs: ReadonlyMap<string, SpawnProgram | null>;
	print: (line: string) => void;
	print_error: (line: string) => void;
}

/** The newest mtime across what feeds a WASM bundle (module doc). */
const newest_source = async (checkout: FsReadDir): Promise<SourceMtime> => {
	const tree = handle_source_tree(checkout);
	let newest = await newest_crate_source_mtime(tree, [...CORE_CRATES, ...WASM_CRATES]);
	const workspace = await newest_workspace_build_mtime(tree);
	if (workspace.ms > newest.ms) newest = workspace;
	const tasks = await checkout.stat(TASKS_FILE);
	if (tasks !== null && tasks.mtime_ms > newest.ms) {
		newest = { ms: tasks.mtime_ms, path: TASKS_FILE };
	}
	return newest;
};

/**
 * The wrapper (module doc).
 * @returns the exit code: the command's, `0` on a fresh skip, `2` on a usage error, `1` for a
 *   command it can't run
 */
export async function run_if_stale(
	deps: RunIfStaleDeps,
	argv: ReadonlyArray<string>
): Promise<number> {
	const target_flag = argv.indexOf('--target');
	const separator = argv.indexOf('--');
	if (target_flag === -1 || separator === -1 || separator < target_flag + 2) {
		deps.print_error('usage: run_if_stale.ts --target <artifact-path> -- <command> [args...]');
		return 2;
	}
	const target = argv[target_flag + 1]!;
	const command = argv.slice(separator + 1);
	if (command.length === 0) {
		deps.print_error('run_if_stale.ts: no command after --');
		return 2;
	}
	const name = command[0]!;

	if (!deps.force) {
		const target_info = await deps.checkout.stat(target);
		// a missing target always builds
		if (target_info !== null) {
			const source = await newest_source(deps.checkout);
			// Strict `<` so an artifact built in the same second as its source rebuilds.
			if (source.ms < target_info.mtime_ms) {
				deps.print(
					`run_if_stale: ${target} is fresh — skipping \`${name}\` (TSV_BUILD_FORCE=1 to force)`
				);
				return 0;
			}
		}
	}

	const program = deps.programs.get(name);
	if (program === undefined) {
		deps.print_error(
			`run_if_stale.ts: \`${name}\` is not a command this script runs ` +
				`(${[...deps.programs.keys()].join(', ')})`
		);
		return 1;
	}
	if (program === null) {
		deps.print_error(`run_if_stale.ts: \`${name}\` is not on PATH`);
		return 1;
	}
	// no timeout and no abort signal, so the command ends by itself: `exited` or `signaled`
	const result = await program.run(command.slice(1), { cwd: deps.checkout });
	if (result.kind === 'exited') return result.code;
	if (result.kind === 'signaled') return signal_exit_code(result.signal);
	throw new Error(`run_if_stale.ts: \`${name}\` was stopped (${result.kind})`);
}
