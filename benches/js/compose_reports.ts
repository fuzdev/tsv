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

const RUNTIMES = ['deno', 'node', 'bun'] as const;
type Runtime = (typeof RUNTIMES)[number];

/** The fields of a per-runtime `report.<runtime>.json` row this composer reads. */
interface Entry {
	name: string;
	group: string;
	mean_ns: number;
	ops_per_second: number;
	files_iterated: number | null;
	runtime: Runtime;
}

/** The machine block a `version` 7+ sibling report carries (`lib/runtime.ts`
 * `Machine`); absent on older siblings, hence optional on `Report`. */
interface Machine {
	cpu_model: string;
	os: string;
	arch: string;
	runtime_version: string;
}

interface Report {
	version: number;
	runtime: Runtime;
	timestamp: string;
	git_commit: string | null;
	machine?: Machine;
	versions: Record<string, string>;
	entries: Entry[];
}

const results_dir = fileURLToPath(new URL('./results/', import.meta.url));

async function read_report(runtime: Runtime): Promise<Report | null> {
	try {
		return JSON.parse(await readFile(`${results_dir}report.${runtime}.json`, 'utf8')) as Report;
	} catch {
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
			'Run `deno task bench` (or a per-runtime bench) first.',
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
}

const rows = new Map<string, Row>();
const order: string[] = [];
for (const r of present) {
	for (const e of reports.get(r)!.entries) {
		const key = `${e.group}/${e.name}`;
		let row = rows.get(key);
		if (!row) {
			row = { group: e.group, name: e.name, ops: {}, mean_ns: {}, files_iterated: {} };
			rows.set(key, row);
			order.push(key);
		}
		row.ops[r] = e.ops_per_second;
		row.mean_ns[r] = e.mean_ns;
		row.files_iterated[r] = e.files_iterated;
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
}));
const mixed_vintage = new Set(sources.map((s) => `${s.git_commit ?? '?'}@${s.tsv ?? '?'}`)).size > 1;

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

// JSON: metadata + provenance per source + the comparison rows.
const combined = {
	version: 7,
	kind: 'combined' as const,
	generated: new Date().toISOString(),
	runtimes: present,
	mixed_vintage,
	mixed_machine,
	sources,
	rows: order.map((key) => {
		const row = rows.get(key)!;
		return {
			group: row.group,
			name: row.name,
			ops_per_second: row.ops,
			mean_ns: row.mean_ns,
			files_iterated: row.files_iterated,
		};
	}),
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
		'— each runtime’s full report is its `report.<runtime>.{json,md}` sibling.\n',
);
for (const s of sources) {
	const rt_ver = s.machine ? ` ${s.machine.runtime_version}` : '';
	md.push(
		`- \`${s.runtime}\`${rt_ver}: ${s.git_commit ?? 'unknown commit'} @ ${s.timestamp}` +
			`${s.tsv ? ` (tsv ${s.tsv})` : ''}`,
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
			'runtime on one machine (`deno task bench:perf`).\n',
	);
}
if (mixed_vintage) {
	md.push(
		'⚠ **Mixed vintages** — the sibling reports above come from different ' +
			'commits/versions, so the cross-runtime ratios are unreliable; re-run the ' +
			'stale runtimes (`deno task bench:perf` refreshes all three).\n',
	);
}
md.push(
	'A per-runtime delta on the same row is the signal: same engine, different ' +
		'runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs ' +
		`\`${base_runtime}\` (> 1 = faster than ${base_runtime}). A group (or row) ` +
		'flagged `⚠ files …` iterated *different per-runtime intersections* (each ' +
		'runtime times the files all its impls passed preflight on), so a sliver ' +
		'of the ratio can be file-set difference rather than runtime effect.\n',
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
		...others.map((r) => `${r}/${base_runtime}`),
	];
	md.push(`| ${header.join(' | ')} |`);
	md.push(`| ${header.map((_, i) => (i === 0 ? '---' : '---:')).join(' | ')} |`);
	for (const row of group_rows) {
		const name_cell = !uniform && files_unequal(row.files_iterated)
			? `${row.name} ⚠ files ${fmt_file_counts(row.files_iterated)}`
			: row.name;
		const cells = [
			name_cell,
			...present.map((r) => fmt_ops(row.ops[r])),
			...others.map((r) => fmt_ratio(row.ops[r], row.ops[base_runtime])),
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
			') — cross-runtime ratios unreliable; re-run the stale runtimes.',
	);
}
if (mixed_machine) {
	console.error(
		'⚠ compose: sibling reports were produced on DIFFERENT machines (' +
			sources.map((s) => `${s.runtime}=${s.machine?.cpu_model ?? '?'}`).join(' | ') +
			') — cross-runtime ratios are not comparable; re-run every runtime on one box.',
	);
}
console.log(`Composed cross-runtime report from: ${present.join(', ')}`);
console.log(`  ${results_dir}report.json`);
console.log(`  ${results_dir}report.md`);
