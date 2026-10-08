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

/** One pass of one row: every sweep it timed, and the sweeps its outlier cleaner kept. */
export interface PassTimings {
	/** Every timed sweep, in order. */
	timings_ns: ReadonlyArray<number>;
	/** The sweeps the outlier cleaner kept, cleaned over THIS pass alone — see `summarize_passes`. */
	cleaned_ns: ReadonlyArray<number>;
}

/** What a row's passes say together — see `summarize_passes`. */
export interface PassSummary {
	/** Every pass's raw timings, concatenated in pass order — the upper-tail percentiles' set. */
	timings_ns: number[];
	/** Every pass's CLEANED timings, concatenated — the set `min` and the dispersion are read from. */
	cleaned_ns: number[];
	/** The row's published mean: the passes' cleaned means, weighted equally. */
	mean_ns: number;
	/** Each pass's cleaned mean, in pass order. */
	pass_mean_ns: number[];
	/**
	 * The sample standard deviation of `pass_mean_ns` (Bessel-corrected) — how far one
	 * more process's mean could land, and what a comparison of two runs' means has to
	 * clear. `null` with one pass, which can say nothing about it.
	 */
	pass_mean_sd_ns: number | null;
	/** `cv_raw` over the pooled timings: within-process and between-process variation both. */
	cv_raw: number | null;
	/**
	 * The within-pass drift furthest from zero, signed — `null` when no pass had the
	 * samples for one. Per pass, never over the pooled series: a level shift between
	 * two processes is `pass_spread`'s to report, and reading it as drift would name
	 * the wrong mechanism (drift is a cost that moved WHILE one process was measured).
	 */
	drift: number | null;
	/**
	 * How far apart the row's passes sat: the largest pass mean over the smallest,
	 * minus one. Zero for a single pass. The between-process reading no in-process
	 * statistic can make — each pass is a fresh process, so this is what one more
	 * draw of the same row could have published instead.
	 */
	pass_spread: number;
}

/**
 * Pool the passes of one row. Three choices, each because the alternative measured
 * something other than the row:
 *
 * - **Each pass is cleaned on its own** (the caller hands in `cleaned_ns`). The
 *   outlier cleaner exists for transients WITHIN a process — a GC pause, a
 *   descheduled sweep. Over the pooled series it reads a level shift between two
 *   processes as outliers instead, and trims a minority pass in part or whole: with
 *   three floor-bound passes a few percent apart it dropped most of one pass in
 *   roughly one run in ten, publishing a mean that was neither the passes' nor any
 *   one pass's level.
 * - **The passes are weighted equally.** Each pass is one draw of the process the row
 *   runs in, and the estimate is of that process's level. Weighting by sweep count
 *   instead would make a duration-bound row's mean the HARMONIC mean of its passes —
 *   a faster pass fits more sweeps into the same budget — leaning toward the faster
 *   draw by an amount that grows with the spread.
 * - **One per-pass figure** (the cleaned mean) carries the mean, the spread and the
 *   run's `process_noise`, so the published number, the spread disclosed beside it,
 *   and the noise it is read against are statements about the same quantity.
 *
 * `min`, the dispersion and the percentiles are still pooled (`cleaned_ns`,
 * `timings_ns`): a sweep's distribution over every process the row ran in is what
 * they describe, and a level shift between passes is part of it.
 *
 * @param passes - each pass's timings, in pass order; every pass with at least one cleaned sweep
 */
export const summarize_passes = (passes: ReadonlyArray<PassTimings>): PassSummary => {
	const pass_mean_ns = passes.map((p) => mean_of(p.cleaned_ns));
	const mean_ns = mean_of(pass_mean_ns);
	let drift: number | null = null;
	for (const pass of passes) {
		const d = raw_timing_stats(pass.timings_ns).drift;
		if (d !== null && (drift === null || Math.abs(d) > Math.abs(drift))) drift = d;
	}
	const timings_ns = passes.flatMap((p) => p.timings_ns);
	const fastest = Math.min(...pass_mean_ns);
	return {
		timings_ns,
		cleaned_ns: passes.flatMap((p) => p.cleaned_ns),
		mean_ns,
		pass_mean_ns,
		pass_mean_sd_ns:
			passes.length < 2
				? null
				: Math.sqrt(
						pass_mean_ns.reduce((a, m) => a + (m - mean_ns) ** 2, 0) / (pass_mean_ns.length - 1)
					),
		cv_raw: raw_timing_stats(timings_ns).cv_raw,
		drift,
		pass_spread: fastest > 0 ? Math.max(...pass_mean_ns) / fastest - 1 : 0
	};
};

/** Student's t two-sided 95% critical values, by degrees of freedom (1–10). */
const T_CRITICAL_95 = [12.706, 4.303, 3.182, 2.776, 2.571, 2.447, 2.365, 2.306, 2.262, 2.228];

/**
 * The two-sided 95% critical value of Student's t at `df` degrees of freedom — what a
 * confidence interval over a handful of pass means needs in place of the normal 1.96,
 * which at three passes (df 2) is narrower than the truth by more than half. Past the
 * table, 1.96 plus a first-order correction (about 1% low at df 11, closer above).
 */
export const t_critical_95 = (df: number): number =>
	df < 1 ? NaN : df <= T_CRITICAL_95.length ? T_CRITICAL_95[df - 1] : 1.96 + 2.4 / df;

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
 * deviation — the slower pass mean over the faster, minus one — is noise by
 * construction. The figures are over every such pair of every timed row. They
 * describe ONE process against one process; a published row pools all its passes
 * and is steadier than this, by how much depending on how independent the passes
 * were, so this is the conservative bound: a ratio between two rows that is inside
 * it is not a difference this run measured.
 *
 * @param rows - each timed row's pass means (`PassSummary.pass_mean_ns`)
 * @returns the noise figures, or `null` when no row has two passes to compare
 */
export const summarize_process_noise = (
	rows: ReadonlyArray<ReadonlyArray<number>>
): ProcessNoise | null => {
	const deviations: number[] = [];
	for (const means of rows) {
		for (let i = 0; i < means.length; i++) {
			for (let j = i + 1; j < means.length; j++) {
				const low = Math.min(means[i], means[j]);
				if (low > 0) deviations.push(Math.max(means[i], means[j]) / low - 1);
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
