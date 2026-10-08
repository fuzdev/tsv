/**
 * One pass of one timed row, in a process of its own.
 *
 * `bench.ts` starts this script once per row per pass:
 *
 * ```bash
 * <runtime> benches/js/bench_row.ts <path to a RowSpec JSON>
 * ```
 *
 * The process loads the row's file set and the ONE engine the row runs, warms it,
 * times its sweeps, writes a `RowResult` and exits. Nothing else that is measured
 * is ever in the process, and that is the point: in one shared process the rows
 * moved each other's numbers. Engines that share code shape each other's type
 * feedback, the collector sizes its young generation from what has been allocating,
 * and a wasm instance another row grew stays grown — none of which a forced
 * collection between rows undoes, since it resets where a row starts and not the
 * regime it runs in.
 *
 * What is deliberately NOT here: the corpus loader (the set is one file the
 * pre-flight process wrote from the strings it ran — `PreflightTimedRow.file_set`),
 * the run's environment parsing, the report. The row's whole protocol rides its
 * spec, so every pass of a row, on every runtime, runs the same one.
 *
 * In order, before the clock:
 *
 * 1. the file set is read and checked against the digest pre-flight recorded, and
 *    the executed artifacts against the bytes pre-flight graded;
 * 2. the row's engine is loaded and probed on the row's own call (`init_row_task`);
 * 3. a full collection, so the warmup starts from the heap those two left and not
 *    from their garbage (`settled_heap_bytes` records where);
 * 4. the warmup sweeps — a count AND a wall-time floor (`warm_up`).
 *
 * @module
 */

// Type declaration for V8's gc function (available with --expose-gc)
declare global {
	var gc: (() => void) | undefined;
}

import { Benchmark } from '@fuzdev/fuz_util/benchmark.ts';
import { Buffer } from 'node:buffer';
import { readFileSync } from 'node:fs';
import { exit, memoryUsage } from 'node:process';
import {
	file_set_digest,
	read_child_spec,
	type RowResult,
	type RowSpec,
	write_child_result
} from './lib/bench_protocol.ts';
import { assert_output_present, suppress_stderr_noise } from './lib/bench_sweep.ts';
import { assert_artifacts_unchanged } from './lib/check_artifact_freshness.ts';
import { init_row_task } from './lib/implementations.ts';
import type { SourceFile } from './lib/types.ts';

const spec = read_child_spec<RowSpec>();
const { row } = spec;
const label = `${row.operation}/${row.language}/${row.name}`;

// before any engine loads — see `suppress_stderr_noise`
const suppressed_noise = suppress_stderr_noise();

/**
 * The row's timed set, read back from the file pre-flight wrote and checked against
 * the digest it recorded. Inside a function so the set as `JSON.parse` returned it is
 * garbage once this returns: only the rebuilt files outlive it, and the heap the row
 * is timed in holds the set once.
 *
 * Each file is rebuilt once, ahead of the sweep, so the timed loop pays nothing per file:
 *
 * - its CONTENT is decoded again from its UTF-8 bytes, so it is held as a file read
 *   holds it — as pre-flight held it, and as a consumer does. One `JSON.parse` over the
 *   whole set is not that under JSC: when any file of the set holds a character past
 *   Latin-1, bun returns EVERY string of the parse in its 16-bit form, ASCII files
 *   included, and a row timed on those measured up to ~1.5% slower. V8 keeps the
 *   narrow form either way, so without this the cost fell on one runtime's column
 *   alone. The round trip is the identity for any string a UTF-8 file read produced;
 *   one that is not (a lone surrogate) fails here rather than being timed altered.
 * - a file the row accepted only at the Script goal is replayed there — at its own
 *   goal the call throws, and here a throw is a harness failure rather than a skip.
 */
const load_file_set = (): SourceFile[] => {
	const loaded = JSON.parse(readFileSync(spec.file_set, 'utf8')) as SourceFile[];
	const digest = file_set_digest(loaded);
	if (digest !== spec.files_digest) {
		throw new Error(
			`${label}: the file set at ${spec.file_set} is not the one pre-flight planned ` +
				`(digest ${digest}, expected ${spec.files_digest})`
		);
	}
	const script_only = new Set(spec.script_only);
	return loaded.map((f): SourceFile => {
		const content = Buffer.from(f.content, 'utf8').toString('utf8');
		if (content !== f.content) {
			throw new Error(`${label}: ${f.path} does not survive a UTF-8 round trip`);
		}
		return { ...f, content, ...(script_only.has(f.path) ? { goal: 'script' } : {}) };
	});
};
const files = load_file_set();

// Before anything loads: the artifacts are the ones pre-flight graded, not a build
// that landed in this checkout since (`assert_artifacts_unchanged`).
assert_artifacts_unchanged(spec.artifacts);

const { task, impl } = await init_row_task(row, spec.task_options);

// One timed iteration: the row's call on every file of the set, in order. The set
// holds only files this row accepted in pre-flight (or the group's intersection),
// so a throw is a real bug — it propagates and fails the pass rather than being
// cataloged. `assert_output_present` is the same per-file verdict pre-flight took.
const { language } = row;
const sweep: () => void | Promise<void> = task.is_async
	? async () => {
			for (const file of files) {
				assert_output_present(
					task.name,
					file,
					await task.run_async!(file.content, language, file.goal)
				);
			}
		}
	: () => {
			for (const file of files) {
				assert_output_present(task.name, file, task.run(file.content, language, file.goal));
			}
		};

/**
 * Force a major collection and record the heap it leaves — the heap the warmup
 * begins from. In a process of its own that is the row's engine plus its file set,
 * so the reading is a property of the row: it should agree across the row's passes,
 * and one that does not is the first thing to compare when their timings do not.
 * No-ops (recording the unsettled heap) when the runtime wasn't started with
 * `--expose-gc`; `bench.ts` warns before the timed phase.
 */
let settled_heap_bytes = 0;
const settle_heap = (): void => {
	globalThis.gc?.();
	settled_heap_bytes = memoryUsage().heapUsed;
};

// For an impl whose heap a collection cannot settle (`TsvImplementation.reset_heap`;
// today biome, whose wasm linear memory leaks per call and never shrinks). The leak
// is the engine's own, so a process of its own does not remove it: it still grows
// with every sweep of this one pass.
const reset_heap = impl.reset_heap ? (): void => impl.reset_heap!() : undefined;

/**
 * The warmup: at least `spec.warmup_iterations` sweeps AND at least `spec.warmup_ms`
 * of wall time, whichever ends later. Returns the sweeps it ran.
 *
 * Neither floor is keyed on anything a row measured, here or in another process, so
 * it is one rule for every pass of every row on every runtime. The wall-time floor is
 * what keeps a fast row from entering its window still tiering — this process starts
 * its row cold, and JSC keeps tiering for seconds of wall time — and the sweep floor
 * gives a slow row a few sweeps of warmup however long one takes. The count the two
 * land on is the reading, published per row (`warmup_iterations`).
 *
 * Run in the task's `setup` (untimed — the library excludes it) rather than by the
 * library's warmup loop, which has neither a clock nor a between-sweeps hook: a
 * `reset_heap` row is offered its reset after every warmup sweep, so its first timed
 * sweep starts on the footing every later one gets from `on_iteration` (the impl
 * decides whether a reset is due), instead of on every warmup sweep's leak at once.
 */
const warm_up = async (): Promise<number> => {
	reset_heap?.();
	const start = performance.now();
	let sweeps = 0;
	while (sweeps < spec.warmup_iterations || performance.now() - start < spec.warmup_ms) {
		await sweep();
		reset_heap?.();
		sweeps++;
	}
	return sweeps;
};

const bench = new Benchmark({
	duration_ms: spec.duration_ms,
	// `warm_up` instead, from the task's `setup`
	warmup_iterations: 0,
	min_iterations: spec.min_iterations,
	// One task, so there is no next task to cool down for — and no timer may be
	// awaited here regardless: oxfmt's async napi binding stalls Deno's timer wheel
	// after its first call (benches/js/CLAUDE.md §Known Issues).
	cooldown_ms: 0,
	// Runs between one sweep's end timer and the next's start timer — outside
	// every timing, inside the duration budget, never during warmup. Two things
	// live here: the opt-in forced GC, and the per-sweep heap reset.
	on_iteration:
		spec.gc_each_iteration || reset_heap
			? () => {
					if (spec.gc_each_iteration) globalThis.gc?.();
					reset_heap?.();
				}
			: undefined
});

let warmup_sweeps = 0;
bench.add({
	name: task.name,
	setup: async () => {
		settle_heap();
		warmup_sweeps = await warm_up();
	},
	fn: sweep,
	async: task.is_async
});

const [measured] = await bench.run();

const result: RowResult = {
	timings_ns: measured.timings_ns,
	budget: measured.budget,
	total_time_ms: measured.total_time_ms,
	warmup_iterations: warmup_sweeps,
	settled_heap_bytes,
	suppressed_noise: Object.fromEntries(suppressed_noise)
};
write_child_result(spec, result);

// Explicitly: a binding may leave a handle or a thread alive, and a row that has
// reported must not keep the run waiting on it.
exit(0);
