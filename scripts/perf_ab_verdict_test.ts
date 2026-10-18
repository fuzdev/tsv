import { deepStrictEqual, throws } from 'node:assert';

import {
	ab_row,
	type AbRow,
	build_inputs_of,
	build_recipe_text,
	CACHE_DIR_PATTERN,
	DEFAULT_ROUNDS,
	file_set_mismatch,
	format_instructions,
	format_pct,
	format_us,
	INSTRUCTIONS_LAYOUT_PCT,
	INSTRUCTIONS_REGRESSION_PCT,
	KEEP_TREES,
	lang_presence,
	layout_reading,
	MAX_ROUNDS,
	mad,
	median,
	needs_sign_off,
	noisy_rows,
	NOISY_SPREAD_PCT,
	overall_passes,
	parse_perf_stat_instructions,
	pct_delta,
	render_table,
	row_is_ambiguous,
	row_is_settleable,
	row_verdict,
	SIGN_OFF_EXCESS_PCT,
	sign_off_rows,
	spread_pct,
	trees_to_evict,
	unsettled_rows,
	WALL_REGRESSION_PCT,
	wall_delta_error,
	wants_more_rounds
} from './perf_ab_verdict.ts';

Deno.test('median takes the middle, or the mean of the middle two', () => {
	deepStrictEqual(median([3, 1, 2]), 2);
	deepStrictEqual(median([4, 1, 3, 2]), 2.5);
	deepStrictEqual(median([7]), 7);
	throws(() => median([]));
});

Deno.test('spread_pct is the median absolute deviation over the median', () => {
	deepStrictEqual(spread_pct([99, 100, 101]), 1);
	deepStrictEqual(spread_pct([100]), 0);
	// one outlier round does not widen it, and more rounds of the same noise do not either
	deepStrictEqual(spread_pct([99, 100, 101, 100, 130]), 1);
	deepStrictEqual(spread_pct([99, 101, 99, 101, 99, 101, 99, 101, 99, 101]), 1);
	deepStrictEqual(mad([1, 2, 4]), 1);
});

Deno.test('pct_delta is relative to the base', () => {
	deepStrictEqual(pct_delta(200, 203), 1.5);
	deepStrictEqual(pct_delta(200, 190), -5);
	deepStrictEqual(pct_delta(0, 0), 0);
});

Deno.test('the thresholds are the ruled ones', () => {
	deepStrictEqual(WALL_REGRESSION_PCT, 1.5);
	deepStrictEqual(INSTRUCTIONS_LAYOUT_PCT, 0.5);
	deepStrictEqual(INSTRUCTIONS_REGRESSION_PCT, 1.5);
	deepStrictEqual(NOISY_SPREAD_PCT, 0.75);
	deepStrictEqual(SIGN_OFF_EXCESS_PCT, 1.5);
});

Deno.test('the rounds policy is balanced', () => {
	deepStrictEqual(DEFAULT_ROUNDS % 2, 0);
	deepStrictEqual(MAX_ROUNDS % 2, 0);
	deepStrictEqual(MAX_ROUNDS > DEFAULT_ROUNDS, true);
});

Deno.test('row_verdict: wall within the threshold passes', () => {
	deepStrictEqual(row_verdict(1.5, 0), 'pass');
	deepStrictEqual(row_verdict(-3, -2), 'pass');
	deepStrictEqual(row_verdict(1.4, 0.49), 'pass');
});

Deno.test('row_verdict: wall over the threshold fails unless instructions are flat', () => {
	deepStrictEqual(row_verdict(1.51, 0.49), 'layout');
	deepStrictEqual(row_verdict(3.3, 0), 'layout');
	deepStrictEqual(row_verdict(3.3, -0.49), 'layout');
	deepStrictEqual(row_verdict(2.1, 0.75), 'fail');
	deepStrictEqual(row_verdict(2.1, 0.5), 'fail');
	// fewer instructions do not excuse a slower wall either: the escape is for FLAT counts
	deepStrictEqual(row_verdict(2.1, -0.5), 'fail');
});

Deno.test('row_verdict: instructions rising under a passing wall warn', () => {
	deepStrictEqual(row_verdict(0.2, 0.5), 'warn');
	deepStrictEqual(row_verdict(-1, 0.76), 'warn');
	// the instruction gate is strict: exactly on it still warns
	deepStrictEqual(row_verdict(1.5, 1.5), 'warn');
	deepStrictEqual(row_verdict(-1, 1.5), 'warn');
});

Deno.test('row_verdict: instructions over their own threshold fail whatever the wall', () => {
	deepStrictEqual(row_verdict(0, 1.501), 'fail');
	deepStrictEqual(row_verdict(-3, 1.501), 'fail');
	deepStrictEqual(row_verdict(1.1, 2.94), 'fail');
	deepStrictEqual(row_verdict(5, 12), 'fail');
	// only a rise fails: fallen instructions under a passing wall pass
	deepStrictEqual(row_verdict(0, -1.501), 'pass');
	deepStrictEqual(row_verdict(-1, -12), 'pass');
});

Deno.test('row_verdict: without instruction counts the wall alone decides', () => {
	deepStrictEqual(row_verdict(1.6, null), 'fail');
	deepStrictEqual(row_verdict(1.4, null), 'pass');
	// no counts, no instruction gate: a passing wall passes however the code changed
	deepStrictEqual(row_verdict(1.5, null), 'pass');
	deepStrictEqual(row_verdict(-5, null), 'pass');
});

Deno.test('row_verdict: a non-finite delta is a harness bug, never a verdict', () => {
	throws(() => row_verdict(NaN, 0), RangeError);
	throws(() => row_verdict(Infinity, 0), RangeError);
	throws(() => row_verdict(0, NaN), RangeError);
	throws(() => row_verdict(0, -Infinity), RangeError);
});

Deno.test('ab_row grades the median of the per-round paired deltas', () => {
	const row = ab_row(
		'svelte',
		'parse',
		{ wall_us: [100, 110, 99], instructions: [1000, 1000, 1001] },
		{ wall_us: [103, 113.3, 102.96], instructions: [1001, 1002, 1001] }
	);
	deepStrictEqual(row.rounds, 3);
	deepStrictEqual(row.base_wall_us, 100);
	deepStrictEqual(row.cand_wall_us, 103);
	// per-round deltas +3, +3, +4: the median pairs each round's two runs
	deepStrictEqual(Math.round(row.wall_pct * 1e9) / 1e9, 3);
	deepStrictEqual(Math.round(row.wall_pct_spread * 1e9) / 1e9, 0);
	deepStrictEqual(row.instructions_pct, 0.1);
	deepStrictEqual(row.verdict, 'layout');
	deepStrictEqual(row.base_wall_spread_pct, 1);

	const uncounted = ab_row(
		'ts',
		'format',
		{ wall_us: [100], instructions: [] },
		{ wall_us: [102], instructions: [] }
	);
	deepStrictEqual(uncounted.instructions_pct, null);
	deepStrictEqual(uncounted.verdict, 'fail');
});

Deno.test('ab_row: drift shared by a round cancels in the paired delta', () => {
	// the base drifts 10% slower over the run; the candidate is 1% slower in every round
	const base = [100, 105, 110, 104];
	const row = ab_row(
		'ts',
		'parse',
		{ wall_us: base, instructions: [] },
		{ wall_us: base.map((b) => b * 1.01), instructions: [] }
	);
	deepStrictEqual(Math.round(row.wall_pct * 1e9) / 1e9, 1);
	deepStrictEqual(Math.round(row.wall_pct_spread * 1e9) / 1e9, 0);
});

Deno.test('ab_row refuses an empty, zero, mismatched or non-finite side', () => {
	const ok = { wall_us: [100, 100], instructions: [] };
	throws(() =>
		ab_row('css', 'parse', { wall_us: [], instructions: [] }, { wall_us: [], instructions: [] })
	);
	throws(() =>
		ab_row(
			'css',
			'parse',
			{ wall_us: [0, 0], instructions: [] },
			{ wall_us: [0, 0], instructions: [] }
		)
	);
	throws(() => ab_row('css', 'parse', ok, { wall_us: [100], instructions: [] }));
	throws(() => ab_row('css', 'parse', ok, { wall_us: [100, NaN], instructions: [] }));
	throws(() =>
		ab_row(
			'css',
			'parse',
			{ wall_us: [100, 100], instructions: [0] },
			{ wall_us: [100, 100], instructions: [5] }
		)
	);
});

Deno.test('ab_row refuses a non-positive or non-finite instruction sample', () => {
	const wall = { wall_us: [100, 100] };
	const counted = (instructions: Array<number>) => ({ ...wall, instructions });
	// a negative base would otherwise flip the delta's sign: a rise graded as a fall
	throws(
		() => ab_row('css', 'parse', counted([-10, -10]), counted([-5, -5])),
		/instruction sample of -10/
	);
	throws(() => ab_row('css', 'parse', counted([100, 100]), counted([100, -1])), RangeError);
	throws(() => ab_row('css', 'parse', counted([100, 0]), counted([100, 100])), RangeError);
	throws(() => ab_row('css', 'parse', counted([100, NaN]), counted([100, 100])), RangeError);
	throws(() => ab_row('css', 'parse', counted([100, 100]), counted([Infinity, 100])), RangeError);
	// positive samples, and none at all (wall-only), grade
	deepStrictEqual(ab_row('css', 'parse', counted([100, 100]), counted([100, 100])).verdict, 'pass');
	deepStrictEqual(ab_row('css', 'parse', counted([]), counted([])).instructions_pct, null);
});

Deno.test(
	'needs_sign_off: a wall FAIL with instructions in the WARN band, a layout move apart',
	() => {
		const sensitive = (wall: number, instr: number | null) =>
			needs_sign_off(row_verdict(wall, instr), wall, instr);
		// the band is inclusive at both ends, as the WARN band is
		deepStrictEqual(sensitive(1.6, 0.5), true);
		deepStrictEqual(sensitive(1.6, 0.753), true);
		deepStrictEqual(sensitive(2.5, 1.5), true);
		// under the wall line it is a WARN, not a failure
		deepStrictEqual(sensitive(1.5, 0.753), false);
		// flat instructions: LAYOUT, a pass
		deepStrictEqual(sensitive(1.6, 0.49), false);
		// over the instruction gate the count decides, whatever the sitting
		deepStrictEqual(sensitive(1.6, 1.501), false);
		deepStrictEqual(sensitive(0, 2.94), false);
		// fallen instructions and no counts are plain wall failures, outside the band
		deepStrictEqual(sensitive(1.6, -0.5), false);
		deepStrictEqual(sensitive(1.6, null), false);
		// the wall may exceed the instructions by a layout move, inclusive, and no more: past it the
		// wall carries cost the count does not, a plain failure
		deepStrictEqual(sensitive(0.5 + SIGN_OFF_EXCESS_PCT, 0.5), true);
		deepStrictEqual(sensitive(0.501 + SIGN_OFF_EXCESS_PCT, 0.5), false);
		deepStrictEqual(sensitive(6, 0.8), false);
	}
);

Deno.test('a sign-off row still fails the run, and says so', () => {
	const row = ab_row(
		'svelte',
		'parse',
		{ wall_us: [100, 100], instructions: [1000, 1000] },
		{ wall_us: [101.6, 101.6], instructions: [1007.5, 1007.5] }
	);
	deepStrictEqual(row.verdict, 'fail');
	deepStrictEqual(row.sign_off, true);
	deepStrictEqual(overall_passes([row]), false);
	deepStrictEqual(sign_off_rows([row, row_with(0, 0.1, 0)]), [row]);
	deepStrictEqual(render_table([row]).includes('FAIL (sign-off)'), true);
	const plain = ab_row(
		'svelte',
		'parse',
		{ wall_us: [100, 100], instructions: [1000, 1000] },
		{ wall_us: [101.6, 101.6], instructions: [1000, 1000] }
	);
	deepStrictEqual(plain.sign_off, false);
});

Deno.test('layout_reading takes the widest wall move among flat-instruction rows', () => {
	const flat_slow = row_with(-1.9, 0.5, 0);
	const flat_fast = row_with(1.2, 0.5, 0.3);
	const risen = row_with(4, 0.5, 0.8);
	deepStrictEqual(layout_reading([flat_fast, risen, flat_slow]), flat_slow);
	// a row with risen, fallen or no counts reads work, not layout
	deepStrictEqual(layout_reading([risen, row_with(3, 0.5, -0.6), row_with(3, 0.5, null)]), null);
	deepStrictEqual(layout_reading([]), null);
});

Deno.test('overall_passes fails on any failing row and only then', () => {
	const row = (wall: number, instr: number) =>
		ab_row(
			'css',
			'parse',
			{ wall_us: [100], instructions: [100] },
			{
				wall_us: [100 + wall],
				instructions: [100 + instr]
			}
		);
	// pass, LAYOUT and WARN all pass
	deepStrictEqual(overall_passes([row(0, 0), row(3, 0), row(0, 1)]), true);
	deepStrictEqual(overall_passes([row(0, 0), row(3, 1)]), false);
	// instructions alone fail the run under a flat wall
	deepStrictEqual(overall_passes([row(0, 0), row(0, 2)]), false);
});

Deno.test('parse_perf_stat_instructions reads the CSV value', () => {
	const csv = '# started on Thu Oct 16 2026\n\n14108901234,,instructions:u,5012345678,100.00,,\n';
	deepStrictEqual(parse_perf_stat_instructions(csv), 14108901234);
	deepStrictEqual(parse_perf_stat_instructions('<not counted>,,instructions:u,0,0.00,,'), null);
	deepStrictEqual(parse_perf_stat_instructions('<not supported>,,instructions:u,0,100.00,,'), null);
	deepStrictEqual(parse_perf_stat_instructions('123,,cycles:u,1,100.00,,'), null);
	deepStrictEqual(parse_perf_stat_instructions(''), null);
});

Deno.test('formatters', () => {
	deepStrictEqual(format_us(127_600), '127.60ms');
	deepStrictEqual(format_us(817.4), '817us');
	deepStrictEqual(format_instructions(1_410_690_000), '1410.69M');
	deepStrictEqual(format_instructions(12_457_000), '12.457M');
	deepStrictEqual(format_pct(1.5), '+1.500%');
	deepStrictEqual(format_pct(-0.25), '-0.250%');
	// the deciding digit shows: 1.504 fails, 0.497 is flat
	deepStrictEqual(format_pct(1.504), '+1.504%');
	deepStrictEqual(format_pct(0.497), '+0.497%');
});

Deno.test('render_table prints one aligned row per entry', () => {
	const table = render_table([
		ab_row(
			'ts',
			'parse',
			{ wall_us: [1000], instructions: [1e9] },
			{
				wall_us: [1020],
				instructions: [1e9]
			}
		),
		ab_row(
			'css',
			'format',
			{ wall_us: [900], instructions: [] },
			{
				wall_us: [900],
				instructions: []
			}
		)
	]);
	const lines = table.split('\n');
	deepStrictEqual(lines.length, 4);
	deepStrictEqual(new Set(lines.map((line) => line.length)).size, 1);
	deepStrictEqual(lines[2]!.includes('TS parse') && lines[2]!.includes('LAYOUT (pass)'), true);
	deepStrictEqual(lines[3]!.includes('CSS format') && lines[3]!.includes('—'), true);
});

const row_with = (
	wall_pct: number,
	wall_pct_spread: number,
	instructions_pct: number | null,
	rounds = DEFAULT_ROUNDS
): AbRow => ({
	lang: 'svelte',
	phase: 'parse',
	rounds,
	base_wall_us: 100,
	cand_wall_us: 100 + wall_pct,
	base_wall_spread_pct: 0.3,
	cand_wall_spread_pct: 0.3,
	wall_pct,
	wall_pct_spread,
	base_instructions: instructions_pct === null ? null : 100,
	cand_instructions: instructions_pct === null ? null : 100 + instructions_pct,
	instructions_pct,
	verdict: row_verdict(wall_pct, instructions_pct),
	sign_off: needs_sign_off(row_verdict(wall_pct, instructions_pct), wall_pct, instructions_pct)
});

Deno.test('wall_delta_error is the median error at the spread, shrinking with √rounds', () => {
	// 1.4826 × 1.2533 × 0.5 / √6
	deepStrictEqual(wall_delta_error(row_with(1.6, 0.5, 0.8)).toFixed(4), '0.3793');
	deepStrictEqual(
		wall_delta_error(row_with(1.6, 0.5, 0.8), 24).toFixed(4),
		(wall_delta_error(row_with(1.6, 0.5, 0.8)) / 2).toFixed(4)
	);
	deepStrictEqual(wall_delta_error(row_with(1.6, 0, 0.8)), 0);
});

Deno.test('row_is_ambiguous: within its error of the line, and the line decides', () => {
	// the error, not the spread: 0.4 from the line at a 0.5 spread is outside 0.379
	deepStrictEqual(row_is_ambiguous(row_with(1.1, 0.5, 0.76)), false);
	deepStrictEqual(row_is_ambiguous(row_with(1.2, 0.5, 0.76)), true);
	// risen instructions: crossing the line flips WARN to FAIL
	deepStrictEqual(row_is_ambiguous(row_with(1.3, 0.3, 0.76)), true);
	deepStrictEqual(row_is_ambiguous(row_with(1.7, 0.3, 0.76)), true);
	deepStrictEqual(row_is_ambiguous(row_with(1.0, 0.3, 0.76)), false);
	deepStrictEqual(row_is_ambiguous(row_with(2.5, 0.3, 0.76)), false);
	// no counts: pass vs FAIL
	deepStrictEqual(row_is_ambiguous(row_with(1.4, 0.3, null)), true);
	// fallen instructions do not excuse a slower wall either, so they flip too
	deepStrictEqual(row_is_ambiguous(row_with(1.4, 0.3, -0.8)), true);
	deepStrictEqual(row_is_ambiguous(row_with(1.4, 0.3, -2)), true);
	// exactly on the instruction gate still warns, so the wall line still decides
	deepStrictEqual(row_is_ambiguous(row_with(1.4, 0.3, 1.5)), true);
	// over the instruction gate the row fails on both sides of the wall line: the count is
	// deterministic, so more rounds cannot change the verdict
	deepStrictEqual(row_is_ambiguous(row_with(1.4, 0.3, 1.501)), false);
	deepStrictEqual(row_is_ambiguous(row_with(1.6, 0.3, 2.94)), false);
	// flat instructions: pass vs LAYOUT, both passing — no need to measure on
	deepStrictEqual(row_is_ambiguous(row_with(1.4, 0.3, 0.1)), false);
	// a zero spread (identical rounds) never extends
	deepStrictEqual(row_is_ambiguous(row_with(1.4, 0, 0.76)), false);
});

Deno.test('row_is_settleable: the cap could narrow the error under the distance', () => {
	// at a 0.3 spread the error is 0.228 at 6 rounds and 0.176 at the cap of 10
	deepStrictEqual(row_is_settleable(row_with(1.3, 0.3, 0.76)), true);
	// the Svelte-parse shape this rule came from: 0.11 from the line at a 0.525 spread (error
	// 0.398 now, 0.308 at the cap) — more rounds spend the time and do not decide it
	deepStrictEqual(row_is_ambiguous(row_with(1.61, 0.525, 0.772)), true);
	deepStrictEqual(row_is_settleable(row_with(1.61, 0.525, 0.772)), false);
	deepStrictEqual(unsettled_rows([row_with(1.61, 0.525, 0.772)]).length, 1);
	// at the cap nothing is left to settle with
	deepStrictEqual(row_is_settleable(row_with(1.45, 0.3, 0.76, MAX_ROUNDS)), false);
});

Deno.test('wants_more_rounds extends in pairs up to the cap', () => {
	const ambiguous = [row_with(0, 0.1, 0), row_with(1.3, 0.3, 0.76)];
	deepStrictEqual(wants_more_rounds(ambiguous, DEFAULT_ROUNDS), true);
	deepStrictEqual(wants_more_rounds(ambiguous, MAX_ROUNDS - 2), true);
	deepStrictEqual(wants_more_rounds(ambiguous, MAX_ROUNDS - 1), false);
	deepStrictEqual(wants_more_rounds(ambiguous, MAX_ROUNDS), false);
	deepStrictEqual(wants_more_rounds([row_with(0, 0.1, 0)], DEFAULT_ROUNDS), false);
	// an ambiguous row the cap cannot settle does not extend the run
	deepStrictEqual(wants_more_rounds([row_with(1.61, 0.525, 0.772)], DEFAULT_ROUNDS), false);
});

Deno.test('noisy_rows names a row with either side over the spread line', () => {
	const quiet = row_with(0, 0.1, 0);
	const noisy = { ...row_with(0, 0.1, 0), cand_wall_spread_pct: 0.8 };
	deepStrictEqual(noisy_rows([quiet, noisy]), [noisy]);
	deepStrictEqual(noisy_rows([{ ...quiet, base_wall_spread_pct: 0.75 }]), []);
});

Deno.test('lang_presence: absent on both is left out, one-sided is an error', () => {
	deepStrictEqual(lang_presence(10, 10), 'measured');
	deepStrictEqual(lang_presence(10, 9), 'measured');
	deepStrictEqual(lang_presence(0, 0), 'absent');
	deepStrictEqual(lang_presence(0, 10), 'one_sided');
	deepStrictEqual(lang_presence(10, 0), 'one_sided');
});

Deno.test('file_set_mismatch compares counts, bytes and parse errors', () => {
	const set = { files: 3, bytes: 900, errors: 0 };
	deepStrictEqual(file_set_mismatch(set, { ...set }), null);
	deepStrictEqual(file_set_mismatch(set, { ...set, bytes: 901 }) !== null, true);
	deepStrictEqual(file_set_mismatch(set, { ...set, files: 4 }) !== null, true);
	deepStrictEqual(file_set_mismatch(set, { ...set, errors: 1 }) !== null, true);
});

Deno.test('build_inputs_of reads the inputs, the missing ones and the key text', () => {
	const driver = 'crates/tsv_debug/examples/perf_ab.rs';
	const lines = [
		'100644 blob 1111111111111111111111111111111111111111\t.cargo/config.toml',
		'100644 blob 2222222222222222222222222222222222222222\tCargo.lock',
		'100644 blob 3333333333333333333333333333333333333333\tCargo.toml',
		'100644 blob 4444444444444444444444444444444444444444\tcrates/tsv_ts/src/lib.rs',
		`100644 blob 5555555555555555555555555555555555555555\t${driver}`,
		'100644 blob 6666666666666666666666666666666666666666\tcrates/tsv_ts/Cargo.toml'
	];
	const required = ['Cargo.toml', 'crates', 'src'];
	const read = build_inputs_of(lines.join('\0'), required, driver);
	deepStrictEqual(read.names, ['.cargo', 'Cargo.lock', 'Cargo.toml', 'crates']);
	deepStrictEqual(read.missing, ['src']);
	// the overlaid driver is left out of the key: a tree without it, or with another copy of
	// it, keys the same
	deepStrictEqual(read.key_text.includes(driver), false);
	const without = lines.filter((line) => !line.endsWith(driver));
	deepStrictEqual(build_inputs_of(without.join('\0'), required, driver).key_text, read.key_text);
	const edited = lines.map((line) =>
		line.endsWith(driver) ? `100644 blob ${'7'.repeat(40)}\t${driver}` : line
	);
	deepStrictEqual(build_inputs_of(edited.join('\0'), required, driver).key_text, read.key_text);
	// any other input moving moves the key
	const moved = lines.map((line) => line.replace('4444', '4445'));
	deepStrictEqual(
		build_inputs_of(moved.join('\0'), required, driver).key_text === read.key_text,
		false
	);
	deepStrictEqual(build_inputs_of('', required, driver).missing, required);
	// `-z` ends every entry with a NUL, the last one included, and leaves a path raw
	const raw = `100644 blob ${'8'.repeat(40)}\tcrates/\u00e9/a\nb.rs`;
	const ended = build_inputs_of(`${lines.join('\0')}\0${raw}\0`, required, driver);
	deepStrictEqual(ended.names, ['.cargo', 'Cargo.lock', 'Cargo.toml', 'crates']);
	deepStrictEqual(ended.key_text.endsWith(raw), true);
});

Deno.test('build_recipe_text moves with the command and the set and kept variables', () => {
	const command = ['cargo', 'build', '--release'];
	const fixed = { CARGO_TARGET_DIR: '../target' };
	const env = ['PATH', 'HOME'];
	const text = build_recipe_text(command, fixed, env);
	deepStrictEqual(build_recipe_text([...command], { ...fixed }, [...env]), text);
	deepStrictEqual(build_recipe_text([...command, '--locked'], fixed, env) === text, false);
	deepStrictEqual(
		build_recipe_text(['cargo', 'build', '--profile', 'corpus'], fixed, env) === text,
		false
	);
	deepStrictEqual(build_recipe_text(command, fixed, [...env, 'RUSTFLAGS']) === text, false);
	// a variable the build sets moves it by name and by value; the order it is written in does not
	deepStrictEqual(
		build_recipe_text(command, { ...fixed, CARGO_PROFILE_RELEASE_LTO: 'fat' }, env) === text,
		false
	);
	deepStrictEqual(build_recipe_text(command, { CARGO_TARGET_DIR: 'target' }, env) === text, false);
	deepStrictEqual(
		build_recipe_text(command, { B: '1', A: '2' }, env),
		build_recipe_text(command, { A: '2', B: '1' }, env)
	);
	// an argument boundary is part of the recipe: `a b` as one argument is not `a`, `b`
	deepStrictEqual(
		build_recipe_text(['a b'], fixed, env) === build_recipe_text(['a', 'b'], fixed, env),
		false
	);
});

Deno.test('CACHE_DIR_PATTERN takes a build-inputs key and a legacy tree id, nothing else', () => {
	deepStrictEqual(CACHE_DIR_PATTERN.test('a'.repeat(64)), true);
	// a snapshot keyed by the whole tree id: never reused, aged out by eviction like any other
	deepStrictEqual(CACHE_DIR_PATTERN.test('b'.repeat(40)), true);
	deepStrictEqual(CACHE_DIR_PATTERN.test('a'.repeat(63)), false);
	deepStrictEqual(CACHE_DIR_PATTERN.test(`${'a'.repeat(64)}.partial.12`), false);
	deepStrictEqual(CACHE_DIR_PATTERN.test('index.12'), false);
});

Deno.test('trees_to_evict keeps the trees in use plus the most recently used', () => {
	deepStrictEqual(KEEP_TREES, 6);
	const trees = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'].map((name, i) => ({ name, mtime_ms: i }));
	// in use: a (oldest) and h; keep the four newest others, g f e d
	deepStrictEqual(trees_to_evict(trees, new Set(['a', 'h'])).sort(), ['b', 'c']);
	deepStrictEqual(trees_to_evict(trees.slice(0, 6), new Set(['a'])), []);
	deepStrictEqual(
		trees_to_evict(trees, new Set(['s', 't', 'u', 'v', 'w', 'x', 'y'])).length,
		trees.length
	);
});
