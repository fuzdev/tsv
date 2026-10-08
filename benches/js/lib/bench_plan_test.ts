/**
 * Tests for the timed run's pass arithmetic (`bench_plan.ts`): the order each pass
 * takes a group's rows in, and what a row's passes say once pooled.
 * Dependency-free, so it gates in `deno task test:deno`.
 */

import assert from 'node:assert';
import {
	DRIFT_MIN_SAMPLES,
	pass_order,
	raw_timing_stats,
	summarize_passes,
	summarize_process_noise,
	t_critical_95,
	warmup_iterations_for
} from './bench_plan.ts';

const ROWS = ['a', 'b', 'c', 'd', 'e', 'f'];

Deno.test('every pass is a permutation of the rows', () => {
	for (const passes of [1, 2, 3, 4, 5, 6]) {
		for (let pass = 0; pass < passes; pass++) {
			assert.deepStrictEqual([...pass_order(ROWS, pass, passes)].sort(), ROWS);
		}
	}
});

Deno.test('pass 0 is the registration order and pass 1 its reverse', () => {
	assert.deepStrictEqual(pass_order(ROWS, 0, 3), ROWS);
	assert.deepStrictEqual(pass_order(ROWS, 1, 3), [...ROWS].reverse());
});

Deno.test('no two passes of a run share an order', () => {
	for (const passes of [2, 3, 4, 5, 6]) {
		const orders = new Set<string>();
		for (let pass = 0; pass < passes; pass++) orders.add(pass_order(ROWS, pass, passes).join(''));
		assert.strictEqual(orders.size, passes, `passes=${passes}`);
	}
});

Deno.test('a later pair starts further round the list', () => {
	// three passes, six rows: the third starts halfway round the forward order
	assert.deepStrictEqual(pass_order(ROWS, 2, 3), ['d', 'e', 'f', 'a', 'b', 'c']);
	// five passes: thirds
	assert.deepStrictEqual(pass_order(ROWS, 2, 5), ['c', 'd', 'e', 'f', 'a', 'b']);
	assert.deepStrictEqual(pass_order(ROWS, 4, 5), ['e', 'f', 'a', 'b', 'c', 'd']);
});

Deno.test('over three passes no row keeps the same predecessor throughout', () => {
	const predecessors = new Map<string, Set<string>>();
	for (let pass = 0; pass < 3; pass++) {
		const order = pass_order(ROWS, pass, 3);
		for (let i = 0; i < order.length; i++) {
			const set = predecessors.get(order[i]) ?? new Set<string>();
			set.add(i === 0 ? '(first)' : order[i - 1]);
			predecessors.set(order[i], set);
		}
	}
	for (const [row, set] of predecessors) assert.ok(set.size > 1, `${row} always follows one row`);
});

Deno.test('a one-row or empty group has the only order there is', () => {
	assert.deepStrictEqual(pass_order(['x'], 2, 3), ['x']);
	assert.deepStrictEqual(pass_order([], 1, 3), []);
});

Deno.test('warmup is the iteration floor or the time floor, whichever is more', () => {
	assert.strictEqual(warmup_iterations_for(25, 3, 5000), 200);
	assert.strictEqual(warmup_iterations_for(13_000, 3, 5000), 3);
	assert.strictEqual(warmup_iterations_for(0, 3, 5000), 3);
});

Deno.test('drift is null below the per-pass floor and reads a level shift above it', () => {
	assert.strictEqual(raw_timing_stats(Array(DRIFT_MIN_SAMPLES - 1).fill(10)).drift, null);
	const stepped = [10, 10, 10, 10, 12, 12, 12, 12];
	const { drift } = raw_timing_stats(stepped);
	assert.ok(drift !== null && Math.abs(drift - 0.2) < 1e-9);
});

/** A pass the cleaner kept whole. */
const kept = (timings_ns: number[]) => ({ timings_ns, cleaned_ns: timings_ns });

Deno.test('a shift BETWEEN passes is pass_spread, never drift', () => {
	// two flat processes at two levels: nothing moved while either was measured
	const summary = summarize_passes([kept(Array(8).fill(100)), kept(Array(8).fill(110))]);
	assert.strictEqual(summary.drift, 0);
	assert.ok(Math.abs(summary.pass_spread - 0.1) < 1e-9);
	assert.deepStrictEqual(summary.pass_mean_ns, [100, 110]);
	assert.strictEqual(summary.timings_ns.length, 16);
	// and the pooled raw cv does see it
	assert.ok(summary.cv_raw !== null && summary.cv_raw > 0.04);
});

Deno.test('the mean weights every pass equally, whatever its sweep count', () => {
	// a duration-bound row: the faster pass fits twice the sweeps into the same budget
	const summary = summarize_passes([kept(Array(20).fill(100)), kept(Array(10).fill(200))]);
	assert.strictEqual(summary.mean_ns, 150);
	// the count-weighted mean would have been the harmonic one
	const pooled = summary.cleaned_ns.reduce((a, b) => a + b, 0) / summary.cleaned_ns.length;
	assert.ok(Math.abs(pooled - 133.33) < 0.01);
});

Deno.test("the mean is over each pass's own cleaned sweeps, not its raw ones", () => {
	const summary = summarize_passes([
		{ timings_ns: [100, 100, 100, 400], cleaned_ns: [100, 100, 100] },
		kept([110, 110, 110, 110])
	]);
	assert.strictEqual(summary.mean_ns, 105);
	assert.deepStrictEqual(summary.cleaned_ns, [100, 100, 100, 110, 110, 110, 110]);
	// the percentiles' set keeps the transient
	assert.strictEqual(summary.timings_ns.length, 8);
});

Deno.test('pass_mean_sd is the Bessel-corrected sd of the pass means, null for one pass', () => {
	const three = summarize_passes([kept([100]), kept([110]), kept([120])]);
	assert.ok(three.pass_mean_sd_ns !== null && Math.abs(three.pass_mean_sd_ns - 10) < 1e-9);
	assert.strictEqual(summarize_passes([kept([100, 101])]).pass_mean_sd_ns, null);
});

Deno.test('drift is the pass furthest from zero, signed', () => {
	const flat = Array(8).fill(100);
	const warming = [100, 100, 100, 100, 90, 90, 90, 90];
	const leaking = [100, 100, 100, 100, 105, 105, 105, 105];
	const { drift } = summarize_passes([kept(flat), kept(warming), kept(leaking)]);
	assert.ok(drift !== null && Math.abs(drift + 0.1) < 1e-9);
});

Deno.test('a single pass has no spread, and short passes no drift', () => {
	const summary = summarize_passes([kept([5, 6, 7])]);
	assert.strictEqual(summary.pass_spread, 0);
	assert.strictEqual(summary.drift, null);
});

Deno.test('process noise is over every pass pair of every row', () => {
	const noise = summarize_process_noise([
		[100, 110, 100],
		[200, 200, 200]
	]);
	assert.ok(noise !== null);
	assert.strictEqual(noise.pairs, 6);
	assert.strictEqual(noise.median, 0);
	assert.ok(Math.abs(noise.max - 0.1) < 1e-9);
	assert.ok(noise.p95 > 0 && noise.p95 <= noise.max);
});

Deno.test('process noise is null when no row has two passes', () => {
	assert.strictEqual(summarize_process_noise([[100], [200]]), null);
	assert.strictEqual(summarize_process_noise([]), null);
});

Deno.test('t critical values: the table, then within about 1% past it', () => {
	assert.strictEqual(t_critical_95(2), 4.303);
	assert.ok(Number.isNaN(t_critical_95(0)));
	for (const [df, t] of [
		[11, 2.201],
		[20, 2.086],
		[60, 2.0]
	]) {
		assert.ok(Math.abs(t_critical_95(df) / t - 1) < 0.011, `df ${df}`);
	}
});
