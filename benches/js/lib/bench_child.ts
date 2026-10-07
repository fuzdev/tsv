/**
 * Starts the child processes of a bench run and reads what they report — the
 * orchestrator's half of `lib/bench_protocol.ts`.
 *
 * A child is the same runtime as the orchestrator, started on one of the run's own
 * scripts, and the orchestrator does nothing while it runs: `spawnSync` blocks, so
 * a timed row shares the machine with an idle parent rather than with a parent's
 * event loop, timers or collector.
 *
 * @module
 */

import { spawnSync } from 'node:child_process';
import { readFileSync, rmSync, writeFileSync } from 'node:fs';
import { execArgv, execPath } from 'node:process';
import { fileURLToPath } from 'node:url';
import type { ChildSpec } from './bench_protocol.ts';
import { rsvelte_binary_path } from './rsvelte.ts';
import { current_runtime } from './runtime.ts';

/** The run's two child scripts, beside `bench.ts`. */
export type ChildScript = 'bench_preflight.ts' | 'bench_row.ts';

/**
 * How long one child may run before the run gives up on it, in ms. Far above any
 * real child (the slowest row's pass is minutes): this bounds a hang, it does not
 * shape a measurement.
 */
const CHILD_TIMEOUT_MS = 2 * 60 * 60_000;

/**
 * The runtime arguments a child is started with, ahead of its script.
 *
 * Node and Bun pass their own on (`execArgv`), so a child gets the orchestrator's
 * flags by construction — `--expose-gc` when the task gave it, nothing when it did
 * not. Deno reports no `execArgv` and grants a child no permission its parent
 * holds, so its list is stated here: this is the ONE place the bench's Deno
 * permissions live (the `bench:deno:run` task itself asks only for what the
 * orchestrator does — read, write `write_dir`, and run `deno` and `git`).
 *
 * - `--allow-ffi` — the C-FFI library, and the N-API addons the alternatives load;
 * - `--allow-read`, `--allow-env`, `--allow-sys`, `--allow-net` — module loading
 *   and the packages' own start-up probes;
 * - `--allow-write` — a child's result, and the pre-flight's file sets;
 * - `--allow-run` — `git` (the corpus sources' origins), `gzip` and `deno` (the
 *   size table and the bundles it builds), and this platform's `rsvelte-fmt`
 *   binary (a coverage-only row: one process per file, in pre-flight alone);
 * - `--no-prompt` — a missing permission fails the child rather than waiting on a
 *   terminal no one is reading.
 *
 * `--v8-flags=--expose-gc` follows the orchestrator: if it has no `gc`, the run was
 * started without the flag (a coverage-only run needs none).
 */
const child_runtime_args = (write_dir: string): string[] => {
	if (current_runtime() !== 'deno') return [...execArgv];
	const run = ['git', 'gzip', 'deno'];
	const rsvelte = rsvelte_binary_path();
	if (rsvelte !== null) run.push(rsvelte);
	return [
		'run',
		'--no-prompt',
		...(typeof globalThis.gc === 'function' ? ['--v8-flags=--expose-gc'] : []),
		'--allow-ffi',
		'--allow-read',
		`--allow-write=${write_dir}`,
		`--allow-run=${run.join(',')}`,
		'--allow-env',
		'--allow-net',
		'--allow-sys'
	];
};

/**
 * Run one child to completion and return what it reported.
 *
 * The spec is written to `spec_path` and the child is started on it; the child
 * writes its result to `spec.result_path`. Its stdout and stderr are this
 * process's, so its progress and any diagnostics appear where the run's do.
 *
 * A child that did not finish on its own terms ends the run — there is no partial
 * result to salvage: a row killed mid-pass has measured nothing, and a pre-flight
 * that failed has refused the run (an unlisted coverage failure, a byte mismatch
 * between bindings, a stale artifact) for a reason already printed above.
 *
 * @param script - which of the run's scripts to start
 * @param spec - the child's spec, serialized to `spec_path`
 * @param spec_path - where to write it, inside `write_dir`
 * @param write_dir - the one directory a child may write to
 * @param label - what the child is, for an error
 * @returns the child's result, parsed
 * @throws if the child cannot be started, is ended by a signal or the timeout, exits non-zero, or leaves no readable result
 */
export const run_bench_child = <TResult>(
	script: ChildScript,
	spec: ChildSpec,
	spec_path: string,
	write_dir: string,
	label: string
): TResult => {
	writeFileSync(spec_path, JSON.stringify(spec));
	// so a child that exits cleanly without reporting can never be read as the last
	// child's report
	rmSync(spec.result_path, { force: true });
	const script_path = fileURLToPath(new URL(`../${script}`, import.meta.url));
	const child = spawnSync(execPath, [...child_runtime_args(write_dir), script_path, spec_path], {
		stdio: 'inherit',
		timeout: CHILD_TIMEOUT_MS
	});
	if (child.error) {
		const timed_out = (child.error as NodeJS.ErrnoException).code === 'ETIMEDOUT';
		throw new Error(
			timed_out
				? `${label} was still running after ${CHILD_TIMEOUT_MS / 60_000} minutes and was stopped`
				: `${label} could not be started: ${child.error.message}`
		);
	}
	if (child.signal) throw new Error(`${label} was ended by ${child.signal}`);
	if (child.status !== 0) {
		throw new Error(`${label} failed (exit ${child.status}) — its own output is above`);
	}
	let text: string;
	try {
		text = readFileSync(spec.result_path, 'utf8');
	} catch {
		throw new Error(`${label} exited cleanly but wrote no result to ${spec.result_path}`);
	}
	return JSON.parse(text) as TResult;
};
