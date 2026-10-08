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
 * follows a five-second one. So no two passes share an order (in a group of three
 * rows or more), and no row keeps one predecessor throughout — its neighbours, a
 * reversal keeps, so what moves is which side of the row each one runs on:
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

/** How many times its expected wall a row may take — see `row_deadline_ms`. */
export const ROW_DEADLINE_SLACK = 3;

/**
 * The fixed part of a row's deadline, in ms: runtime start-up, the file set's read,
 * and the engine's load and init probes — none of which the pre-flight sweep prices.
 */
export const ROW_DEADLINE_ALLOWANCE_MS = 5 * 60_000;

/**
 * How long one child of the run may take before it is stopped, in ms — a bound on a
 * hang, not a shape on a measurement, so it is generous: `ROW_DEADLINE_SLACK` times
 * the row's expected wall, plus `ROW_DEADLINE_ALLOWANCE_MS` for start-up and engine
 * load.
 *
 * A row's expected wall is its warmup (at least `warmup_ms`, and at least
 * `warmup_iterations` sweeps) plus its timed window (at least `duration_ms`, and at
 * least `min_iterations` sweeps, one more sweep to finish the window it overran),
 * each sweep priced at the row's pre-flight sweep (`sweep_ms`) — a cold one, so an
 * over-estimate, which is the direction a deadline wants.
 */
export const row_deadline_ms = (row: {
	sweep_ms: number;
	warmup_ms: number;
	warmup_iterations: number;
	duration_ms: number;
	min_iterations: number;
}): number => {
	const warmup = Math.max(row.warmup_ms, row.warmup_iterations * row.sweep_ms);
	const timed = Math.max(row.duration_ms, row.min_iterations * row.sweep_ms) + row.sweep_ms;
	return ROW_DEADLINE_SLACK * (warmup + timed) + ROW_DEADLINE_ALLOWANCE_MS;
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

/**
 * The sample standard deviation of `xs` (Bessel-corrected) about `mean` (theirs by
 * default) — `0` below two values, which carry no spread.
 */
export const sample_sd = (xs: ReadonlyArray<number>, mean = mean_of(xs)): number =>
	xs.length < 2 ? 0 : Math.sqrt(xs.reduce((a, x) => a + (x - mean) ** 2, 0) / (xs.length - 1));

/** The median of `xs` — the mean of the two middle values when their count is even. */
export const median_of = (xs: ReadonlyArray<number>): number => {
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
		pass_mean_sd_ns: passes.length < 2 ? null : sample_sd(pass_mean_ns, mean_ns),
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

/**
 * The coefficient of variation at or past which a timed row is UNSTABLE — disclosed
 * rather than published bare, by the per-runtime report (`bench.ts` §Unstable Rows)
 * and the combined one (`compose_reports.ts` `unstable_cells`) alike, from this one
 * definition.
 *
 * Ratios are the reports' product and each divides two means, so an unstable row
 * silently widens every comparison it appears in. 10% is about three times the p90 of
 * the cleaned cv across the reports it was set against — so ordinary variation sits
 * nowhere near it, and a row that trips it is doing something other than varying.
 * Deliberately tighter than `benchmark_baseline_compare`'s 30% noise gate, which
 * answers another question (is a REGRESSION real) on a run a plain `deno task bench`
 * never makes.
 */
// TODO: re-derive the calibration from the pass-pooled reports — a row's cv now
// includes the variation between its processes, which the reports it was set against
// (one shared process) did not.
export const UNSTABLE_CV_THRESHOLD = 0.1;

/**
 * The `|drift|` at or past which a row is unstable regardless of its cv. 5% is well
 * outside stationary variation (a stationary row's two half-medians agree to well
 * under 1% at any sample count the bench reaches) and well inside the regime this
 * exists for: a row that stepped +25–45% mid-measurement published a cleaned cv
 * under 6% at most sample counts.
 */
export const UNSTABLE_DRIFT_THRESHOLD = 0.05;

/**
 * The `pass_spread` at or past which a row is unstable regardless of the rest: its
 * passes — fresh processes of the same thing — sat further apart than this, so its
 * level depends on the process it was drawn in. The same 5% as the drift's, for the
 * same kind of reason: both are a level shift, one inside a process (read from
 * medians) and one between two (read from the passes' cleaned means).
 */
// TODO: calibrate against `process_noise` once full runs under all three runtimes
// exist — the threshold wants to sit well clear of the run's own p95.
export const UNSTABLE_PASS_SPREAD_THRESHOLD = 0.05;

/**
 * Below this many raw timings PER PASS the RAW cv also trips a row — per pass, since
 * the question is about one process's series, and a pooled count would move the line
 * with the pass count. With few samples one deviant sweep is a real share of the
 * measurement, and exactly what the MAD cleaner's keep-closest fallback blends into
 * the mean, so a raw cv past the threshold is the disclosure the cleaned cv withheld.
 * With hundreds the raw cv is dominated by isolated pauses (one 80 ms GC among 600 ×
 * 8 ms sweeps reads 35%) that the cleaner rightly removes; there the drift is the
 * detector.
 */
export const RAW_CV_SAMPLE_CEILING = 30;

/** The stability readings a row carries, each `null` where its report lacks it. */
export interface StabilityReadings {
	cv: number | null;
	cv_raw: number | null;
	drift: number | null;
	pass_spread: number | null;
	/** Raw timings per pass — the pooled count over the pass count (one, before passes). */
	raw_samples_per_pass: number | null;
}

/**
 * The readings that make a row unstable, each as a magnitude — empty for a stable
 * row. Four, any one of which trips it: the cleaned cv (ordinary noise), the RAW cv on
 * a thin pass (a second mode the cleaner deleted), the drift (a cost that moved while
 * a process measured it) and the pass spread (a level that differs between
 * processes). The cleaned cv alone is blind to the last three — a bimodal row can
 * clean to a quiet cv over a mean that is neither mode.
 */
export const unstable_readings = (r: StabilityReadings): number[] => {
	const tripped: number[] = [];
	if (r.cv !== null && r.cv >= UNSTABLE_CV_THRESHOLD) tripped.push(r.cv);
	if (
		r.cv_raw !== null &&
		r.cv_raw >= UNSTABLE_CV_THRESHOLD &&
		r.raw_samples_per_pass !== null &&
		r.raw_samples_per_pass < RAW_CV_SAMPLE_CEILING
	) {
		tripped.push(r.cv_raw);
	}
	if (r.drift !== null && Math.abs(r.drift) >= UNSTABLE_DRIFT_THRESHOLD) {
		tripped.push(Math.abs(r.drift));
	}
	if (r.pass_spread !== null && r.pass_spread >= UNSTABLE_PASS_SPREAD_THRESHOLD) {
		tripped.push(r.pass_spread);
	}
	return tripped;
};
