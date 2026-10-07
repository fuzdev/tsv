/**
 * The arithmetic of a timed run that is several processes: which order a pass takes
 * a group's rows in, and what the timings of a row's passes say once pooled.
 *
 * Pure and dependency-free on purpose — no clock, no process, no timing library —
 * so the rules a published number rests on are testable as rules
 * (`bench_plan_test.ts`), apart from the orchestration that applies them
 * (`bench.ts`).
 *
 * @module
 */

/**
 * The order one pass takes a group's rows in.
 *
 * Every row is timed in a process of its own, so no row's engine shares a heap with
 * another's. What a fixed order would still carry between rows is the machine: the
 * row that follows a two-minute sweep starts on a hotter package than the one that
 * follows a five-second one. So no two passes share an order, and no row keeps one
 * neighbour throughout:
 *
 * - even passes run the registration order, odd passes its reverse — a row's
 *   predecessor in one is its successor in the next;
 * - each further pair starts further round the list (the forward and reversed
 *   orders rotated by the same step), so the rows at the ends take a turn in the
 *   middle.
 *
 * Deterministic, so a run is reproducible and two runtimes time a group in the same
 * orders; the order is a balance, not a randomization, and the spread a row shows
 * across its passes (`summarize_passes`) is what is left of the effect.
 *
 * @param rows - the group's rows, in registration order
 * @param pass - the pass, from 0
 * @param passes - how many passes the run makes
 * @returns the rows in this pass's order
 */
export const pass_order = <T>(rows: ReadonlyArray<T>, pass: number, passes: number): T[] => {
	const n = rows.length;
	if (n === 0) return [];
	const base = pass % 2 === 0 ? [...rows] : [...rows].reverse();
	const pairs = Math.ceil(passes / 2);
	const shift = Math.floor((n * Math.floor(pass / 2)) / pairs) % n;
	return [...base.slice(shift), ...base.slice(0, shift)];
};

/**
 * Warmup sweeps for a row: the iteration floor, or as many sweeps as it takes to
 * warm for `floor_ms`, whichever is more — sized from the row's cold pre-flight
 * sweep, which over-estimates a warm sweep and so under-counts a little (fine: the
 * floor is a floor). A row with no pre-flight time gets the iteration floor.
 *
 * Sized once, from the pre-flight, rather than by each pass's own process: every
 * pass of a row then runs one protocol, and so does the same row on another runtime.
 */
export const warmup_iterations_for = (
	preflight_ms: number,
	floor: number,
	floor_ms: number
): number => {
	if (preflight_ms <= 0) return floor;
	return Math.max(floor, Math.ceil(floor_ms / preflight_ms));
};

/**
 * The raw sample count below which `drift` is `null` — one pass's iteration floor.
 * The statistic is one half's median against the other's, and below four a side a
 * single deviant sweep IS the median: at n=5 one slow first sweep read as a −6%
 * drift on rows whose cleaned cv was under 2%. The floor guarantees every full-run
 * pass clears it, so a `null` here means a limited run.
 */
export const DRIFT_MIN_SAMPLES = 8;

const mean_of = (xs: ReadonlyArray<number>): number => xs.reduce((a, b) => a + b, 0) / xs.length;

const median_of = (xs: ReadonlyArray<number>): number => {
	const sorted = [...xs].sort((a, b) => a - b);
	const mid = Math.floor(sorted.length / 2);
	return sorted.length % 2 === 0 ? (sorted[mid - 1] + sorted[mid]) / 2 : sorted[mid];
};

/** The `q`-quantile of `xs` (0–1), linearly interpolated between order statistics. */
const quantile_of = (xs: ReadonlyArray<number>, q: number): number => {
	const sorted = [...xs].sort((a, b) => a - b);
	const at = (sorted.length - 1) * q;
	const low = Math.floor(at);
	const high = Math.ceil(at);
	return sorted[low] + (sorted[high] - sorted[low]) * (at - low);
};

/**
 * Stability statistics over RAW timings — what the MAD-cleaned `cv` cannot see.
 * `cv_raw` is std_dev / mean before outlier removal; `drift` is
 * median(second half) / median(first half) − 1 in iteration order, so it is only
 * meaningful over the timings of ONE process. Both `null` below the sample count
 * that makes them meaningful (2 for `cv_raw`, `DRIFT_MIN_SAMPLES` for `drift`).
 */
export const raw_timing_stats = (
	timings: ReadonlyArray<number>
): { cv_raw: number | null; drift: number | null } => {
	const n = timings.length;
	if (n < 2) return { cv_raw: null, drift: null };
	const mean = mean_of(timings);
	const variance = timings.reduce((a, t) => a + (t - mean) ** 2, 0) / (n - 1);
	const cv_raw = mean > 0 ? Math.sqrt(variance) / mean : null;
	if (n < DRIFT_MIN_SAMPLES) return { cv_raw, drift: null };
	const half = Math.floor(n / 2);
	const first = median_of(timings.slice(0, half));
	const second = median_of(timings.slice(n - half));
	return { cv_raw, drift: first > 0 ? second / first - 1 : null };
};

/** What a row's passes say together — see `summarize_passes`. */
export interface PassSummary {
	/** Every pass's timings, concatenated in pass order — what the row's statistics are taken over. */
	timings_ns: number[];
	/** `cv_raw` over the pooled timings: within-process and between-process variation both. */
	cv_raw: number | null;
	/**
	 * The within-pass drift furthest from zero, signed — `null` when no pass had the
	 * samples for one. Per pass, never over the pooled series: a level shift between
	 * two processes is `pass_spread`'s to report, and reading it as drift would name
	 * the wrong mechanism (drift is a cost that moved WHILE one process was measured).
	 */
	drift: number | null;
	/** Each pass's median sweep time, in pass order. */
	pass_p50_ns: number[];
	/**
	 * How far apart the row's passes sat: the largest pass median over the smallest,
	 * minus one. Zero for a single pass. The between-process reading no in-process
	 * statistic can make — each pass is a fresh process, so this is what one more
	 * draw of the same row could have published instead.
	 */
	pass_spread: number;
}

/**
 * Pool the passes of one row.
 *
 * The row's published statistics are taken over ALL its timings rather than as a
 * mean of per-pass means: the passes need not have the same sweep count (a
 * duration-bound row fits however many its process manages), and a pooled series
 * lets the between-process variation reach `cv` and `cv_raw` instead of being
 * averaged out of sight.
 *
 * @param passes - each pass's raw timings, in pass order; every pass non-empty
 */
export const summarize_passes = (passes: ReadonlyArray<ReadonlyArray<number>>): PassSummary => {
	const timings_ns = passes.flat();
	const pass_p50_ns = passes.map(median_of);
	let drift: number | null = null;
	for (const pass of passes) {
		const d = raw_timing_stats(pass).drift;
		if (d !== null && (drift === null || Math.abs(d) > Math.abs(drift))) drift = d;
	}
	const fastest = Math.min(...pass_p50_ns);
	return {
		timings_ns,
		cv_raw: raw_timing_stats(timings_ns).cv_raw,
		drift,
		pass_p50_ns,
		pass_spread: fastest > 0 ? Math.max(...pass_p50_ns) / fastest - 1 : 0
	};
};

/** The run's between-process noise — see `summarize_process_noise`. */
export interface ProcessNoise {
	/** How many (row, pass pair) comparisons the figures are taken over. */
	pairs: number;
	/** The median deviation between two fresh processes of one row. */
	median: number;
	/** The 95th percentile — the figure to read a small ratio against. */
	p95: number;
	/** The largest. */
	max: number;
}

/**
 * How much two fresh processes of the SAME row disagree, over every row of the run:
 * a process-level A/A the passes give for free.
 *
 * Each pair of a row's passes is one comparison of a thing with itself, so its
 * deviation — the slower pass median over the faster, minus one — is noise by
 * construction. The figures are over every such pair of every timed row. They
 * describe ONE process against one process; a published row pools all its passes
 * and is steadier than this, by how much depending on how independent the passes
 * were, so this is the conservative bound: a ratio between two rows that is inside
 * it is not a difference this run measured.
 *
 * @param rows - each timed row's pass medians (`PassSummary.pass_p50_ns`)
 * @returns the noise figures, or `null` when no row has two passes to compare
 */
export const summarize_process_noise = (
	rows: ReadonlyArray<ReadonlyArray<number>>
): ProcessNoise | null => {
	const deviations: number[] = [];
	for (const medians of rows) {
		for (let i = 0; i < medians.length; i++) {
			for (let j = i + 1; j < medians.length; j++) {
				const low = Math.min(medians[i], medians[j]);
				if (low > 0) deviations.push(Math.max(medians[i], medians[j]) / low - 1);
			}
		}
	}
	if (deviations.length === 0) return null;
	return {
		pairs: deviations.length,
		median: median_of(deviations),
		p95: quantile_of(deviations, 0.95),
		max: Math.max(...deviations)
	};
};
