/**
 * Compose the per-runtime sibling reports (`report.deno.json` / `report.node.json`
 * / `report.bun.json`) into ONE compact cross-runtime view:
 * `results/report.{json,md}`.
 *
 * Deliberately NOT a verbose triplicate — the full per-runtime reports stay as
 * the `report.<runtime>.{json,md}` siblings. This emits only the cross-runtime
 * comparison: per `(group, impl)` row, each runtime's ops/sec side by side plus
 * the ratio vs the first present runtime. A per-runtime delta on the same row is
 * the signal worth reading — same engine, different runtime + binding boundary
 * (Deno → FFI, Node/Bun → N-API) — and the whole reason the bench runs under
 * multiple runtimes (see benches/js/CLAUDE.md §Cross-Runtime). This is also
 * what tsv.fuz.dev composes at the display layer.
 *
 * Runs whatever subset of reports exists (a missing runtime is skipped, not an
 * error — unless none exist). Portable (`node:` builtins) — runs under any
 * runtime.
 *
 * Run: deno task bench:compose   (after one or more runtimes' reports exist)
 */

import { readFile, writeFile } from 'node:fs/promises';
import { exit } from 'node:process';
import { fileURLToPath } from 'node:url';

import type { Machine, Runtime } from './lib/runtime.ts';

/**
 * Every runtime a sibling report can come from, and the order they fold in.
 * `Runtime` is the authority on the vocabulary; this is the enumeration.
 *
 * A `Record` over the union rather than an array, because the two directions are
 * not equally cheap to state. `['deno','node','bun'] satisfies readonly Runtime[]`
 * proves only that each listed value IS a runtime — the direction that matters
 * here is the reverse, that every runtime is LISTED, and a new member of the union
 * would otherwise go on being silently unfolded. A `Record` key set is exhaustive
 * by construction: add a runtime in `lib/runtime.ts` and this literal fails to
 * compile until it is placed.
 */
const RUNTIME_FOLD_ORDER: Record<Runtime, number> = { deno: 0, node: 1, bun: 2 };

const RUNTIMES: readonly Runtime[] = (Object.keys(RUNTIME_FOLD_ORDER) as Runtime[]).sort(
	(a, b) => RUNTIME_FOLD_ORDER[a] - RUNTIME_FOLD_ORDER[b]
);

/**
 * The fields of a per-runtime `report.<runtime>.json` row this composer reads.
 *
 * The timings are nullable because a perf report can carry a **coverage-only**
 * row — a tool measured for what it accepts but deliberately never timed
 * (`rsvelte-fmt`; see docs/benchmarks.md §Coverage-only rows). Such a row is
 * dropped below rather than folded.
 */
interface Entry {
	name: string;
	group: string;
	mean_ns: number | null;
	ops_per_second: number | null;
	files_iterated: number | null;
	runtime: Runtime;
	/**
	 * Coefficient of variation (`std_dev_ns / mean_ns`, post-outlier-removal), or
	 * null on a coverage-only row. Optional because siblings written before it was
	 * folded here carry it in `entries[]` but this composer never read it.
	 *
	 * The one field this report needs that is not a number it prints: every cell
	 * here is a RATIO of two sibling means, and a ratio inherits the noise of both.
	 */
	cv?: number | null;
	/**
	 * Timings behind `cv` and `mean_ns` — the MAD-cleaned count, not the raw
	 * iterations. Optional on the same terms as `cv`.
	 *
	 * Read together with it, never separately: a cv is an ESTIMATE, and its own
	 * error falls off with n. The bench drives sample count from `duration_ms` with
	 * a floor of 5 (7 on the slow tier), so a multi-second row can land at 3–7
	 * cleaned timings while a microsecond row lands at four figures — a spread of
	 * two orders of magnitude inside one table.
	 */
	sample_size?: number | null;
}

/**
 * One impl that failed to init on a sibling's machine (`version` 10+ reports;
 * `rows` from `version` 12).
 *
 * The fold below reads `rows`, never `impl`: these tables are keyed by row name,
 * and `impl` is the init-line label (`Biome`), which matches none of them.
 */
interface UnavailableImpl {
	impl: string;
	reason: string;
	rows?: string[];
}

interface Report {
	version: number;
	runtime: Runtime;
	timestamp: string;
	git_commit: string | null;
	/**
	 * Absent on pre-`version` 7 siblings — the hardware identity behind the numbers.
	 *
	 * The one field here typed from the PRODUCER's module rather than described
	 * locally, which buys one definition and costs an assertion: `Machine`'s fields
	 * are all required, and the siblings on disk may predate any of them. Sound as
	 * written because the reads below stay inside the four fields `Machine` shipped
	 * with at `version` 7 — a field added to it later must be read as optional here,
	 * whatever the shared type says (see `Machine`).
	 */
	machine?: Machine;
	versions: Record<string, string>;
	entries: Entry[];
	/** Absent on pre-`version` 10 siblings — read as "not recorded", never as "none". */
	unavailable?: UnavailableImpl[];
}

const results_dir = fileURLToPath(new URL('./results/', import.meta.url));

async function read_report(runtime: Runtime): Promise<Report | null> {
	const path = `${results_dir}report.${runtime}.json`;
	let text: string;
	try {
		text = await readFile(path, 'utf8');
	} catch {
		// Absent is ordinary — the composer folds whatever exists, and a runtime
		// nobody has run yet simply has no column.
		return null;
	}
	try {
		return JSON.parse(text) as Report;
	} catch (e) {
		// UNREADABLE is not the same claim as absent, and a catch-all made them one:
		// an interrupted run leaves a truncated `report.<runtime>.json`, and folding
		// it as "absent" drops that runtime from the table with nothing saying why —
		// a report that silently measures two runtimes while its header still says
		// the run covered three.
		console.error(
			`⚠ compose: ${path} exists but is not valid JSON (${
				e instanceof Error ? e.message : String(e)
			}) — treating ${runtime} as absent. Usually an interrupted run; re-run ` +
				`\`deno task bench:${runtime}\` to rewrite it.`
		);
		return null;
	}
}

const reports = new Map<Runtime, Report>();
for (const r of RUNTIMES) {
	const report = await read_report(r);
	if (report) reports.set(r, report);
}

if (reports.size === 0) {
	console.error(
		'No per-runtime reports found in results/ ' +
			'(report.deno.json / report.node.json / report.bun.json).\n' +
			'Run `deno task bench` (or a per-runtime bench) first.'
	);
	exit(1);
}

/** Runtimes present, in canonical order; the first is the ratio baseline. */
const present = RUNTIMES.filter((r) => reports.has(r));
const base_runtime = present[0];

/** One cross-runtime comparison row, keyed by `${group}/${name}`. */
interface Row {
	group: string;
	name: string;
	ops: Partial<Record<Runtime, number>>;
	mean_ns: Partial<Record<Runtime, number>>;
	files_iterated: Partial<Record<Runtime, number | null>>;
	/** Per-runtime measurement noise, for `within_noise` below. */
	cv: Partial<Record<Runtime, number>>;
	/** Timings behind each `cv` — the precondition for trusting it. */
	samples: Partial<Record<Runtime, number>>;
}

const rows = new Map<string, Row>();
const order: string[] = [];
for (const r of present) {
	for (const e of reports.get(r)!.entries) {
		// Coverage-only rows carry no timing, and this report is purely the
		// cross-runtime THROUGHPUT view — folding one in would mint a row whose
		// every cell is empty, and whose per-runtime delta (the whole point of the
		// combined report) can't exist. Its coverage is already published in each
		// per-runtime sibling.
		if (e.ops_per_second == null || e.mean_ns == null) continue;
		const key = `${e.group}/${e.name}`;
		let row = rows.get(key);
		if (!row) {
			row = {
				group: e.group,
				name: e.name,
				ops: {},
				mean_ns: {},
				files_iterated: {},
				cv: {},
				samples: {}
			};
			rows.set(key, row);
			order.push(key);
		}
		row.ops[r] = e.ops_per_second;
		row.mean_ns[r] = e.mean_ns;
		row.files_iterated[r] = e.files_iterated;
		if (e.cv != null) row.cv[r] = e.cv;
		if (e.sample_size != null) row.samples[r] = e.sample_size;
	}
}

// Provenance per source, plus a loud flag when the siblings come from
// different commits/versions. The composer folds whatever reports exist, so a
// fresh `report.deno.json` can otherwise sit silently next to a stale
// `report.node.json` — and cross-runtime ratios are only meaningful on
// same-vintage siblings.
const sources = present.map((r) => ({
	runtime: r,
	timestamp: reports.get(r)!.timestamp,
	git_commit: reports.get(r)!.git_commit,
	tsv: reports.get(r)!.versions?.tsv ?? null,
	machine: reports.get(r)!.machine ?? null,
	// Per-sibling, because this is exactly a per-runtime fact: an impl that loads
	// under Deno and not under Bun leaves a row in one sibling and none in the
	// other, which reads as a runtime PERFORMANCE difference in a table of ratios.
	// `null` (not `[]`) when the sibling predates the field — "not recorded" and
	// "nothing missing" are different claims, and only one of them is safe to make.
	unavailable: reports.get(r)!.unavailable ?? null
}));

/**
 * Row names each sibling recorded as unavailable, or `null` when the sibling
 * predates the field — "not recorded" and "nothing was missing" are different
 * claims, and only one of them is safe to make.
 *
 * The ONE derivation of that fact. Both readers below take it from here rather
 * than walking `sources` again: the folded disclosure (`unavailable_by_runtime`)
 * and the staleness detector (`partial_rows`) ask the same question at different
 * fidelities — the first collapses "not recorded" into "nothing missing", the
 * second must not — and two walks would be free to drift on which one they meant.
 * Insertion order is `sources` order, so the array below stays in runtime order.
 */
const unavailable_rows_by_runtime = new Map<Runtime, ReadonlySet<string> | null>(
	sources.map((s) => [
		s.runtime,
		s.unavailable === null ? null : new Set(s.unavailable.flatMap((u) => u.rows ?? []))
	])
);

/**
 * Every ROW a sibling recorded as unavailable, listed under the runtime that
 * recorded it — the asymmetry a reader of the folded rows would otherwise attribute
 * to speed. Siblings predating the field (`unavailable === null`) contribute
 * nothing, which is "not recorded" rather than a claim that nothing was missing.
 *
 * Rows, not impl labels, because that is the only identity that joins: a consumer
 * asking "is this blank cell a load failure?" holds a row name, and `Biome` /
 * `OXC WASM` match no row (`biome-wasm`, `oxc-parser-wasm`). A load failure that
 * cost this surface no row contributes nothing here — correctly, since no table has
 * a gap to explain.
 *
 * A sibling from the brief `version` 10–11 window recorded failures WITHOUT rows;
 * those contribute nothing here too, and unlike the cases above that is a shortfall
 * of the data rather than a claim about it. Neither version was ever a released
 * sibling — the field and its rows landed in one change — so the state is only
 * reachable from a local run made mid-change, and it clears on the next full
 * `bench:perf`. Their `reason`s survive in `sources[].unavailable` either way.
 *
 * Not narrowed to the rows missing on SOME runtimes: a row absent on all of them is
 * still a shortfall of the machine that produced these reports, and the fold has no
 * row for it either way. The md line below is worded to cover both.
 */
const unavailable_by_runtime = [...unavailable_rows_by_runtime].flatMap(([runtime, rows]) =>
	rows === null || rows.size === 0 ? [] : [{ runtime, rows: [...rows] }]
);
const mixed_vintage =
	new Set(sources.map((s) => `${s.git_commit ?? '?'}@${s.tsv ?? '?'}`)).size > 1;

/**
 * Rows one sibling MEASURED that another doesn't carry at all, with no load failure
 * on that side to explain the gap.
 *
 * The composer folds whatever reports exist, so a row added to the harness after a
 * sibling was last run lands in its column as a bare `—` — visually identical to a
 * row whose impl failed to load, which `unavailable_by_runtime` above DOES explain,
 * and to a language a row doesn't cover, which never reaches the group at all.
 * `mixed_vintage` flags the general condition; this names the rows it actually cost,
 * which is the part a reader of one group's table can act on.
 *
 * A runtime whose sibling predates `unavailable` is skipped rather than counted:
 * with nothing recorded there, an absent row can't be told from an unloadable impl,
 * and claiming drift would be the unsafe half of that pair. (A sibling from the
 * brief `version` 10–11 window records failures WITHOUT `rows`, so its absences do
 * read as unexplained here — the same data shortfall `unavailable_by_runtime`
 * carries, and it clears on the next full `bench:perf`.)
 */
const partial_rows = order
	.map((key) => {
		const row = rows.get(key)!;
		const missing = present.filter((r) => {
			if (row.ops[r] !== undefined) return false;
			const recorded = unavailable_rows_by_runtime.get(r) ?? null;
			return recorded !== null && !recorded.has(row.name);
		});
		return { group: row.group, name: row.name, missing };
	})
	.filter((entry) => entry.missing.length > 0);

// Loud flag when the siblings were produced on DIFFERENT boxes — the
// throughput numbers are machine-relative, so cross-runtime ratios are only
// meaningful on same-machine siblings. Compares the HARDWARE identity only
// (CPU/OS/arch); `runtime_version` differs per sibling by design. Ignores
// siblings with no `machine` (pre-`version` 7), so a stale old sibling can't
// spuriously trip the flag during the transition.
const machine_ids = sources
	.map((s) => (s.machine ? `${s.machine.cpu_model}|${s.machine.os}|${s.machine.arch}` : null))
	.filter((v): v is string => v !== null);
const mixed_machine = machine_ids.length > 0 && new Set(machine_ids).size > 1;
/** The shared hardware identity (any present source has it — they agree unless
 * `mixed_machine`), for the one-line md disclosure. */
const machine = sources.find((s) => s.machine)?.machine ?? null;

/**
 * The COMBINED report's schema version — distinct from the per-runtime siblings'
 * (`bench.ts` `REPORT_SCHEMA_VERSION`), which this composer reads rather than emits.
 *
 * BUMP IT whenever a top-level field is added, removed, or has its meaning or key
 * names changed, and say what the new number means in one line below, so a consumer
 * can tell "this composer didn't record it" from "there was nothing to record".
 *
 * 11: `within_noise[]` — per-runtime deltas smaller than the combined cv of the two
 * measurements they divide, i.e. the cells that are not runtime effects. The first
 * field here that qualifies a number this report prints rather than adding one.
 *
 * 10: `partial_rows[]` — rows one sibling measured and another doesn't carry, with
 * no recorded load failure to explain it. Kept beside 11 rather than replaced by it:
 * both landed before any consumer saw either, so a reader at 11 would otherwise find
 * one of the two fields undocumented at the only place this file documents them.
 *
 * 12: `within_noise[]` entries carry `samples` — `[base runtime, cell runtime]`
 * cleaned timing counts — and a cell is only classified when BOTH clear
 * `MIN_NOISE_SAMPLES`, so membership narrowed. A consumer that read 11's list as "every quiet cell" would
 * read 12's as the same claim, and it is a stricter one.
 */
const COMBINED_SCHEMA_VERSION = 12;

// JSON: metadata + provenance per source + the comparison rows.
/**
 * The cleaned-timing count below which a row's cv is not a usable noise estimate.
 *
 * Ten, not the 30 `benchmark.ts` names as the central-limit floor for its Welch
 * test: this is a reading aid, and 30 would silence it on every row the bench
 * measures in seconds — most of the format surface. Ten keeps the aid where the
 * estimate is merely rough and withdraws it where it is arbitrary.
 */
const MIN_NOISE_SAMPLES = 10;

/**
 * Per-runtime deltas that are smaller than the noise of the two measurements they
 * divide — i.e. the cells a reader must NOT read as a runtime effect.
 *
 * This report's stated subject is exactly those deltas ("A per-runtime delta on the
 * same row is the signal"), and every cell is a ratio of two sibling means, so it
 * inherits both means' noise while printing neither. A `1.15x` between two rows that
 * each wobble 10% is not a runtime difference; a `1.15x` between two 1% rows is.
 * Nothing here could tell them apart.
 *
 * The bar is the quadrature sum of the two rows' cv — the standard combination for
 * independent relative errors, and deliberately a rough one: it is a READING AID,
 * not a significance test (that is `benchmark_baseline_compare`'s Welch job, on a
 * run this composer never sees). Rows missing a cv on either side are skipped
 * rather than assumed quiet — a sibling predating the field would otherwise read as
 * noiseless.
 *
 * ⚠ A row too thinly sampled to have a trustworthy cv is skipped on the SAME
 * argument (`MIN_NOISE_SAMPLES`), and it is the more reachable half. A cv is an
 * estimate, and this test consumes it in the direction where being wrong is
 * expensive: too SMALL a cv makes a real per-runtime difference read as "no
 * difference", which is the one verdict here a reader cannot check from the table.
 * The bench floors iterations at 5 (7 on the slow tier) and drives the rest from
 * `duration_ms`, so a multi-second row lands at a handful of cleaned timings —
 * measured, 17 of 44 rows per runtime sit under ten — while a fast row lands at
 * four figures. Three timings that happen to agree are not evidence of quiet.
 *
 * Measured when this was written: across the three committed reports, 6 of 44
 * node/deno deltas land inside their combined cv, all six at ~1.00x — and one of
 * the six rests on 7 timings a side, which is exactly the cell this guard now
 * declines to call. So it currently confirms "no difference" rather than
 * overturning anything; it is here for the case it does overturn.
 */
const within_noise = order.flatMap((key) => {
	const row = rows.get(key)!;
	const cells: Array<{
		group: string;
		name: string;
		runtime: Runtime;
		delta: number;
		noise: number;
		/** `[base runtime, this cell's runtime]` — the order the md renders as `n=a/b`. */
		samples: [number, number];
	}> = [];
	const base_ops = row.ops[base_runtime];
	const base_cv = row.cv[base_runtime];
	const base_n = row.samples[base_runtime];
	if (base_ops === undefined || base_cv === undefined) return cells;
	if (base_n === undefined || base_n < MIN_NOISE_SAMPLES) return cells;
	for (const r of present) {
		if (r === base_runtime) continue;
		const ops = row.ops[r];
		const cv = row.cv[r];
		const n = row.samples[r];
		if (ops === undefined || cv === undefined) continue;
		if (n === undefined || n < MIN_NOISE_SAMPLES) continue;
		const delta = Math.abs(ops / base_ops - 1);
		const noise = Math.sqrt(base_cv ** 2 + cv ** 2);
		if (delta < noise) {
			cells.push({
				group: row.group,
				name: row.name,
				runtime: r,
				delta,
				noise,
				samples: [base_n, n]
			});
		}
	}
	return cells;
});

const combined = {
	version: COMBINED_SCHEMA_VERSION,
	kind: 'combined' as const,
	generated: new Date().toISOString(),
	runtimes: present,
	mixed_vintage,
	mixed_machine,
	unavailable_by_runtime,
	partial_rows,
	within_noise,
	sources,
	rows: order.map((key) => {
		const row = rows.get(key)!;
		return {
			group: row.group,
			name: row.name,
			ops_per_second: row.ops,
			mean_ns: row.mean_ns,
			files_iterated: row.files_iterated
		};
	})
};

// Markdown: one table per group, runtimes side by side + ratio vs base_runtime.
function fmt_ops(n: number | undefined): string {
	return n === undefined ? '—' : n.toFixed(1);
}

function fmt_ratio(self: number | undefined, base: number | undefined): string {
	if (self === undefined || base === undefined || base === 0) return '—';
	return `${(self / base).toFixed(2)}x`;
}

const others = present.filter((r) => r !== base_runtime);

const md: string[] = [];
md.push('# tsv benchmark results — cross-runtime\n');
md.push(`**Generated:** ${combined.generated}\n`);
md.push(
	`**Runtimes:** ${present.join(', ')} ` +
		'— each runtime’s full report is its `report.<runtime>.{json,md}` sibling.\n'
);
for (const s of sources) {
	const rt_ver = s.machine ? ` ${s.machine.runtime_version}` : '';
	md.push(
		`- \`${s.runtime}\`${rt_ver}: ${s.git_commit ?? 'unknown commit'} @ ${s.timestamp}` +
			`${s.tsv ? ` (tsv ${s.tsv})` : ''}`
	);
}
md.push('');
if (machine) {
	md.push(`**Machine:** ${machine.cpu_model} · ${machine.os}/${machine.arch}\n`);
}
if (mixed_machine) {
	md.push(
		'⚠ **Mixed machines** — the sibling reports were produced on different ' +
			'hardware, so the cross-runtime ratios are not comparable; re-run every ' +
			'runtime on one machine (`deno task bench:perf`).\n'
	);
}
if (mixed_vintage) {
	md.push(
		'⚠ **Mixed vintages** — the sibling reports above come from different ' +
			'commits/versions, so the cross-runtime ratios are unreliable; re-run the ' +
			'stale runtimes (`deno task bench:perf` refreshes all three).\n'
	);
}
if (unavailable_by_runtime.length > 0) {
	md.push(
		'**Not measured everywhere:** ' +
			unavailable_by_runtime.map((u) => `${u.runtime} — ${u.rows.join(', ')}`).join('; ') +
			'. The implementation behind each row failed to load on the runtime(s) named, so it ' +
			'contributes no measurement there — a row thinner than its neighbours, or missing ' +
			'outright, is a load failure rather than a speed result. The per-runtime report’s ' +
			'`unavailable` carries the impl and the cause.\n'
	);
}
if (partial_rows.length > 0) {
	md.push(
		'**Partially measured:** ' +
			partial_rows
				.map((r) => `\`${r.group}/${r.name}\` (absent: ${r.missing.join(', ')})`)
				.join('; ') +
			'. Those runtimes recorded no load failure for the row, so its absence is unexplained — ' +
			'usually a sibling report predating the row. Re-run the stale runtimes ' +
			'(`deno task bench:perf`) before reading the ratios in its group.\n'
	);
}
if (within_noise.length > 0) {
	md.push(
		`**Within noise:** ${within_noise.length} per-runtime delta(s) are smaller than the two ` +
			"measurements' combined variation, so they are not runtime effects — " +
			within_noise
				.map(
					(c) =>
						`\`${c.group}/${c.name}\` ${c.runtime} (${(c.delta * 100).toFixed(1)}% vs ` +
						`${(c.noise * 100).toFixed(1)}% noise, n=${c.samples.join('/')})`
				)
				.join('; ') +
			'. Read those cells as "no difference". The two cv values behind each are ' +
			"`entries[].cv` in the per-runtime JSON — NOT that report's §Unstable Rows, which lists " +
			'only rows past its own 10% threshold and so names none of these: a cell lands here ' +
			'whenever the delta is small relative to the noise, which two perfectly ordinary 3% rows ' +
			`satisfy. \`n\` is the cleaned timings behind each cv — a row under ${MIN_NOISE_SAMPLES} ` +
			'a side is left unclassified rather than called quiet on an estimate that thin.\n'
	);
}
md.push(
	'A per-runtime delta on the same row is the signal: same engine, different ' +
		'runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs ' +
		`\`${base_runtime}\` (> 1 = faster than ${base_runtime}). A group (or row) ` +
		'flagged `⚠ files …` iterated *different per-runtime intersections* (each ' +
		'runtime times the files all its impls passed preflight on), so a sliver ' +
		'of the ratio can be file-set difference rather than runtime effect.\n'
);

const groups: string[] = [];
for (const key of order) {
	const g = rows.get(key)!.group;
	if (!groups.includes(g)) groups.push(g);
}

/** `deno 1214 / node 1215 / bun 1217` — the per-runtime iterated counts. */
function fmt_file_counts(fi: Row['files_iterated']): string {
	return present.map((r) => `${r} ${fi[r] ?? '—'}`).join(' / ');
}

/** Whether a row's per-runtime iterated counts differ (nulls ignored). */
function files_unequal(fi: Row['files_iterated']): boolean {
	const counts = present.map((r) => fi[r]).filter((v) => typeof v === 'number');
	return new Set(counts).size > 1;
}

for (const group of groups) {
	const group_rows = order.map((key) => rows.get(key)!).filter((row) => row.group === group);

	// Disclose unequal per-runtime intersections (see the header note). In
	// intersection mode every row in a group iterates the same set, so the
	// annotation lifts to ONE group-level line; per-row markers only remain for
	// rows that deviate from the group pattern (union mode).
	const signatures = new Set(group_rows.map((row) => fmt_file_counts(row.files_iterated)));
	const uniform = signatures.size === 1;
	const group_flagged = uniform && files_unequal(group_rows[0].files_iterated);

	md.push(`## ${group}\n`);
	if (group_flagged) {
		md.push(`⚠ files ${fmt_file_counts(group_rows[0].files_iterated)}\n`);
	}
	const header = [
		'Impl',
		...present.map((r) => `${r} sweeps/sec`),
		...others.map((r) => `${r}/${base_runtime}`)
	];
	md.push(`| ${header.join(' | ')} |`);
	md.push(`| ${header.map((_, i) => (i === 0 ? '---' : '---:')).join(' | ')} |`);
	for (const row of group_rows) {
		const name_cell =
			!uniform && files_unequal(row.files_iterated)
				? `${row.name} ⚠ files ${fmt_file_counts(row.files_iterated)}`
				: row.name;
		const cells = [
			name_cell,
			...present.map((r) => fmt_ops(row.ops[r])),
			...others.map((r) => fmt_ratio(row.ops[r], row.ops[base_runtime]))
		];
		md.push(`| ${cells.join(' | ')} |`);
	}
	md.push('');
}

await writeFile(`${results_dir}report.json`, JSON.stringify(combined, null, '\t'));
await writeFile(`${results_dir}report.md`, md.join('\n'));

if (mixed_vintage) {
	console.error(
		'⚠ compose: sibling reports have MIXED VINTAGES (' +
			sources.map((s) => `${s.runtime}=${(s.git_commit ?? '?').slice(0, 8)}`).join(' ') +
			') — cross-runtime ratios unreliable; re-run the stale runtimes.'
	);
}
if (mixed_machine) {
	console.error(
		'⚠ compose: sibling reports were produced on DIFFERENT machines (' +
			sources.map((s) => `${s.runtime}=${s.machine?.cpu_model ?? '?'}`).join(' | ') +
			') — cross-runtime ratios are not comparable; re-run every runtime on one box.'
	);
}
if (partial_rows.length > 0) {
	console.error(
		'⚠ compose: rows measured on some runtimes and ABSENT on others with no load failure (' +
			partial_rows.map((r) => `${r.group}/${r.name}=${r.missing.join('+')}`).join(' | ') +
			') — unexplained, usually a sibling predating the row; re-run them.'
	);
}
if (unavailable_by_runtime.length > 0) {
	console.error(
		'⚠ compose: rows unavailable on some runtimes (' +
			unavailable_by_runtime.map((u) => `${u.runtime}=${u.rows.join('+')}`).join(' | ') +
			') — absent there because the impl behind them failed to load, not as a speed result.'
	);
}
console.log(`Composed cross-runtime report from: ${present.join(', ')}`);
console.log(`  ${results_dir}report.json`);
console.log(`  ${results_dir}report.md`);
