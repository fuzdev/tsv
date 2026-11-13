/**
 * F1 (idempotency) sweep over the **real-code** corpus.
 *
 * Drives every file of the `robustness` view — the WHOLE `../corpora` snapshot (every
 * collection it vendors, placed or not), the `svelte_styles` cache,
 * plus the live diff (the files of this machine's
 * working trees of the `own` repos that differ from, or are absent in, the snapshot) —
 * through `tsv_debug fuzz --iterations 0`:
 * the fuzzer's
 * pristine pass, which asserts the three invariants on each seed **as authored**,
 * with no mutation. The load-bearing one here is **F1** — `format(format(x)) ==
 * format(x)`, tsv's core "input always formats to itself" invariant — alongside
 * no-panic and structural reparse. The pass also runs the fuzzer's two extra legs on
 * each seed: the `locations` wire of every input tsv parses (no panic), and the type
 * strip of every TypeScript-family input (no panic, the position contract, idempotence).
 *
 * Why this isn't in `npm run check`: the corpus is sibling checkouts (the snapshot
 * is not cloned in CI; the working trees are machine-dependent) and the sweep is
 * minutes, not seconds. `check`'s `fuzz:audit`
 * covers `tests/fixtures`; this covers real code, which is a different risk surface —
 * a formatter can be idempotent on every curated fixture and still reflow a real
 * component on the second pass. Run it at conformance/release cadence, or after any
 * printer change.
 *
 * Builds with `--profile corpus` (release + `panic = "unwind"`), because the fuzzer
 * drives each file under `catch_unwind` — a `panic = "abort"` release build would
 * take the process down instead of reporting the panic.
 *
 * `run_idempotency_sweep` is the library function; its root,
 * `benches/js/roots/idempotency_sweep.ts`, binds its deps (`IdempotencySweepDeps`).
 *
 * @module
 */

import type { Logger } from './lib/types.ts';

/** What a corpus tool's `cargo` returns: a code when it exited. */
export type CargoStatus =
	{ readonly kind: 'exited'; readonly code: number } | { readonly kind: string };

/** What `run_idempotency_sweep` reaches the world through. */
export interface IdempotencySweepDeps {
	/** The `robustness` view's seeds (`corpus_robustness_seeds`). */
	seeds(logger: Logger): Promise<{ dirs: string[]; live_files: string[] }>;
	/** `cargo` in the checkout, its output inherited. */
	cargo: { run(args: ReadonlyArray<string>): Promise<CargoStatus> };
}

/**
 * The sweep.
 * @returns the exit code: the sweep's own, `1` when it didn't exit or found no corpus
 */
export async function run_idempotency_sweep(deps: IdempotencySweepDeps): Promise<number> {
	const { dirs, live_files } = await deps.seeds((...args) => console.error(...args));
	const seeds = [...dirs, ...live_files];

	if (seeds.length === 0) {
		console.error('Error: no corpus present — nothing to sweep.');
		return 1;
	}

	console.error(
		`idempotency sweep over ${dirs.length} directory seeds + ${live_files.length} live-diff files\n`
	);

	const status = await deps.cargo.run([
		'run',
		'--profile',
		'corpus',
		'-q',
		'-p',
		'tsv_debug',
		'--',
		'fuzz',
		'--iterations',
		'0',
		...seeds
	]);
	return exit_code_of(status) ?? 1;
}

/** The code `status` exited with, or `null` when it didn't exit (`spawnSync`'s `status`). */
export const exit_code_of = (status: CargoStatus): number | null =>
	'code' in status && status.kind === 'exited' ? status.code : null;
