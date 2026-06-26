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

interface Report {
	version: number;
	runtime: Runtime;
	timestamp: string;
	git_commit: string | null;
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

// JSON: metadata + provenance per source + the comparison rows.
const combined = {
	version: 5,
	kind: 'combined' as const,
	generated: new Date().toISOString(),
	runtimes: present,
	sources: present.map((r) => ({
		runtime: r,
		timestamp: reports.get(r)!.timestamp,
		git_commit: reports.get(r)!.git_commit,
		tsv: reports.get(r)!.versions?.tsv ?? null,
	})),
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
md.push(
	'A per-runtime delta on the same row is the signal: same engine, different ' +
		'runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs ' +
		`\`${base_runtime}\` (> 1 = faster than ${base_runtime}).\n`,
);

const groups: string[] = [];
for (const key of order) {
	const g = rows.get(key)!.group;
	if (!groups.includes(g)) groups.push(g);
}

for (const group of groups) {
	md.push(`## ${group}\n`);
	const header = [
		'Impl',
		...present.map((r) => `${r} ops/sec`),
		...others.map((r) => `${r}/${base_runtime}`),
	];
	md.push(`| ${header.join(' | ')} |`);
	md.push(`| ${header.map((_, i) => (i === 0 ? '---' : '---:')).join(' | ')} |`);
	for (const key of order) {
		const row = rows.get(key)!;
		if (row.group !== group) continue;
		const cells = [
			row.name,
			...present.map((r) => fmt_ops(row.ops[r])),
			...others.map((r) => fmt_ratio(row.ops[r], row.ops[base_runtime])),
		];
		md.push(`| ${cells.join(' | ')} |`);
	}
	md.push('');
}

await writeFile(`${results_dir}report.json`, JSON.stringify(combined, null, '\t'));
await writeFile(`${results_dir}report.md`, md.join('\n'));

console.log(`Composed cross-runtime report from: ${present.join(', ')}`);
console.log(`  ${results_dir}report.json`);
console.log(`  ${results_dir}report.md`);
