import { deepStrictEqual, throws } from 'node:assert';
import { test } from 'vitest';

import {
	type BranchReport,
	exit_code,
	combine_floors,
	type FloorContext,
	type FloorSource,
	floors_of,
	type Leg,
	type LegStatus,
	render_markdown,
	report_problems,
	type Row,
	row_floor_key,
	row_pct,
	row_verdict,
	section_status
} from './report.ts';

function row(partial: Partial<Row> & { key: string }): Row {
	return { unit: '', base: 100, cand: 100, exact: true, ...partial };
}

test('exact rows: same, differs, added, removed', () => {
	deepStrictEqual(row_verdict(row({ key: 'a' }), undefined), 'same');
	deepStrictEqual(row_verdict(row({ key: 'a', cand: 101 }), undefined), 'differs');
	deepStrictEqual(row_verdict(row({ key: 'a', base: null }), undefined), 'added');
	deepStrictEqual(row_verdict(row({ key: 'a', cand: null }), undefined), 'removed');
});

test('a measured row is never real without a floor', () => {
	const measured = row({ key: 'wall', exact: false, cand: 105 });
	deepStrictEqual(row_verdict(measured, undefined), 'unfloored');
	deepStrictEqual(row_verdict(measured, { floor: 10, draws: 2 }), 'noise');
	deepStrictEqual(row_verdict(measured, { floor: 1, draws: 2 }), 'beyond_floor');
	deepStrictEqual(row_verdict(row({ key: 'wall', exact: false }), undefined), 'same');
});

test('row_pct', () => {
	deepStrictEqual(row_pct(row({ key: 'a', base: 200, cand: 210 })), 5);
	deepStrictEqual(row_pct(row({ key: 'a', base: 0, cand: 1 })), null);
	deepStrictEqual(row_pct(row({ key: 'a', base: null, cand: 1 })), null);
});

test('section_status is the worst leg', () => {
	const leg = (status: LegStatus): Leg => ({
		name: status,
		status,
		wall_ms: 0,
		commands: [],
		tables: [],
		notes: []
	});
	deepStrictEqual(section_status([leg('ok'), leg('failed')]), 'failed');
	deepStrictEqual(section_status([leg('ok'), leg('skipped')]), 'skipped');
	deepStrictEqual(section_status([leg('ok')]), 'ok');
});

const CONTEXT: FloorContext = {
	corpus_tree: 'tree1',
	corpus_relative_path: 'collections/',
	corpus_dirty: false,
	cpu: 'cpu',
	hostname: 'lap',
	rounds: 2,
	iterations: 10,
	timing_legs: ['plain'],
	bench_env: {},
	cargo_build_jobs: '4'
};

function report_with(
	rows: Array<Row>,
	same_ref: boolean,
	leg_status: 'ok' | 'failed' = 'ok'
): BranchReport {
	const side = (slot: 'base' | 'cand') => ({
		slot,
		ref: 'HEAD',
		commit: 'a'.repeat(40),
		tree: 'b'.repeat(40),
		build_dir: '/cache/x',
		rustc: 'rustc 1.94.1',
		cargo: 'cargo 1.94.1'
	});
	return {
		version: 1,
		generated: '2026-11-08T00:00:00.000Z',
		same_ref,
		provenance: {
			command_line: 'npm run branch:report -- HEAD HEAD',
			base: side('base'),
			cand: side('cand'),
			machine: { hostname: 'lap' },
			tools: { deno: '2.0' },
			corpus: { path: '/c', tree: 't', dirty: false, relative_path: 'collections/' },
			cargo_build_jobs: '4',
			flags: []
		},
		floor_context: CONTEXT,
		aa_sources: [],
		floor_mismatches: [],
		sections: [
			{
				id: 'sizes',
				status: leg_status,
				legs: [
					{
						name: 'native sizes',
						status: leg_status,
						wall_ms: 1000,
						commands: ['size -A tsv'],
						tables: [{ id: 'native_files', title: 'Native files', rows, display: 'all' }],
						notes: []
					}
				]
			}
		]
	};
}

test('a same-ref difference on a deterministic row is a nondeterministic problem and exit 1', () => {
	const report = report_with([row({ key: 'cli', cand: 101 })], true);
	const problems = report_problems(report);
	deepStrictEqual(problems.length, 1);
	deepStrictEqual(problems[0]!.kind, 'nondeterministic');
	deepStrictEqual(exit_code(report, false), 1);
	// no flag accepts it: a same-ref difference is a bug, not a gap
	deepStrictEqual(exit_code(report, true), 1);
});

test('a difference on a real pair is no problem at all', () => {
	const report = report_with([row({ key: 'cli', cand: 101 })], false);
	deepStrictEqual(report_problems(report), []);
	deepStrictEqual(exit_code(report, false), 0);
});

test('a failed leg is a gap: exit 2 unless accepted', () => {
	const report = report_with([], false, 'failed');
	deepStrictEqual(report_problems(report)[0]!.kind, 'gap');
	deepStrictEqual(exit_code(report, false), 2);
	deepStrictEqual(exit_code(report, true), 0);
});

test('floors_of keys measured rows only, and refuses a real pair', () => {
	const rows = [row({ key: 'wall', exact: false, cand: 102 }), row({ key: 'size', cand: 101 })];
	const same = report_with(rows, true);
	const floors = floors_of(same);
	deepStrictEqual(floors.size, 1);
	deepStrictEqual(floors.get(row_floor_key('sizes', 'native_files', 'wall')), 2);
	throws(() => floors_of(report_with(rows, false)));
});

test('the markdown states a same-ref qualification and every section', () => {
	const report = report_with([row({ key: 'cli' })], true);
	const text = render_markdown(report, new Map());
	deepStrictEqual(text.startsWith('# tsv branch report: A/A null,'), true);
	deepStrictEqual(text.includes('no deterministic row differs'), true);
	deepStrictEqual(text.includes('## A. Sizes'), true);
	deepStrictEqual(
		text.includes('| cli | 100 | 100 | +0'.replace('+0', '0')) ||
			text.includes('| cli | 100 | 100 |'),
		true
	);
});

test('a real pair with no floor marks a measured difference unfloored in the document', () => {
	const report = report_with([row({ key: 'wall', exact: false, cand: 110 })], false);
	const text = render_markdown(report, new Map());
	deepStrictEqual(text.includes('unfloored'), true);
	deepStrictEqual(text.includes('beyond_floor'), false);
});

const measured = (key: string, cand: number): Row => row({ key, exact: false, cand });

/** A same-ref draw holding `rows` in its sizes table and a timing-section row. */
function draw_of(
	rows: Array<Row>,
	context: FloorContext | null,
	timing_cand: number
): { source: FloorSource; report: BranchReport } {
	const report = report_with(rows, true);
	report.sections.push({
		id: 'timing',
		status: 'ok',
		legs: [
			{
				name: 'plain',
				status: 'ok',
				wall_ms: 1,
				commands: [],
				notes: [],
				tables: [{ id: 't', title: 'T', rows: [measured('wall', timing_cand)], display: 'all' }]
			}
		]
	});
	return {
		report,
		source: { path: '/aa.json', generated: 'g', commit: 'c'.repeat(40), context }
	};
}

const SIZES_WALL = row_floor_key('sizes', 'native_files', 'rss');
const TIMING_WALL = row_floor_key('timing', 't', 'wall');

test("combine_floors takes each row's maximum across draws and counts them", () => {
	const { floors, mismatches } = combine_floors(
		[draw_of([measured('rss', 102)], CONTEXT, 101), draw_of([measured('rss', 105)], CONTEXT, 103)],
		CONTEXT
	);
	deepStrictEqual(mismatches, []);
	deepStrictEqual(floors.get(SIZES_WALL), { floor: 5, draws: 2 });
	deepStrictEqual(floors.get(TIMING_WALL), { floor: 3, draws: 2 });
});

test('a floor from another commit is valid: the commit is not part of its context', () => {
	const draw = draw_of([measured('rss', 102)], CONTEXT, 101);
	draw.source.commit = 'd'.repeat(40);
	deepStrictEqual(combine_floors([draw], CONTEXT).mismatches, []);
});

for (const [field, value] of [
	['corpus_tree', 'other'],
	['corpus_relative_path', 'collections/zzz/'],
	['corpus_dirty', true],
	['cpu', 'other'],
	['hostname', 'desk']
] as const) {
	test(`a ${field} mismatch voids the whole draw and names it`, () => {
		const draw = draw_of([measured('rss', 102)], { ...CONTEXT, [field]: value }, 101);
		const { floors, mismatches } = combine_floors([draw], CONTEXT);
		deepStrictEqual(floors.size, 0);
		deepStrictEqual(mismatches.length, 1);
		deepStrictEqual(mismatches[0]!.includes(field), true);
	});
}

for (const [field, value] of [
	['rounds', 4],
	['iterations', 1],
	['timing_legs', ['pgo']],
	['bench_env', { BENCH_LIMIT: '3' }],
	['cargo_build_jobs', '8']
] as const) {
	test(`a ${field} mismatch voids the timing rows and keeps the others`, () => {
		const draw = draw_of([measured('rss', 102)], { ...CONTEXT, [field]: value }, 101);
		const { floors, mismatches } = combine_floors([draw], CONTEXT);
		deepStrictEqual(floors.has(TIMING_WALL), false);
		deepStrictEqual(floors.get(SIZES_WALL), { floor: 2, draws: 1 });
		deepStrictEqual(mismatches[0]!.includes(field), true);
	});
}

test('a draw with no provenance gives no floors', () => {
	const { floors, mismatches } = combine_floors(
		[draw_of([measured('rss', 102)], null, 101)],
		CONTEXT
	);
	deepStrictEqual(floors.size, 0);
	deepStrictEqual(mismatches[0]!.includes('no floor provenance'), true);
});

test('a floor mismatch is a problem: exit 2 unless accepted, and the row reads unfloored', () => {
	const report = report_with([measured('rss', 110)], false);
	report.floor_mismatches = ['/aa.json: hostname "desk" (now "lap"); all its rows are unfloored'];
	deepStrictEqual(report_problems(report)[0]!.kind, 'floor_mismatch');
	deepStrictEqual(exit_code(report, false), 2);
	deepStrictEqual(exit_code(report, true), 0);
	deepStrictEqual(render_markdown(report, new Map()).includes('unfloored'), true);
});

test('differing compilers confound the pair: a problem, exit 2 unless accepted', () => {
	const report = report_with([], false);
	report.provenance.cand.rustc = 'rustc 1.99.1';
	deepStrictEqual(report_problems(report)[0]!.kind, 'confounded_toolchain');
	deepStrictEqual(exit_code(report, false), 2);
	deepStrictEqual(exit_code(report, true), 0);
});

test('beyond_floor says when it rests on one draw', () => {
	const report = report_with([measured('rss', 110)], false);
	const key = SIZES_WALL;
	const one = render_markdown(report, new Map([[key, { floor: 2, draws: 1 }]]));
	deepStrictEqual(one.includes('beyond_floor (one draw)'), true);
	const two = render_markdown(report, new Map([[key, { floor: 2, draws: 2 }]]));
	deepStrictEqual(two.includes('beyond_floor (one draw)'), false);
	deepStrictEqual(two.includes('beyond_floor'), true);
});

test('the markdown counts the A/A draws', () => {
	const report = report_with([], false);
	report.aa_sources = [
		{ path: '/a.json', generated: 'g', commit: 'c'.repeat(40), context: CONTEXT },
		{ path: '/b.json', generated: 'g', commit: 'c'.repeat(40), context: CONTEXT }
	];
	deepStrictEqual(render_markdown(report, new Map()).includes('2 A/A draws'), true);
});

test('a run with no timing legs takes no timing-row mismatch at all', () => {
	const current = { ...CONTEXT, timing_legs: [] };
	const draw = draw_of([measured('rss', 102)], { ...CONTEXT, rounds: 6 }, 101);
	const { floors, mismatches } = combine_floors([draw], current);
	deepStrictEqual(mismatches, []);
	deepStrictEqual(floors.has(SIZES_WALL), true);
});

test('a draw covers the current timing legs when it holds at least them', () => {
	const current = { ...CONTEXT, timing_legs: ['plain'] };
	const covering = draw_of([], { ...CONTEXT, timing_legs: ['pgo', 'plain'] }, 101);
	const covered = combine_floors([covering], current);
	deepStrictEqual(covered.mismatches, []);
	deepStrictEqual(covered.floors.has(TIMING_WALL), true);

	const short = draw_of([], { ...CONTEXT, timing_legs: ['plain'] }, 101);
	const wider = combine_floors([short], { ...CONTEXT, timing_legs: ['pgo', 'plain'] });
	deepStrictEqual(wider.mismatches[0]!.includes('timing_legs'), true);
	deepStrictEqual(wider.floors.has(TIMING_WALL), false);
});

test('the A/A qualification line prints only without gaps', () => {
	const clean = render_markdown(report_with([row({ key: 'cli' })], true), new Map());
	deepStrictEqual(clean.includes('qualifies the report as the A/A null'), true);
	const gapped = render_markdown(report_with([row({ key: 'cli' })], true, 'failed'), new Map());
	deepStrictEqual(gapped.includes('qualifies the report as the A/A null'), false);
	deepStrictEqual(gapped.includes('does not qualify'), true);
});
