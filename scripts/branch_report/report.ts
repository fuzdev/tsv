/**
 * The branch report's model and its pure half: rows, row verdicts against an A/A floor, the
 * problems that make a run exit non-zero, and the Markdown rendering. Everything here is a
 * function of data, so `report_test.ts` grades it without a build.
 *
 * A row carries both sides' values and a kind. An EXACT row (bytes, counts, type widths) is the
 * same on two builds of one source, or something is wrong; a MEASURED row (wall, cycles, peak
 * RSS) sits on noise, so the document never calls one of its differences real without a floor,
 * the same row's delta in a same-ref run (`--aa`).
 *
 * @module
 */

export const REPORT_VERSION = 1;

export const SECTION_IDS = [
	'sizes',
	'types',
	'work',
	'build_wall',
	'timing',
	'store_rows'
] as const;
export type SectionId = (typeof SECTION_IDS)[number];

export const SECTION_LETTERS: Readonly<Record<SectionId, string>> = {
	sizes: 'A',
	types: 'B',
	work: 'C',
	build_wall: 'D',
	timing: 'E',
	store_rows: 'F'
};

export const SECTION_TITLES: Readonly<Record<SectionId, string>> = {
	sizes: 'Sizes',
	types: 'Type sizes',
	work: 'Work counters',
	build_wall: 'Build wall',
	timing: 'Timing',
	store_rows: 'Store rows'
};

/**
 * A leg's outcome. `ok`, `not_requested` and `not_applicable` are no gap; every other status is
 * one, and a gap fails the run unless `--accept-gaps` names it accepted.
 */
export type LegStatus =
	| 'ok'
	| 'planned'
	| 'not_requested'
	| 'not_applicable'
	| 'skipped'
	| 'missing_tool'
	| 'failed'
	| 'unavailable_on_ref';

export function status_is_gap(status: LegStatus): boolean {
	return (
		status === 'skipped' ||
		status === 'missing_tool' ||
		status === 'failed' ||
		status === 'unavailable_on_ref'
	);
}

export interface Row {
	key: string;
	unit: string;
	/** The base side's value, or null when the base side has no such row. */
	base: number | null;
	cand: number | null;
	/** True for a deterministic row: its two sides are compared exactly, never against a floor. */
	exact: boolean;
	note?: string;
}

export interface Table {
	id: string;
	title: string;
	note?: string;
	rows: Array<Row>;
	/** `changed` renders only the rows that differ; the JSON keeps every row either way. */
	display: 'all' | 'changed';
}

/** One command the leg ran (or, for a planned leg, would run), as a line a person can re-run. */
export type CommandLine = string;

export interface Leg {
	name: string;
	status: LegStatus;
	reason?: string;
	wall_ms: number | null;
	commands: Array<CommandLine>;
	tables: Array<Table>;
	notes: Array<string>;
}

export interface Section {
	id: SectionId;
	/** The worst of the legs', or the section's own when it has none. */
	status: LegStatus;
	reason?: string;
	legs: Array<Leg>;
}

export interface SideInfo {
	slot: 'base' | 'cand';
	ref: string;
	commit: string;
	tree: string;
	build_dir: string;
	rustc: string;
	cargo: string;
}

export interface Provenance {
	command_line: string;
	base: SideInfo;
	cand: SideInfo;
	machine: Record<string, string>;
	tools: Record<string, string>;
	corpus: { path: string; tree: string; dirty: boolean; relative_path: string };
	cargo_build_jobs: string | null;
	flags: Array<string>;
}

export interface BranchReport {
	version: number;
	generated: string;
	same_ref: boolean;
	provenance: Provenance;
	/** What a later run's floors are compared against. */
	floor_context: FloorContext;
	/** The A/A reports the floors came from, one per `--aa`. */
	aa_sources: Array<FloorSource>;
	/** Why a draw (or its timing rows) was refused as a floor. */
	floor_mismatches: Array<string>;
	sections: Array<Section>;
}

// -- rows --------------------------------------------------------------------------------------

export type RowVerdict =
	'same' | 'differs' | 'added' | 'removed' | 'noise' | 'beyond_floor' | 'unfloored';

/**
 * What a floor is only valid against. The commit is deliberately absent: a floor from main
 * grades a later pair. Corpus and machine bind every measured row; rounds, iterations, the
 * timing legs and the bench's narrowing bind the timing sections' rows alone.
 */
export interface FloorContext {
	corpus_tree: string;
	/** The corpus's path inside its repo: the tree id alone names the whole snapshot. */
	corpus_relative_path: string;
	corpus_dirty: boolean;
	cpu: string;
	hostname: string;
	rounds: number;
	iterations: number;
	timing_legs: Array<string>;
	bench_env: Record<string, string>;
	cargo_build_jobs: string | null;
}

export interface FloorSource {
	path: string;
	generated: string;
	commit: string;
	context: FloorContext | null;
}

/** A row's floor: its largest absolute percent delta across the A/A draws that hold it. */
export interface FloorEntry {
	floor: number;
	draws: number;
}

export type Floors = ReadonlyMap<string, FloorEntry>;

export function row_floor_key(section: SectionId, table: string, row: string): string {
	return `${section}/${table}/${row}`;
}

export function row_pct(row: Row): number | null {
	if (row.base === null || row.cand === null || row.base === 0) return null;
	return ((row.cand - row.base) / row.base) * 100;
}

export function row_delta(row: Row): number | null {
	if (row.base === null || row.cand === null) return null;
	return row.cand - row.base;
}

/** A row's verdict; `floor` is its A/A floor, or undefined. */
export function row_verdict(row: Row, floor: FloorEntry | undefined): RowVerdict {
	if (row.base === null) return 'added';
	if (row.cand === null) return 'removed';
	if (row.exact) return row.base === row.cand ? 'same' : 'differs';
	if (row.base === row.cand) return 'same';
	if (floor === undefined) return 'unfloored';
	return Math.abs(row_pct(row) ?? Number.POSITIVE_INFINITY) <= floor.floor
		? 'noise'
		: 'beyond_floor';
}

/** One same-ref report's measured rows, as absolute percent deltas. */
export function floors_of(report: BranchReport): Map<string, number> {
	if (!report.same_ref) {
		throw new Error('the floor report is not a same-ref run: its base and candidate differ');
	}
	const floors = new Map<string, number>();
	for (const section of report.sections) {
		for (const leg of section.legs) {
			for (const table of leg.tables) {
				for (const row of table.rows) {
					if (row.exact) continue;
					const pct = row_pct(row);
					if (pct !== null) floors.set(row_floor_key(section.id, table.id, row.key), Math.abs(pct));
				}
			}
		}
	}
	return floors;
}

/** The fields of `draw` that differ from `current`, split by which rows each one voids. */
export function floor_context_mismatches(
	draw: FloorContext,
	current: FloorContext
): { every_row: Array<string>; timing_rows: Array<string> } {
	const differs = (field: keyof FloorContext): string | null => {
		const [a, b] = [JSON.stringify(draw[field]), JSON.stringify(current[field])];
		return a === b ? null : `${field} ${a} (now ${b})`;
	};
	const present = (fields: ReadonlyArray<keyof FloorContext>): Array<string> =>
		fields.map(differs).filter((text): text is string => text !== null);
	// a draw of more legs than this run still floors the ones this run took
	const legs_covered = current.timing_legs.every((leg) => draw.timing_legs.includes(leg));
	const legs_mismatch = `timing_legs ${JSON.stringify(draw.timing_legs)} (now ${JSON.stringify(current.timing_legs)})`;
	return {
		every_row: present(['corpus_tree', 'corpus_relative_path', 'corpus_dirty', 'cpu', 'hostname']),
		timing_rows:
			current.timing_legs.length === 0
				? []
				: [
						...present(['rounds', 'iterations', 'bench_env', 'cargo_build_jobs']),
						...(legs_covered ? [] : [legs_mismatch])
					]
	};
}

const TIMING_FLOOR_PREFIXES = ['build_wall/', 'timing/'];

/**
 * The floors for this run from its A/A draws: each row's maximum across the draws valid for it. A
 * draw with no provenance, or one whose context differs, gives its affected rows no floor and
 * names why, so those rows read `unfloored` and the run raises a problem.
 */
export function combine_floors(
	draws: ReadonlyArray<{ source: FloorSource; report: BranchReport }>,
	current: FloorContext
): { floors: Map<string, FloorEntry>; mismatches: Array<string> } {
	const floors = new Map<string, FloorEntry>();
	const mismatches: Array<string> = [];
	for (const { source, report } of draws) {
		if (source.context === null) {
			mismatches.push(`${source.path}: no floor provenance; all its rows are unfloored`);
			continue;
		}
		const { every_row, timing_rows } = floor_context_mismatches(source.context, current);
		if (every_row.length > 0) {
			mismatches.push(`${source.path}: ${every_row.join(', ')}; all its rows are unfloored`);
			continue;
		}
		if (timing_rows.length > 0) {
			mismatches.push(`${source.path}: ${timing_rows.join(', ')}; its timing rows are unfloored`);
		}
		for (const [key, pct] of floors_of(report)) {
			if (timing_rows.length > 0 && TIMING_FLOOR_PREFIXES.some((p) => key.startsWith(p))) continue;
			const entry = floors.get(key);
			floors.set(key, {
				floor: Math.max(entry?.floor ?? 0, pct),
				draws: (entry?.draws ?? 0) + 1
			});
		}
	}
	return { floors, mismatches };
}

// -- section status -----------------------------------------------------------------------------

/** Worst first: a gap a person must see outranks a leg that ran. */
const STATUS_ORDER: ReadonlyArray<LegStatus> = [
	'failed',
	'missing_tool',
	'unavailable_on_ref',
	'skipped',
	'planned',
	'not_applicable',
	'not_requested',
	'ok'
];

export function section_status(legs: ReadonlyArray<Leg>): LegStatus {
	for (const status of STATUS_ORDER) {
		if (legs.some((leg) => leg.status === status)) return status;
	}
	return 'ok';
}

// -- problems and exit -----------------------------------------------------------------------------

export interface Problem {
	kind: 'gap' | 'nondeterministic' | 'floor_mismatch' | 'confounded_toolchain';
	message: string;
}

/** Every reason the run should not read as clean: gaps, and exact rows that differ on a same-ref
 * pair (a bug in the harness, or a non-determinism to name and move to a measured row). */
export function report_problems(report: BranchReport): Array<Problem> {
	const problems: Array<Problem> = [];
	for (const message of report.floor_mismatches) problems.push({ kind: 'floor_mismatch', message });
	const { base, cand } = report.provenance;
	if (base.rustc !== cand.rustc) {
		problems.push({
			kind: 'confounded_toolchain',
			message: `the sides build with different compilers, so \`.text\` is confounded: ${base.rustc} vs ${cand.rustc}`
		});
	}
	for (const section of report.sections) {
		for (const leg of section.legs) {
			if (status_is_gap(leg.status)) {
				problems.push({
					kind: 'gap',
					message: `${SECTION_LETTERS[section.id]} ${leg.name}: ${leg.status}${leg.reason ? ` — ${leg.reason}` : ''}`
				});
			}
			if (!report.same_ref) continue;
			for (const table of leg.tables) {
				for (const row of table.rows) {
					const verdict = row_verdict(row, undefined);
					if (verdict === 'differs' || verdict === 'added' || verdict === 'removed') {
						problems.push({
							kind: 'nondeterministic',
							message:
								`${SECTION_LETTERS[section.id]} ${leg.name} / ${table.title} / ${row.key}: ` +
								`${verdict} on a same-ref pair (${row.base} vs ${row.cand})`
						});
					}
				}
			}
		}
	}
	return problems;
}

/** 0 clean; 1 a same-ref difference; 2 a gap or a void comparison (unless accepted). */
export function exit_code(report: BranchReport, accept_gaps: boolean): number {
	const problems = report_problems(report);
	if (problems.some((p) => p.kind === 'nondeterministic')) return 1;
	if (problems.length > 0 && !accept_gaps) return 2;
	return 0;
}

// -- rendering ----------------------------------------------------------------------------------

export function format_quantity(value: number | null, unit: string): string {
	if (value === null) return '—';
	const shown = Number.isInteger(value) ? value.toLocaleString('en-US') : value.toFixed(2);
	return unit === '' ? shown : `${shown} ${unit}`;
}

const signed = (value: number, digits: number): string =>
	`${value > 0 ? '+' : ''}${value.toFixed(digits)}`;

function render_row(section: SectionId, table: Table, row: Row, floors: Floors): string {
	const floor = floors.get(row_floor_key(section, table.id, row.key));
	const verdict = row_verdict(row, floor);
	const verdict_text =
		verdict === 'beyond_floor' && floor?.draws === 1 ? 'beyond_floor (one draw)' : verdict;
	const delta = row_delta(row);
	const pct = row_pct(row);
	return `| ${row.key} | ${format_quantity(row.base, row.unit)} | ${format_quantity(row.cand, row.unit)} | ${
		delta === null ? '—' : signed(delta, Number.isInteger(delta) ? 0 : 2)
	} | ${pct === null ? '—' : `${signed(pct, 3)}%`} | ${verdict_text}${row.note ? ` — ${row.note}` : ''} |`;
}

/** The exact rows of a table that differ between the sides. */
function exact_rows_differing(section: SectionId, table: Table, floors: Floors): number {
	return table.rows.filter((row) => {
		if (!row.exact) return false;
		const verdict = row_verdict(row, floors.get(row_floor_key(section, table.id, row.key)));
		return verdict !== 'same';
	}).length;
}

const TABLE_HEAD =
	'| row | base | cand | Δ | Δ% | verdict |\n| --- | ---: | ---: | ---: | ---: | --- |';

function render_table(section: SectionId, table: Table, floors: Floors): string {
	const shown =
		table.display === 'changed'
			? table.rows.filter((row) => {
					const verdict = row_verdict(row, floors.get(row_floor_key(section, table.id, row.key)));
					return verdict !== 'same';
				})
			: table.rows;
	const lines = [`#### ${table.title}`, ''];
	if (table.note) lines.push(table.note, '');
	const hidden = table.rows.length - shown.length;
	if (table.display === 'changed') {
		lines.push(
			`${table.rows.length} rows, ${shown.length} differ${hidden > 0 ? ` (${hidden} identical not shown; all in the JSON)` : ''}.`,
			''
		);
	}
	if (shown.length > 0)
		lines.push(TABLE_HEAD, ...shown.map((row) => render_row(section, table, row, floors)));
	else if (table.display === 'all') lines.push('(no rows)');
	return lines.join('\n');
}

function render_leg(section: SectionId, leg: Leg, floors: Floors): string {
	const wall = leg.wall_ms === null ? '' : `, ${(leg.wall_ms / 1000).toFixed(1)} s`;
	const parts = [`### ${leg.name} — ${leg.status}${wall}`];
	if (leg.reason) parts.push(`**${leg.reason}**`);
	for (const note of leg.notes) parts.push(note);
	for (const table of leg.tables) parts.push(render_table(section, table, floors));
	if (leg.commands.length > 0) {
		parts.push(
			`<details><summary>commands</summary>\n\n\`\`\`\n${leg.commands.join('\n')}\n\`\`\`\n\n</details>`
		);
	}
	return parts.join('\n\n');
}

function render_provenance(p: Provenance): string {
	const rows: Array<[string, string]> = [
		['base', `${p.base.ref} → ${p.base.commit} (tree ${p.base.tree})`],
		['cand', `${p.cand.ref} → ${p.cand.commit} (tree ${p.cand.tree})`],
		['base rustc', p.base.rustc],
		['cand rustc', p.cand.rustc],
		['base cargo', p.base.cargo],
		['cand cargo', p.cand.cargo],
		['corpus', `${p.corpus.path} (tree ${p.corpus.tree}${p.corpus.dirty ? ', DIRTY' : ''})`],
		['CARGO_BUILD_JOBS', p.cargo_build_jobs ?? 'unset'],
		...Object.entries(p.machine).map(([k, v]): [string, string] => [k, v]),
		...Object.entries(p.tools).map(([k, v]): [string, string] => [k, v])
	];
	return [
		`Command: \`${p.command_line}\``,
		'',
		'| | |',
		'| --- | --- |',
		...rows.map(([k, v]) => `| ${k} | ${v.replaceAll('|', '\\|').replaceAll('\n', '<br>')} |`),
		'',
		`Base build tree: \`${p.base.build_dir}\``,
		`Candidate build tree: \`${p.cand.build_dir}\``,
		...(p.flags.length > 0 ? ['', `Flags: ${p.flags.join(' ')}`] : [])
	].join('\n');
}

export function render_markdown(report: BranchReport, floors: Floors): string {
	const { base, cand } = report.provenance;
	const title = report.same_ref
		? `# tsv branch report: A/A null, ${base.commit.slice(0, 10)}`
		: `# tsv branch report: ${base.commit.slice(0, 10)} → ${cand.commit.slice(0, 10)}`;
	const lines = [title, '', `Generated ${report.generated}.`, ''];

	const problems = report_problems(report);
	const exact_differs = report.sections.reduce(
		(sum, section) =>
			sum +
			section.legs.reduce(
				(leg_sum, leg) =>
					leg_sum + leg.tables.reduce((n, t) => n + exact_rows_differing(section.id, t, floors), 0),
				0
			),
		0
	);
	lines.push('## Verdict', '');
	if (report.same_ref) {
		lines.push(
			problems.some((p) => p.kind === 'nondeterministic')
				? '**A same-ref pair differs on a deterministic row.** Each is listed below; each is a bug to fix or a non-determinism to name.'
				: problems.some((p) => p.kind === 'gap')
					? 'Same ref on both sides: no deterministic row differs, but the gaps below mean this run does not qualify the report as the A/A null.'
					: 'Same ref on both sides: no deterministic row differs. This run qualifies the report as the A/A null.',
			''
		);
	}
	if (report.aa_sources.length > 0) {
		lines.push(
			`Measured rows graded against ${report.aa_sources.length} A/A draw${report.aa_sources.length === 1 ? '' : 's'}: each row's floor is its largest delta across the draws valid for it.`,
			''
		);
		for (const source of report.aa_sources) {
			lines.push(
				`- \`${source.path}\` (${source.generated}, commit ${source.commit.slice(0, 10)})`
			);
		}
		lines.push('');
	} else {
		lines.push(
			'No A/A floor was given (`--aa`): a difference in a measured row reads `unfloored`, never real.',
			''
		);
	}
	lines.push(
		'| section | status | deterministic tables with a difference |',
		'| --- | --- | ---: |'
	);
	for (const section of report.sections) {
		const with_diff = section.legs.reduce(
			(n, leg) =>
				n + leg.tables.filter((t) => exact_rows_differing(section.id, t, floors) > 0).length,
			0
		);
		lines.push(
			`| ${SECTION_LETTERS[section.id]}. ${SECTION_TITLES[section.id]} | ${section.status}${section.reason ? ` — ${section.reason}` : ''} | ${with_diff} |`
		);
	}
	lines.push('');
	if (problems.length > 0) {
		lines.push('### Problems', '', ...problems.map((p) => `- (${p.kind}) ${p.message}`), '');
	} else {
		lines.push('No gaps or same-ref differences.', '');
	}
	lines.push(`Deterministic rows that differ, total: ${exact_differs}.`, '');
	lines.push('## Provenance', '', render_provenance(report.provenance), '');
	for (const section of report.sections) {
		lines.push(`## ${SECTION_LETTERS[section.id]}. ${SECTION_TITLES[section.id]}`, '');
		lines.push(`Status: ${section.status}${section.reason ? ` — ${section.reason}` : ''}.`, '');
		for (const leg of section.legs) lines.push(render_leg(section.id, leg, floors), '');
	}
	return `${lines.join('\n').trimEnd()}\n`;
}
