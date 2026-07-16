/**
 * Skip an expensive build command when its output artifact is already newer
 * than every source that feeds it.
 *
 * Motivation: `wasm-pack` re-runs `wasm-opt` (~8–27s per bundle) even when
 * cargo itself is a no-op, so a fully-cached `deno task bench` still pays
 * ~90s of unconditional wasm-opt across its four bundles. This wrapper is the
 * build-side sibling of `benches/js/lib/check_artifact_freshness.ts`: the
 * `:run` tasks abort when the artifact is STALE; the wrapped build tasks skip
 * when it is FRESH. Same mtime discipline, opposite ends.
 *
 * Freshness inputs (newest mtime wins): every `*.rs` and `Cargo.toml` under
 * the crates that feed the WASM bundle (the run-side guard's `CORE_CRATES` +
 * `WASM_CRATES` — `tsv_wasm` plus the `tsv_ignore`/`tsv_discover` IgnoreStack
 * crates; imported, so the two sides can't drift; the dev-tooling crates
 * are deliberately out, else every `tsv_debug` fixture-workflow edit would
 * force a pointless wasm rebuild), the workspace `Cargo.toml` + `Cargo.lock`
 * (dependency bumps), and
 * `deno.json` (the wrapped command's own flags live there, so editing a build
 * task re-triggers it). What the check CANNOT see is a toolchain change —
 * a wasm-pack / wasm-opt / rustc upgrade produces different bytes from
 * identical sources — so after a toolchain update run once with
 * `TSV_BUILD_FORCE=1` (same blind spot as the harvest stamps' `--force`).
 *
 * The publish path must never skip: `scripts/publish.ts` sets
 * `TSV_BUILD_FORCE=1` around its `build:packages` step, so released bundles
 * are always freshly built regardless of mtimes.
 *
 * Usage:
 *   deno run --allow-read --allow-run scripts/run_if_stale.ts \
 *     --target <artifact-path> -- <command> [args...]
 *
 * Exits 0 on a fresh skip; otherwise runs the command (stdio inherited) and
 * propagates its exit code. A missing target always runs.
 */

import { stat } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

import {
	CORE_CRATES,
	newest_source_mtime,
	type SourceMtime,
	WASM_CRATES,
} from '../benches/js/lib/check_artifact_freshness.ts';

const ROOT = fileURLToPath(new URL('..', import.meta.url));

const newest_source = async (): Promise<SourceMtime> => {
	// Copy — newest_source_mtime memoizes its result object; don't mutate the cache.
	const newest = { ...(await newest_source_mtime([...CORE_CRATES, ...WASM_CRATES])) };
	const consider = async (path: string, label: string): Promise<void> => {
		try {
			const st = await stat(path);
			if (st.isFile() && st.mtimeMs > newest.ms) {
				newest.ms = st.mtimeMs;
				newest.path = label;
			}
		} catch {
			// optional input (e.g. no Cargo.lock in a fresh clone) — ignore
		}
	};
	await consider(`${ROOT}Cargo.toml`, 'Cargo.toml');
	await consider(`${ROOT}Cargo.lock`, 'Cargo.lock');
	await consider(`${ROOT}deno.json`, 'deno.json');
	return newest;
};

const main = async (): Promise<void> => {
	const args = Deno.args;
	const target_flag = args.indexOf('--target');
	const separator = args.indexOf('--');
	if (target_flag === -1 || separator === -1 || separator < target_flag + 2) {
		console.error('usage: run_if_stale.ts --target <artifact-path> -- <command> [args...]');
		Deno.exit(2);
	}
	const target = args[target_flag + 1];
	const command = args.slice(separator + 1);
	if (command.length === 0) {
		console.error('run_if_stale.ts: no command after --');
		Deno.exit(2);
	}

	if (Deno.env.get('TSV_BUILD_FORCE') !== '1') {
		let target_ms: number | null = null;
		try {
			target_ms = (await stat(target)).mtimeMs;
		} catch {
			// missing target — always build
		}
		if (target_ms !== null) {
			const source = await newest_source();
			// Strict `<` so an artifact built in the same second as its source rebuilds.
			if (source.ms < target_ms) {
				console.log(
					`run_if_stale: ${target} is fresh — skipping \`${command[0]}\` (TSV_BUILD_FORCE=1 to force)`,
				);
				return;
			}
		}
	}

	const result = new Deno.Command(command[0], {
		args: command.slice(1),
		stdin: 'inherit',
		stdout: 'inherit',
		stderr: 'inherit',
	}).outputSync();
	if (!result.success) Deno.exit(result.code);
};

await main();
