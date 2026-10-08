/**
 * Starts the child processes of a bench run and reads what they report — the
 * orchestrator's half of `lib/bench_protocol.ts`.
 *
 * A child is the same runtime as the orchestrator, started on one of the run's own
 * scripts, and the orchestrator does next to nothing while it runs: it awaits the
 * child's exit, waking once a second to look for its result file (see
 * `EXIT_GRACE_MS` for why it looks at all) — so a timed row shares the machine with
 * a parent that holds no engine and allocates nothing, not with a parent's work.
 *
 * @module
 */

import { type ChildProcess, spawn } from 'node:child_process';
import { existsSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { execArgv, execPath } from 'node:process';
import { fileURLToPath } from 'node:url';
import type { ChildSpec } from './bench_protocol.ts';
import { rsvelte_binary_path } from './rsvelte.ts';
import { current_runtime } from './runtime.ts';

/** The run's two child scripts, beside `bench.ts`. */
export type ChildScript = 'bench_preflight.ts' | 'bench_row.ts';

/**
 * How long the pre-flight child may run before the run gives up on it, in ms. Far
 * above any real pre-flight: this bounds a hang, it does not shape a measurement. A
 * timed row's own deadline is priced from what it will run (`lib/bench_plan.ts`
 * `row_deadline_ms`), so a row that hangs before reporting stops the run in minutes.
 */
export const PREFLIGHT_DEADLINE_MS = 2 * 60 * 60_000;

/**
 * How long a child that has written its result may take to exit, in ms, before it is
 * killed and its result accepted.
 *
 * A child's result is its last act and is written whole (`write_child_result`
 * renames it into place), so once it exists the child's measurement is complete and
 * all that is left is process teardown — which can deadlock. Seen once under Node
 * (a `format/svelte/biome-wasm` row, on a busy machine): result written, then the
 * main thread and the last V8 platform worker parked on futexes inside `exit(0)`
 * with every other thread joined, for good, and isolated re-runs of the same spec
 * never repeated it. A run is hundreds of child exits, so a rare hang is a likely
 * one, and without this it would hold the run until the child's deadline.
 * Far above any real teardown, and charged only to a child that hangs.
 */
const EXIT_GRACE_MS = 30_000;

/** How often the orchestrator looks for a running child's result file, in ms. */
const POLL_MS = 1000;

/** The child running now, if any — killed with the orchestrator (`kill_running_child`). */
let running: ChildProcess | null = null;

/**
 * Kill the running child, if any. For the orchestrator's exit path: a child is
 * spawned asynchronously, so an orchestrator that exits (a signal sent to it alone,
 * a thrown error) would otherwise leave its child running on, unobserved.
 */
export const kill_running_child = (): void => {
	running?.kill('SIGKILL');
};

/** What `run_bench_child` read back. */
export interface BenchChildOutcome<TResult> {
	result: TResult;
	/**
	 * The child wrote its result but did not exit within `EXIT_GRACE_MS`, and was
	 * killed. The result stands — it was complete before teardown began — and the
	 * caller records the hang, which says something about the runtime, not the row.
	 */
	exit_hung: boolean;
}

/**
 * The runtime arguments a child is started with, ahead of its script.
 *
 * Node and Bun pass their own on (`execArgv`), so a child gets the orchestrator's
 * flags by construction — `--expose-gc` when the task gave it, nothing when it did
 * not. Deno reports no `execArgv` and grants a child no permission its parent
 * holds, so its list is stated here: this is the ONE place the bench's Deno
 * permissions live (the `bench:deno:run` task itself asks only for what the
 * orchestrator does — read, write the results directory, and run `deno` and `git`).
 *
 * Every child:
 *
 * - `--allow-ffi` — the C-FFI library, and the N-API addons the alternatives load;
 * - `--allow-read`, `--allow-env`, `--allow-sys` — module loading and the packages'
 *   own start-up probes;
 * - `--allow-write` to the run's directory alone — a child's result, and the
 *   pre-flight's file sets;
 * - `--no-prompt` — a missing permission fails the child rather than waiting on a
 *   terminal no one is reading.
 *
 * The pre-flight process alone:
 *
 * - `--allow-run` — `git` (the corpus sources' origins), `gzip` and `deno` (the
 *   size table and the bundles it builds), and this platform's `rsvelte-fmt`
 *   binary (a coverage-only row: one process per file, in pre-flight alone);
 * - `--allow-net`, which the packages it loads all at once may reach for.
 *
 * A timed row's process gets neither: it runs one engine over a file set it was
 * handed, and a row that tried to spawn or connect should fail loudly rather than
 * measure it.
 *
 * `--v8-flags=--expose-gc` follows the orchestrator: if it has no `gc`, the run was
 * started without the flag (a coverage-only run needs none).
 */
const child_runtime_args = (script: ChildScript, write_dir: string): string[] => {
	if (current_runtime() !== 'deno') return [...execArgv];
	const args = [
		'run',
		'--no-prompt',
		...(typeof globalThis.gc === 'function' ? ['--v8-flags=--expose-gc'] : []),
		'--allow-ffi',
		'--allow-read',
		`--allow-write=${write_dir}`,
		'--allow-env',
		'--allow-sys'
	];
	if (script === 'bench_preflight.ts') {
		const run = ['git', 'gzip', 'deno'];
		const rsvelte = rsvelte_binary_path();
		if (rsvelte !== null) run.push(rsvelte);
		args.push(`--allow-run=${run.join(',')}`, '--allow-net');
	}
	return args;
};

/**
 * A child that did not finish on its own terms. `command` is the exact invocation,
 * so the child can be re-run alone against the spec it failed on.
 */
export class BenchChildError extends Error {
	readonly command: string;
	constructor(message: string, command: string) {
		super(message);
		this.name = 'BenchChildError';
		this.command = command;
	}
}

/** One argument, quoted for a POSIX shell when it needs to be. */
const shell_quote = (arg: string): string =>
	/^[\w@%+=:,./-]+$/.test(arg) ? arg : `'${arg.replaceAll("'", `'\\''`)}'`;

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
 * between bindings, a stale artifact) for a reason already printed above. The one
 * exception is a child that hangs AFTER writing its result (`EXIT_GRACE_MS`).
 *
 * @param script - which of the run's scripts to start
 * @param spec - the child's spec, serialized to `spec_path`
 * @param spec_path - where to write it, inside `write_dir`
 * @param write_dir - the one directory a child may write to — the run's own
 * @param label - what the child is, for an error
 * @param deadline_ms - how long the child may run before it is stopped and the run ends
 * @returns the child's result, parsed, and whether its exit hung
 * @throws `BenchChildError` if the child cannot be started, is ended by a signal or its deadline, exits non-zero, or leaves no readable result
 */
export const run_bench_child = async <TResult>(
	script: ChildScript,
	spec: ChildSpec,
	spec_path: string,
	write_dir: string,
	label: string,
	deadline_ms: number
): Promise<BenchChildOutcome<TResult>> => {
	writeFileSync(spec_path, JSON.stringify(spec));
	// so a child that exits cleanly without reporting can never be read as the last
	// child's report
	rmSync(spec.result_path, { force: true });
	const script_path = fileURLToPath(new URL(`../${script}`, import.meta.url));
	const args = [...child_runtime_args(script, write_dir), script_path, spec_path];
	const fail = (message: string): BenchChildError =>
		new BenchChildError(message, [execPath, ...args].map(shell_quote).join(' '));
	const child = spawn(execPath, args, { stdio: 'inherit' });
	running = child;
	let timed_out = false;
	let exit_hung = false;
	const started = Date.now();
	let reported_at: number | null = null;
	const exited = await new Promise<
		{ error: Error } | { code: number | null; signal: NodeJS.Signals | null }
	>(
		(resolve) => {
			const poll = setInterval(() => {
				const now = Date.now();
				if (reported_at === null && existsSync(spec.result_path)) reported_at = now;
				if (reported_at !== null && now - reported_at > EXIT_GRACE_MS) {
					exit_hung = true;
				} else if (now - started > deadline_ms) {
					timed_out = true;
				} else {
					return;
				}
				// once: the `exit` event below ends the wait
				clearInterval(poll);
				child.kill('SIGKILL');
			}, POLL_MS);
			child.once('error', (error) => {
				clearInterval(poll);
				resolve({ error });
			});
			child.once('exit', (code, signal) => {
				clearInterval(poll);
				resolve({ code, signal });
			});
		}
	);
	running = null;
	if ('error' in exited) throw fail(`${label} could not be started: ${exited.error.message}`);
	if (timed_out) {
		throw fail(
			`${label} was still running after ${Math.round(deadline_ms / 60_000)} minutes, its ` +
				`deadline, and was stopped`
		);
	}
	if (!exit_hung) {
		if (exited.signal) throw fail(`${label} was ended by ${exited.signal}`);
		if (exited.code !== 0) {
			throw fail(`${label} failed (exit ${exited.code}) — its own output is above`);
		}
	}
	let text: string;
	try {
		text = readFileSync(spec.result_path, 'utf8');
	} catch {
		throw fail(`${label} exited cleanly but wrote no result to ${spec.result_path}`);
	}
	return { result: JSON.parse(text) as TResult, exit_hung };
};
