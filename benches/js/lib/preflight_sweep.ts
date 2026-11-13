/**
 * One row's untimed pre-flight sweep over its group's files: what it accepted, what it
 * skipped and why, and what tsv crashed on. `bench_preflight.ts` runs it once per row and
 * keeps the results as the run's coverage; the sweep holds no state of its own, so a suite
 * drives it over fake rows.
 *
 * A tsv row's crash (`is_tsv_crash`: a native panic the N-API addon caught, or a WASM trap)
 * is neither an accept nor a skip. Recorded as a skip it would read as one more file the
 * row declines, and when the engine both of tsv's bindings share panics, both rows would
 * skip the same file, their accept sets would still agree, and the run would publish lower
 * coverage without a word. So the sweep lists the crash apart, with its row and file, and
 * the pre-flight fails on it. Every other row's throw stays a skip: a third-party engine's
 * failure is a verdict on the source as far as this bench is concerned.
 *
 * @module
 */

import { empty_output_error } from './bench_sweep.ts';
import { crash_text, is_tsv_crash, type PanicEntry } from './divergence/panic_errors.ts';
import type { Language, ParseGoal, SourceFile } from './types.ts';

/** The part of a bench row (`BenchmarkTask` in `lib/implementations.ts`) a sweep reads. */
export interface PreflightTask {
	name: string;
	/** The implementation slot backing the row: `native` and `wasm` are tsv's own. */
	impl: string;
	tracking_key: string;
	is_async: boolean;
	run: (source: string, language: Language, goal?: ParseGoal) => unknown;
	run_async?: (source: string, language: Language, goal?: ParseGoal) => Promise<unknown>;
}

/**
 * Whether `task` runs tsv's engine: the `native` slot (the C FFI under Deno, the N-API addon
 * under Node and Bun) or the `wasm` one, whichever variant row it backs. Only these rows'
 * throws are classified as crashes.
 */
export const is_tsv_task = (task: Pick<PreflightTask, 'impl'>): boolean =>
	task.impl === 'native' || task.impl === 'wasm';

/** What one row's sweep found. */
export interface PreflightSweep {
	/** The files the row accepted. */
	accepted: Set<string>;
	/** Of `accepted`, the ones accepted only on the Script-goal retry. */
	script_only: Set<string>;
	/** The files the row declined, each with the message it declined with. */
	skipped: Map<string, string>;
	/** tsv's crashes, labeled with the row's tracking key and the file: in no other set. */
	crashes: PanicEntry[];
	/** The accepted files' output digests, or `null` for a row `digest` was not given for. */
	digests: Map<string, string> | null;
	/** Accepted outputs `digest` returned no digest for, or `null` when there were none. */
	ungraded: { count: number; example: string } | null;
	/** The accepted files' bytes. */
	bytes: number;
	/** Wall time of the sweep, digesting included. */
	elapsed_ms: number;
}

/**
 * Run `task` once over every file, untimed.
 *
 * Only the row's call sits inside the `try`: anything else there would turn a harness
 * failure on a file the row handled into a skip against the row. A `goal_fallback` file
 * the row rejects at `module` is retried at `script`, and a rejection there records the
 * module-goal error, the primary reading. A crash at either goal is a crash, and a crash at
 * `module` is not retried.
 *
 * @param task - the row
 * @param files - the group's files
 * @param language - the group's language
 * @param digest - the byte-parity digest of an accepted output, or `null` for a row the
 *   parity check doesn't grade; a nullish output is never digested or counted as ungraded
 */
export async function sweep_preflight_task(
	task: PreflightTask,
	files: readonly SourceFile[],
	language: Language,
	digest: ((result: unknown) => string | null) | null
): Promise<PreflightSweep> {
	const tsv = is_tsv_task(task);
	const sweep: PreflightSweep = {
		accepted: new Set(),
		script_only: new Set(),
		skipped: new Map(),
		crashes: [],
		digests: digest === null ? null : new Map(),
		ungraded: null,
		bytes: 0,
		elapsed_ms: 0
	};
	const crash = (file: SourceFile, e: unknown, at_script: boolean): void => {
		sweep.crashes.push({
			label: `${task.tracking_key}${at_script ? ' (script goal)' : ''}: ${file.path}`,
			error: crash_text(e)
		});
	};
	// the first verdict on a path stands, should the group list it twice
	const skip = (file: SourceFile, message: string): void => {
		if (!sweep.skipped.has(file.path)) sweep.skipped.set(file.path, message);
	};
	const start_ms = performance.now();
	for (const file of files) {
		let result: unknown;
		let at_script = false;
		try {
			// `file.goal` is set only on the conformance surface: test262's declared goal, or a
			// Prettier fixture's extension goal (`.mjs`, `.cts`, …); every other file leaves it
			// undefined, the default module parse
			result = task.is_async
				? await task.run_async!(file.content, language, file.goal)
				: task.run(file.content, language, file.goal);
		} catch (e) {
			if (tsv && is_tsv_crash(e)) {
				crash(file, e, false);
				continue;
			}
			if (!file.goal_fallback) {
				skip(file, error_message(e));
				continue;
			}
			try {
				result = task.is_async
					? await task.run_async!(file.content, language, 'script')
					: task.run(file.content, language, 'script');
			} catch (retry_error) {
				if (tsv && is_tsv_crash(retry_error)) crash(file, retry_error, true);
				else skip(file, error_message(e));
				continue;
			}
			at_script = true;
		}
		// a tool-side verdict, not a harness failure: an empty output on a non-empty input is
		// the row declining the file without saying so (`empty_output_error`)
		const empty = empty_output_error(task.name, file, result);
		if (empty !== null) {
			skip(file, empty.message);
			continue;
		}
		sweep.accepted.add(file.path);
		if (at_script) sweep.script_only.add(file.path);
		sweep.bytes += file.bytes;
		if (digest === null || sweep.digests === null) continue;
		const file_digest = digest(result);
		if (file_digest !== null) {
			sweep.digests.set(file.path, file_digest);
		} else if (result !== undefined && result !== null) {
			// an accepted output that produced no digest: the byte check's blind spot, counted
			if (sweep.ungraded) sweep.ungraded.count += 1;
			else sweep.ungraded = { count: 1, example: file.path };
		}
	}
	sweep.elapsed_ms = performance.now() - start_ms;
	return sweep;
}

const error_message = (e: unknown): string => (e instanceof Error ? e.message : String(e));
