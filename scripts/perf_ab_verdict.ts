/**
 * The pure half of `deno task perf:ab` (`scripts/perf_ab.ts`): round statistics, the per-row
 * verdict, the rounds policy, the file-set and cache-eviction decisions, `perf stat` output
 * parsing, and the table. No I/O, so every rule is unit-tested (`perf_ab_verdict_test.ts`).
 *
 * The rule, per language × phase row: a candidate whose wall-clock exceeds the base's by more
 * than `WALL_REGRESSION_PCT` FAILS, unless its retired user instructions (`instructions:u`)
 * moved by less than `INSTRUCTIONS_LAYOUT_PCT` either way — then the wall move is read as code
 * layout (a build's code placement alone moves wall by about as much as the wall threshold;
 * `docs/performance.md` §Reading `cycles:u`) and the row passes as LAYOUT. Instructions that
 * rose by `INSTRUCTIONS_LAYOUT_PCT` or more under a passing wall are real added work the wall
 * did not show: a WARN, not a failure — up to `INSTRUCTIONS_REGRESSION_PCT`. Instructions that
 * rose by more than that FAIL the row on their own, whatever the wall says: the count is
 * deterministic — it repeats from run to run where the wall wanders — so a rise that large is
 * added work, not noise, and a wall that happened to stay under its threshold does not excuse
 * it.
 * Without instruction counts there is no layout escape and no instruction gate, so the wall
 * alone decides.
 *
 * A row that FAILS on its wall while its instructions sit inside the WARN band, and whose wall
 * exceeds what its instructions explain by no more than a layout move (`SIGN_OFF_EXCESS_PCT`),
 * is still a failure, but a SIGN-OFF one (`needs_sign_off`): the count says the change added a
 * little work, and whether the wall carries it over the line moves between sittings by more
 * than the in-run spread measures — the same two binaries read differently an hour apart. A
 * re-run re-rolls only the wall, so it cannot clear the row; a person accepts the instruction
 * rise as the cost or fixes it (`docs/performance.md` §Gating a change against a base).
 *
 * The wall delta is PAIRED: each round measures both sides back to back, so the row's delta is
 * the median of the per-round candidate/base ratios, not a ratio of two medians — drift that
 * moves both runs of a round (a background job, the machine warming) cancels inside the round.
 *
 * @module
 */

/** A candidate wall more than this many percent above the base's fails its row. */
export const WALL_REGRESSION_PCT = 1.5;

/** An instruction delta under this many percent (either way) reads a wall move as layout; a
 * rise at or above it under a passing wall is reported as a warning. */
export const INSTRUCTIONS_LAYOUT_PCT = 0.5;

/** Instructions that rose by more than this many percent fail their row on their own,
 * whatever the wall delta. Equal to `WALL_REGRESSION_PCT` in value but its own rule: the wall
 * threshold sits on run-to-run noise, this one on a deterministic count. */
export const INSTRUCTIONS_REGRESSION_PCT = 1.5;

/** How far, in points, a WARN-band row's wall delta may exceed its instruction delta and still
 * read as a sign-off rather than a plain failure: the wall move code placement alone gives a
 * build, about the wall threshold (`docs/performance.md` §Reading `cycles:u`). Past it the wall
 * carries cost the instructions do not count — cache or branch misses — which a person should
 * not wave through as layout. Equal to `WALL_REGRESSION_PCT` in value, its own rule. */
export const SIGN_OFF_EXCESS_PCT = 1.5;

/** The standard error of a median of normal samples, per unit of their median absolute
 * deviation: the MAD reads σ at ×1.4826, and a median's error is ×1.2533 a mean's (σ/√n). */
const MEDIAN_ERROR_PER_MAD = 1.4826 * 1.2533;

/** A side whose round spread (`spread_pct`) exceeds this many percent warns: the machine was
 * too noisy for the wall threshold to mean much. Half the wall threshold. */
export const NOISY_SPREAD_PCT = WALL_REGRESSION_PCT / 2;

/** Rounds run by default. Even, so each side goes first equally often. */
export const DEFAULT_ROUNDS = 6;

/** The most rounds an ambiguous row extends a run to (`wants_more_rounds`); extra rounds come
 * in balanced pairs. */
export const MAX_ROUNDS = 10;

/** How many cached snapshot trees a run keeps, the ones it measures included; older ones are
 * evicted when a run starts. Room for a working-tree A/A, a release check and a throwaway pair
 * without evicting HEAD's tree. */
export const KEEP_TREES = 6;

export const LANGS = ['ts', 'svelte', 'css'] as const;
export type Lang = (typeof LANGS)[number];

export const PHASES = ['parse', 'format'] as const;
export type Phase = (typeof PHASES)[number];

const LANG_LABELS: Record<Lang, string> = { ts: 'TS', svelte: 'Svelte', css: 'CSS' };

/** `pass` = within the wall threshold; `layout` = over it at flat instructions (passes);
 * `warn` = within it but instructions rose, by no more than `INSTRUCTIONS_REGRESSION_PCT`
 * (passes); `fail` = a regression, by wall or by instructions. */
export type RowVerdict = 'pass' | 'layout' | 'warn' | 'fail';

/** The median of `values` (the mean of the middle two for an even count). Throws on none —
 * a statistic over zero rounds is a harness bug, never a zero. */
export function median(values: ReadonlyArray<number>): number {
	if (values.length === 0) throw new Error('median of no values');
	const sorted = [...values].sort((a, b) => a - b);
	const mid = sorted.length >> 1;
	return sorted.length % 2 === 1 ? sorted[mid]! : (sorted[mid - 1]! + sorted[mid]!) / 2;
}

/** The median absolute deviation of `values` from their median, in their own unit. Zero for
 * fewer than two values. A round count's worth of samples does not widen it, as a range would:
 * the range of ten rounds reads wider than the range of six on the same machine. */
export function mad(values: ReadonlyArray<number>): number {
	if (values.length < 2) return 0;
	const mid = median(values);
	return median(values.map((v) => Math.abs(v - mid)));
}

/** `mad` as a percent of the median — the round-to-round spread printed beside each side's
 * median. Zero for fewer than two values. */
export function spread_pct(values: ReadonlyArray<number>): number {
	if (values.length < 2) return 0;
	const mid = median(values);
	if (mid === 0) return 0;
	return (mad(values) / mid) * 100;
}

/** `cand` relative to `base`, in percent; non-finite when `base` is zero and `cand` is not. */
export function pct_delta(base: number, cand: number): number {
	if (base === 0) return cand === 0 ? 0 : Infinity;
	return ((cand - base) / base) * 100;
}

/** The verdict for one row; `instructions_pct` is null when no counts were taken. Throws on a
 * non-finite delta: that is a harness bug (an empty or zero side), never a verdict. */
export function row_verdict(wall_pct: number, instructions_pct: number | null): RowVerdict {
	if (!Number.isFinite(wall_pct)) throw new RangeError(`non-finite wall delta: ${wall_pct}`);
	if (instructions_pct !== null && !Number.isFinite(instructions_pct)) {
		throw new RangeError(`non-finite instruction delta: ${instructions_pct}`);
	}
	if (instructions_pct !== null && instructions_pct > INSTRUCTIONS_REGRESSION_PCT) return 'fail';
	if (wall_pct > WALL_REGRESSION_PCT) {
		return instructions_pct !== null && Math.abs(instructions_pct) < INSTRUCTIONS_LAYOUT_PCT
			? 'layout'
			: 'fail';
	}
	return instructions_pct !== null && instructions_pct >= INSTRUCTIONS_LAYOUT_PCT ? 'warn' : 'pass';
}

/** One side's per-round samples for a row, index-aligned with the other side's (round `r` of
 * both sides ran back to back). */
export interface RowSamples {
	/** Wall-clock µs per round: the per-file medians summed over the language. */
	wall_us: Array<number>;
	/** Retired user instructions per iteration per round, fixed cost removed; empty when no
	 * counts were taken. */
	instructions: Array<number>;
}

export interface AbRow {
	lang: Lang;
	phase: Phase;
	rounds: number;
	base_wall_us: number;
	cand_wall_us: number;
	base_wall_spread_pct: number;
	cand_wall_spread_pct: number;
	/** The median of the per-round paired deltas. */
	wall_pct: number;
	/** The median absolute deviation of the per-round paired deltas, in percentage points. */
	wall_pct_spread: number;
	base_instructions: number | null;
	cand_instructions: number | null;
	instructions_pct: number | null;
	verdict: RowVerdict;
	/** A wall FAIL that a person signs off rather than re-runs (`needs_sign_off`). */
	sign_off: boolean;
}

/** Fold one row's samples into its medians, its paired delta and its verdict. Throws when the
 * sides' round counts differ or a sample — wall or instruction — is not a positive finite
 * number: an instruction sample is a difference of two process totals, so a non-positive one
 * means the subtraction went wrong, and a negative base would flip the delta's sign. */
export function ab_row(lang: Lang, phase: Phase, base: RowSamples, cand: RowSamples): AbRow {
	const rounds = base.wall_us.length;
	if (rounds === 0 || cand.wall_us.length !== rounds) {
		throw new Error(`${lang} ${phase}: ${rounds} base rounds against ${cand.wall_us.length} cand`);
	}
	for (const value of [...base.wall_us, ...cand.wall_us]) {
		if (!(Number.isFinite(value) && value > 0)) {
			throw new RangeError(`${lang} ${phase}: a wall sample of ${value} µs`);
		}
	}
	for (const value of [...base.instructions, ...cand.instructions]) {
		if (!(Number.isFinite(value) && value > 0)) {
			throw new RangeError(`${lang} ${phase}: an instruction sample of ${value} per iteration`);
		}
	}
	const deltas = base.wall_us.map((b, r) => pct_delta(b, cand.wall_us[r]!));
	const wall_pct = median(deltas);
	const counted = base.instructions.length > 0 && cand.instructions.length > 0;
	const base_instructions = counted ? median(base.instructions) : null;
	const cand_instructions = counted ? median(cand.instructions) : null;
	const instructions_pct =
		base_instructions !== null && cand_instructions !== null
			? pct_delta(base_instructions, cand_instructions)
			: null;
	const verdict = row_verdict(wall_pct, instructions_pct);
	return {
		lang,
		phase,
		rounds,
		base_wall_us: median(base.wall_us),
		cand_wall_us: median(cand.wall_us),
		base_wall_spread_pct: spread_pct(base.wall_us),
		cand_wall_spread_pct: spread_pct(cand.wall_us),
		wall_pct,
		wall_pct_spread: mad(deltas),
		base_instructions,
		cand_instructions,
		instructions_pct,
		verdict,
		sign_off: needs_sign_off(verdict, wall_pct, instructions_pct)
	};
}

/** Whether a verdict is a wall FAIL with instructions inside the WARN band and a wall no more
 * than `SIGN_OFF_EXCESS_PCT` beyond them — the one failure the instruction column neither
 * excuses (as LAYOUT) nor decides on its own. Such a row still fails, but its wall delta near
 * the line is a reading of the sitting as much as of the change, while its instruction delta is
 * the change's alone: the cost to accept or fix is the count, and no re-run moves it. */
export function needs_sign_off(
	verdict: RowVerdict,
	wall_pct: number,
	instructions_pct: number | null
): boolean {
	return (
		verdict === 'fail' &&
		wall_pct > WALL_REGRESSION_PCT &&
		instructions_pct !== null &&
		instructions_pct >= INSTRUCTIONS_LAYOUT_PCT &&
		instructions_pct <= INSTRUCTIONS_REGRESSION_PCT &&
		wall_pct - instructions_pct <= SIGN_OFF_EXCESS_PCT
	);
}

/** The failing rows a person signs off rather than re-runs. */
export function sign_off_rows(rows: ReadonlyArray<AbRow>): Array<AbRow> {
	return rows.filter((row) => row.sign_off);
}

/** The flat-instruction row (|Δ instr| under `INSTRUCTIONS_LAYOUT_PCT`) whose wall moved the
 * most, or null when no row is flat: the wall move code placement alone gave this pair of
 * builds, the run's own reading of the layout noise its wall column sits on. A reading, not a
 * gate input — layout moves per function, so one row's draw does not bound another's. */
export function layout_reading(rows: ReadonlyArray<AbRow>): AbRow | null {
	let widest: AbRow | null = null;
	for (const row of rows) {
		if (row.instructions_pct === null) continue;
		if (Math.abs(row.instructions_pct) >= INSTRUCTIONS_LAYOUT_PCT) continue;
		if (widest === null || Math.abs(row.wall_pct) > Math.abs(widest.wall_pct)) widest = row;
	}
	return widest;
}

/** The run fails when any row does. */
export function overall_passes(rows: ReadonlyArray<AbRow>): boolean {
	return rows.every((row) => row.verdict !== 'fail');
}

/** The standard error of a row's paired wall delta, in points, after `rounds` rounds at its
 * measured spread: the spread is a property of the machine and stays put, the error of their
 * median shrinks with √rounds. Zero for a zero spread. */
export function wall_delta_error(row: AbRow, rounds: number = row.rounds): number {
	return (MEDIAN_ERROR_PER_MAD * row.wall_pct_spread) / Math.sqrt(rounds);
}

/**
 * Whether a row's verdict is still a coin toss: its wall delta sits within its own standard
 * error (`wall_delta_error`) of `WALL_REGRESSION_PCT`, AND crossing the threshold would change
 * the verdict. The error, not the round-to-round spread: the spread never narrows with more
 * rounds, so a row graded against it stays ambiguous however long the run. At
 * flat instructions both sides of the line pass (pass / LAYOUT); over
 * `INSTRUCTIONS_REGRESSION_PCT` both sides fail, decided by a count more rounds would not move.
 * So only a row with instructions risen within the instruction gate, or fallen, or none
 * counted, can flip between passing and FAIL.
 */
export function row_is_ambiguous(row: AbRow): boolean {
	const instructions = row.instructions_pct;
	const flips =
		instructions === null ||
		(Math.abs(instructions) >= INSTRUCTIONS_LAYOUT_PCT &&
			instructions <= INSTRUCTIONS_REGRESSION_PCT);
	return flips && Math.abs(row.wall_pct - WALL_REGRESSION_PCT) < wall_delta_error(row);
}

/** Whether an ambiguous row could be settled by the cap: at `MAX_ROUNDS` its error, at the
 * spread measured so far, would fall under its distance from the line. A row closer than that
 * is graded on its point estimate now — more rounds would spend the time and not decide it. */
export function row_is_settleable(row: AbRow): boolean {
	return (
		row.rounds < MAX_ROUNDS &&
		Math.abs(row.wall_pct - WALL_REGRESSION_PCT) >= wall_delta_error(row, MAX_ROUNDS)
	);
}

/** The ambiguous rows the run ends without settling: graded on their point estimate. */
export function unsettled_rows(rows: ReadonlyArray<AbRow>): Array<AbRow> {
	return rows.filter(row_is_ambiguous);
}

/** Whether to run another balanced pair of rounds: some row is ambiguous, the cap could settle
 * it, and the pair fits under `MAX_ROUNDS`. */
export function wants_more_rounds(rows: ReadonlyArray<AbRow>, rounds_done: number): boolean {
	return (
		rounds_done + 2 <= MAX_ROUNDS &&
		rows.some((row) => row_is_ambiguous(row) && row_is_settleable(row))
	);
}

/** The rows with a side noisier than `NOISY_SPREAD_PCT`. */
export function noisy_rows(rows: ReadonlyArray<AbRow>): Array<AbRow> {
	return rows.filter(
		(row) =>
			row.base_wall_spread_pct > NOISY_SPREAD_PCT || row.cand_wall_spread_pct > NOISY_SPREAD_PCT
	);
}

/** What one driver run says about the files it measured. */
export interface FileSet {
	files: number;
	bytes: number;
	errors: number;
}

/** How a language's corpus stands between the sides: `measured` (both have files), `absent`
 * (neither has any — the language is left out with a note), `one_sided` (a harness error). */
export type LangPresence = 'measured' | 'absent' | 'one_sided';

export function lang_presence(base_files: number, cand_files: number): LangPresence {
	if (base_files === 0 && cand_files === 0) return 'absent';
	return base_files === 0 || cand_files === 0 ? 'one_sided' : 'measured';
}

/** Why two runs did not measure the same file set (count, bytes, parse failures), or null when
 * they did. A per-file median summed over different files is not an A/B. */
export function file_set_mismatch(a: FileSet, b: FileSet): string | null {
	if (a.files === b.files && a.bytes === b.bytes && a.errors === b.errors) return null;
	const show = (s: FileSet) => `${s.files} files / ${s.bytes} bytes / ${s.errors} parse errors`;
	return `${show(a)} against ${show(b)}`;
}

/**
 * Read a tree's recursive build-inputs listing (`git ls-tree -r -z <tree> -- <inputs>`: entries
 * of `<mode> <type> <oid>\t<path>`, each ended by a NUL, the path raw — `-z` turns off the
 * C-quoting `core.quotePath` applies to a non-ASCII or special byte, which would put a leading
 * `"` on a top-level name): the top-level inputs present, in order; the `required` ones missing;
 * and the key text — the listing without the `overlaid` path, whose content the run replaces
 * before every build (the measuring driver), so neither its presence nor its content in the
 * tree changes the binary. The cache is keyed on the key text's sha256: the blob ids cover
 * exactly the files a build reads, so two trees that differ only outside them — `tests/`, the
 * docs, `scripts/`, the driver — share one snapshot and one built driver. The key text keeps
 * the NUL separators, since a raw path may hold a newline.
 */
export function build_inputs_of(
	ls_tree: string,
	required: ReadonlyArray<string>,
	overlaid: string
): { names: Array<string>; missing: Array<string>; key_text: string } {
	const names: Array<string> = [];
	const kept: Array<string> = [];
	for (const entry of ls_tree.split('\0')) {
		const tab = entry.indexOf('\t');
		if (tab === -1) continue;
		const path = entry.slice(tab + 1);
		const top = path.split('/', 1)[0]!;
		if (!names.includes(top)) names.push(top);
		if (path !== overlaid) kept.push(entry);
	}
	return {
		names,
		missing: required.filter((name) => !names.includes(name)),
		key_text: kept.join('\0')
	};
}

/**
 * The text a built driver's RECIPE is keyed on: the build's whole command line, the variables it
 * sets itself (`BUILD_FIXED_ENV`, names and values) and the names of the variables it keeps from
 * the caller (`BUILD_ENV`). Its hash is part of the binary's name beside the driver's content
 * hash, so a change to how a driver builds — a cargo flag, the profile, a set or kept variable —
 * never reuses a binary built the old way, on either side.
 */
export function build_recipe_text(
	command: ReadonlyArray<string>,
	fixed_env: Readonly<Record<string, string>>,
	kept_env: ReadonlyArray<string>
): string {
	const fixed = Object.entries(fixed_env).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
	return JSON.stringify({ command, fixed_env: fixed, kept_env });
}

/** A cache directory name: the sha256 of a tree's build-inputs key text (`build_inputs_of`). A
 * 40-hex name is a snapshot keyed by the whole tree id, from before the key moved to the build
 * inputs: it is never reused, only kept or evicted like any other by when it was last used. */
export const CACHE_DIR_PATTERN = /^[0-9a-f]{40}([0-9a-f]{24})?$/;

/** One cached snapshot: its directory name (`CACHE_DIR_PATTERN`) and when a run last used it. */
export interface CachedTree {
	name: string;
	mtime_ms: number;
}

/** The cached snapshots to delete: everything but the `in_use` ones and the most recently used
 * others, `KEEP_TREES` kept in all. */
export function trees_to_evict(
	trees: ReadonlyArray<CachedTree>,
	in_use: ReadonlySet<string>
): Array<string> {
	const others = trees
		.filter((tree) => !in_use.has(tree.name))
		.sort((a, b) => b.mtime_ms - a.mtime_ms);
	const keep = Math.max(0, KEEP_TREES - in_use.size);
	return others.slice(keep).map((tree) => tree.name);
}

/**
 * The `instructions:u` count from `perf stat -x,` output (CSV: value, unit, event, …; `#`
 * lines are comments), or null when the event is absent or was not counted
 * (`<not counted>` / `<not supported>`).
 */
export function parse_perf_stat_instructions(text: string): number | null {
	for (const line of text.split('\n')) {
		if (line.startsWith('#')) continue;
		const fields = line.split(',');
		if (fields.length < 3 || !fields[2]!.startsWith('instructions')) continue;
		const value = fields[0]!.trim();
		return /^\d+$/.test(value) ? Number(value) : null;
	}
	return null;
}

/** A µs total the way `tsv_debug profile` prints one: ms with two decimals from 1000 µs, else
 * whole µs. */
export function format_us(us: number): string {
	return us >= 1000 ? `${(us / 1000).toFixed(2)}ms` : `${us.toFixed(0)}us`;
}

/** An instruction count per iteration, in millions. */
export function format_instructions(n: number): string {
	return `${(n / 1e6).toFixed(n < 1e8 ? 3 : 2)}M`;
}

/** A signed percent with three decimals — enough that a value deciding a threshold never
 * rounds onto it in print. */
export function format_pct(pct: number): string {
	return `${pct >= 0 ? '+' : ''}${pct.toFixed(3)}%`;
}

const VERDICT_LABELS: Record<RowVerdict, string> = {
	pass: 'pass',
	layout: 'LAYOUT (pass)',
	warn: 'WARN instr',
	fail: 'FAIL'
};

/** The verdict cell: a sign-off FAIL says so. */
function verdict_label(row: AbRow): string {
	return row.sign_off ? 'FAIL (sign-off)' : VERDICT_LABELS[row.verdict];
}

/** The result table, Markdown-shaped so it pastes into a report as is. */
export function render_table(rows: ReadonlyArray<AbRow>): string {
	const header = [
		'row',
		'base wall',
		'cand wall',
		'Δ wall (paired)',
		'base instr/iter',
		'cand instr/iter',
		'Δ instr',
		'verdict'
	];
	const body = rows.map((row) => [
		`${LANG_LABELS[row.lang]} ${row.phase}`,
		`${format_us(row.base_wall_us)} ±${row.base_wall_spread_pct.toFixed(1)}%`,
		`${format_us(row.cand_wall_us)} ±${row.cand_wall_spread_pct.toFixed(1)}%`,
		`${format_pct(row.wall_pct)} ±${row.wall_pct_spread.toFixed(3)}`,
		row.base_instructions === null ? '—' : format_instructions(row.base_instructions),
		row.cand_instructions === null ? '—' : format_instructions(row.cand_instructions),
		row.instructions_pct === null ? '—' : format_pct(row.instructions_pct),
		verdict_label(row)
	]);
	const widths = header.map((h, i) => Math.max(h.length, ...body.map((cells) => cells[i]!.length)));
	const line = (cells: ReadonlyArray<string>): string =>
		`| ${cells.map((cell, i) => cell.padEnd(widths[i]!)).join(' | ')} |`;
	return [line(header), line(widths.map((w) => '-'.repeat(w))), ...body.map(line)].join('\n');
}
