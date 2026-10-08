/**
 * What the processes of one bench run hand each other.
 *
 * A run is an orchestrator (`bench.ts`), one pre-flight child
 * (`bench_preflight.ts`) and one child per timed row per pass (`bench_row.ts`). Each
 * child is started with the path of a JSON spec and writes one JSON result to the
 * path the spec names; nothing rides stdin or stdout, which stay the orchestrator's
 * terminal. This module is the shapes of those files, and the two calls a child
 * makes to read its spec and write its result.
 *
 * Everything here is plain JSON data. The types it names from elsewhere are
 * imported as types only, so a timed row's process — which imports this — loads
 * none of the modules they live in.
 *
 * @module
 */

import type { BenchmarkBudget } from '@fuzdev/fuz_util/benchmark_types.ts';
import { createHash } from 'node:crypto';
import { readFileSync, renameSync, writeFileSync } from 'node:fs';
import { argv } from 'node:process';
import type { CollectedBinarySizes } from './binary_sizes.ts';
import type { ArtifactIdentity } from './check_artifact_freshness.ts';
import type { CorpusRepoRef, CorpusSource, ExclusionCacheState } from './corpus.ts';
import type {
	BenchmarkTaskOptions,
	ImplKey,
	RowIdentity,
	UnavailableImpl
} from './implementations.ts';
import type { GroupOmissions } from './perf_omit.ts';
import type { ReportVersions, SourceCoverageCell } from './report.ts';
import type { Language, SourceFile } from './types.ts';

/** What every child's spec carries. */
export interface ChildSpec {
	/** Where the child writes its one JSON result. */
	result_path: string;
}

/** The pre-flight child's spec. */
export interface PreflightSpec extends ChildSpec {
	/** Whether the process's progress goes to stderr — see `create_logger`. */
	log_to_stderr: boolean;
	/** The run's scratch directory, where the file sets are written (`PreflightTimedRow.file_set`). */
	run_dir: string;
}

/** One same-engine pair that disagreed — on its accept set, its output, or both. */
export interface VariantParityFinding {
	group: string;
	/** The base row of the pair (the native binding, or the default-option wire). */
	impl: string;
	/** Its same-engine sibling (the wasm binding, or the reduced-option wire). */
	sibling: string;
	/** Files only `impl` accepted. */
	impl_only: number;
	/** Files only `sibling` accepted. */
	sibling_only: number;
	/**
	 * Files BOTH accepted whose outputs differ byte-for-byte. Always `0` for a pair
	 * `sibling_outputs_must_match` doesn't grade — those carry no digests, and a
	 * zero there means "not measured", not "agreed". Non-zero is fatal.
	 */
	output_mismatch: number;
	/** Up to three `output_mismatch` paths, so the failure names files, not just a count. */
	output_mismatch_examples: string[];
}

/** What the timed phase needs to know about a row pre-flight cleared for timing. */
export interface PreflightTimedRow {
	/**
	 * The file holding the row's timed set: a JSON array of `SourceFile`, in sweep
	 * order, written by the pre-flight process from the very strings it ran the row
	 * on. A timed row's process reads this one file and never the corpus, so what it
	 * sweeps is byte-for-byte what pre-flight accepted — a harvested stylesheet that
	 * exists in no file on disk included. The rows of a group share one file in
	 * `intersection` mode.
	 */
	file_set: string;
	/** How many files the set holds — the row's `files_iterated`. */
	files: number;
	/** sha1 (first 12 hex digits) of the set's sorted paths — `files_iterated_digest`. */
	digest: string;
	/**
	 * Paths the row accepted only at the Script goal (`SourceFile.goal_fallback`),
	 * which its timed sweep must replay there: at the file's own goal the call throws,
	 * and inside the timed loop a throw is a harness failure rather than a skip.
	 */
	script_only: string[];
	/**
	 * Wall-clock ms of the row's one cold pre-flight sweep — what prices the deadline
	 * of its timed processes (`lib/bench_plan.ts` `row_deadline_ms`), an over-estimate
	 * of a warm sweep, which is the side a deadline wants.
	 */
	preflight_ms: number;
}

/** One row of a group, as pre-flight measured it. */
export interface PreflightRow {
	/** The row's name within its group. */
	name: string;
	/** `<group>/<key>` — the harness's own handle on the row. */
	tracking_key: string;
	/** The implementation slot behind it. */
	impl: ImplKey;
	/** Measured for coverage and never timed — `BenchmarkTask.coverage_only`. */
	coverage_only: boolean;
	/** Files the row accepted in pre-flight. */
	processed: number;
	/** Files it was offered — the language's corpus. */
	total: number;
	/**
	 * The bytes behind the row's throughput: of its timed set for a timed row, of what
	 * it accepted for a row that is not timed.
	 */
	effective_bytes: number;
	/** `null` for a coverage-only row, and for every row of a coverage-only run. */
	timed: PreflightTimedRow | null;
}

/** One `operation/language` group, its rows in registration order. */
export interface PreflightGroup {
	name: string;
	operation: 'parse' | 'format';
	language: Language;
	rows: PreflightRow[];
}

/**
 * Everything the pre-flight process learned, as the orchestrator needs it: the plan
 * for the timed phase (`groups[].rows[].timed`) and every fact the report states
 * that only a process holding the corpus and the implementations could establish.
 *
 * The pre-flight process exits once this is written. The corpus and every loaded
 * engine go with it, so the timed rows run on a machine that holds neither.
 */
export interface PreflightSnapshot {
	/** Files per language after `BENCH_FILTER` / `BENCH_LIMIT`. */
	corpus: Record<Language, number>;
	/** Bytes per language, of the same files. */
	bytes_by_language: Record<Language, number>;
	corpus_sources: CorpusSource[];
	exclusion_caches: ExclusionCacheState[];
	corpus_snapshot: CorpusRepoRef | null;
	/** The canonical oracles' versions and whichever alternatives loaded. */
	versions: ReportVersions;
	binary_sizes: CollectedBinarySizes;
	/** Load failures, each with the rows it cost this surface. */
	unavailable: UnavailableImpl[];
	/** The conformance report's row-composition disclosures, already checked against the registry. */
	surface_disclosure_prose: string[];
	/** Every group pre-flight ran, in `LANGUAGES × OPERATIONS` order. */
	groups: PreflightGroup[];
	/** `tracking_key → path → error`, for every file a row failed. */
	skipped_files: Record<string, Record<string, string>>;
	/** `group → source → row → cell`; coverage-only runs only. */
	coverage_by_source: Record<string, Record<string, Record<string, SourceCoverageCell>>> | null;
	/** Perf surface, intersection mode, timed runs only. */
	omissions: GroupOmissions[] | null;
	output_digest_ungraded: Record<string, number>;
	variant_parity: VariantParityFinding[];
	/** Third-party stderr noise the pre-flight process silenced, by pattern. */
	suppressed_noise: Record<string, number>;
	/**
	 * The artifacts this runtime executes, as pre-flight loaded and graded them —
	 * every timed row's process checks it loads the same bytes (`RowSpec.artifacts`).
	 */
	artifacts: ArtifactIdentity[];
}

/** One pass of one timed row: what its process is asked to measure. */
export interface RowSpec extends ChildSpec {
	row: RowIdentity;
	/** The run's registry options — the ones pre-flight asked the registry with. */
	task_options: BenchmarkTaskOptions;
	/** `PreflightTimedRow.file_set`. */
	file_set: string;
	/** `PreflightTimedRow.digest`, which the process checks its loaded set against. */
	files_digest: string;
	/** `PreflightTimedRow.script_only`. */
	script_only: string[];
	/** The pass's timed budget in ms. */
	duration_ms: number;
	/** Untimed sweeps before the clock starts — the fewest; see `warmup_ms`. */
	warmup_iterations: number;
	/**
	 * The least wall time the warmup runs for, in ms (`BENCH_WARMUP_MS`): it sweeps
	 * until both this and `warmup_iterations` are reached (`bench_row.ts` `warm_up`).
	 */
	warmup_ms: number;
	/** The fewest timed sweeps, whatever the duration budget says. */
	min_iterations: number;
	/** `BENCH_GC`: force a major collection between every two timed sweeps. */
	gc_each_iteration: boolean;
	/**
	 * `PreflightSnapshot.artifacts` — checked before the process loads anything, so a
	 * build mid-run ends the run rather than timing a binary pre-flight never graded.
	 */
	artifacts: ArtifactIdentity[];
}

/** What one pass of one timed row measured. */
export interface RowResult {
	/** Every timed sweep, in order, in nanoseconds. */
	timings_ns: number[];
	/** The protocol the timing library resolved for the pass. */
	budget: BenchmarkBudget;
	/** The pass's whole wall time in ms — settle, warmup and measurement. */
	total_time_ms: number;
	/**
	 * Warmup sweeps actually run — at least the spec's count, more when the wall-time
	 * floor needed them (the library's own count is 0: the row warms in its `setup`).
	 */
	warmup_iterations: number;
	/** The JS heap (`heapUsed`) the warmup began from, straight after a full collection. */
	settled_heap_bytes: number;
	/** Third-party stderr noise the process silenced, by pattern. */
	suppressed_noise: Record<string, number>;
}

/**
 * Read this process's spec — the JSON file named by its first argument.
 *
 * @throws if the argument is missing or the file is not JSON
 */
export const read_child_spec = <T extends ChildSpec>(): T => {
	const spec_path = argv[2];
	if (!spec_path) {
		throw new Error(
			'no spec path — this script is one process of a bench run, started by benches/js/bench.ts'
		);
	}
	return JSON.parse(readFileSync(spec_path, 'utf8')) as T;
};

/**
 * Write this process's result where its spec says — whole or not at all: written
 * beside it and renamed into place, so the file existing means the result is
 * complete. The orchestrator relies on that (`lib/bench_child.ts` `EXIT_GRACE_MS`).
 */
export const write_child_result = (spec: ChildSpec, result: unknown): void => {
	const partial = `${spec.result_path}.partial`;
	writeFileSync(partial, JSON.stringify(result));
	renameSync(partial, spec.result_path);
};

/**
 * sha1 (first 12 hex digits) of a file set's newline-joined SORTED paths — the
 * identity of a timed set: what pre-flight records (`PreflightTimedRow.digest`, the
 * report's `files_iterated_digest`) and what a row's process checks its loaded set
 * against, so the two can never hash it two ways.
 */
export const file_set_digest = (files: ReadonlyArray<Pick<SourceFile, 'path'>>): string =>
	createHash('sha1')
		.update(
			files
				.map((f) => f.path)
				.sort()
				.join('\n')
		)
		.digest('hex')
		.slice(0, 12);
