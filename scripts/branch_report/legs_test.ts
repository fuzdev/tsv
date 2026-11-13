/**
 * The branch report's legs over fakes (`report_fakes.ts`): each leg's missing-tool path, which
 * starts nothing; a built program made through the cache only once its build ran, by its name
 * beneath the cache, and handed to the tool it runs under by path; the bench leg's runner per
 * side and its shared installs; the `perf:ab` child that prints no report; and a whole report
 * assembled from fake measurements. No cargo, no bench, no `perf` runs.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';

import { fs, type FsDir } from '@fuzdev/gro/fs.ts';
import { test } from 'vitest';

import { type BranchReportDeps, run_branch_report_command } from './branch_report.ts';
import {
	type Artifacts,
	leg_allocations,
	leg_native_sizes,
	leg_rss,
	leg_type_sizes_host,
	leg_wasm_memory,
	leg_wasm_sizes
} from './legs.ts';
import { type FakeCall, fake_host, fake_program, fake_side } from './report_fakes.ts';
import type { Leg } from './report.ts';
import type { ReportHost, Side } from './side.ts';
import { leg_bench, leg_pgo, leg_plain, leg_small_files, type TimingOptions } from './timing.ts';
import { BOARD_SOURCE } from './wasm_board.ts';

const artifacts_of = (side: Side): Artifacts => ({
	cli: `${side.name}/target/release/tsv`,
	addon: `${side.name}/target/napi/libtsv_napi.so`,
	cli_symbols: `${side.name}/target_symbols/release/tsv`,
	addon_symbols: `${side.name}/target_symbols/napi/libtsv_napi.so`,
	wasm: {
		format: `${side.name}/wasm/format-x/tsv_wasm_bg.wasm`,
		parse: `${side.name}/wasm/parse-x/tsv_wasm_bg.wasm`,
		all: `${side.name}/wasm/all-x/tsv_wasm_bg.wasm`
	}
});

const timing_options = (overrides: Partial<TimingOptions> = {}): TimingOptions => ({
	legs: new Set(),
	rounds: 2,
	iterations: 10,
	corpus: '/corpora/collections',
	dry_run: false,
	bench_runtimes: ['node'],
	bench_env: {},
	...overrides
});

interface World {
	cache: FsDir;
	checkout: FsDir;
	calls: Array<FakeCall>;
	sides: readonly [Side, Side];
	artifacts: readonly [Artifacts, Artifacts];
	[Symbol.asyncDispose]: () => Promise<void>;
}

const world = async (files: Readonly<Record<string, string>> = {}): Promise<World> => {
	const cache = await fs.temp_dir('branch_report_cache-');
	const checkout = await fs.temp_dir('branch_report_checkout-');
	const sides = [
		await fake_side(cache, 'base', files),
		await fake_side(cache, 'cand', files)
	] as const;
	return {
		cache,
		checkout,
		calls: [],
		sides,
		artifacts: [artifacts_of(sides[0]), artifacts_of(sides[1])],
		[Symbol.asyncDispose]: async () => {
			await cache[Symbol.asyncDispose]();
			await checkout[Symbol.asyncDispose]();
		}
	};
};

test('each leg whose tool is absent is missing_tool, its line naming the tool, and starts nothing', async () => {
	await using w = await world({ 'scripts/pgo/pgo.ts': '' });
	const host = fake_host(w.cache, w.checkout, w.calls);
	const { sides, artifacts } = w;
	const legs: Array<[Promise<Leg>, string]> = [
		[leg_wasm_sizes(host, 'WASM bundle bytes', sides, artifacts), 'gzip'],
		[leg_native_sizes(host, 'native', sides, artifacts), 'nm'],
		[leg_allocations(host, sides, artifacts, '/corpus', 'scratch.1'), 'heaptrack'],
		[leg_rss(host, sides, artifacts, '/corpus'), '/usr/bin/time'],
		[leg_wasm_memory(host, sides, artifacts, '/corpus', 'scratch.1'), 'node'],
		[leg_plain(host, sides, artifacts, timing_options()), 'perf'],
		[leg_pgo(host, sides, timing_options()), 'perf']
	];
	for (const [pending, tool] of legs) {
		const leg = await pending;
		strictEqual(leg.status, 'missing_tool', leg.name);
		strictEqual(leg.reason, `\`${tool}\` is not installed`, leg.name);
	}
	deepStrictEqual(w.calls, []);
});

test('a built program is made through the cache only after its build, by its name beneath the cache', async () => {
	await using w = await world({ [BOARD_SOURCE]: '' });
	const events: Array<string> = [];
	const board = JSON.stringify({
		types: [{ group: 'ast', name: 'Node', size: 8, align: 8, result_size: 16 }]
	});
	const host = fake_host(w.cache, w.checkout, w.calls, {
		build: (call) => void events.push(`build ${call.args.slice(5).join(' ')} in ${call.cwd}`),
		on_side_program: (name) => events.push(`program ${name}`),
		side: () => ({ stdout: board })
	});
	const leg = await leg_type_sizes_host(host, w.sides);
	strictEqual(leg.status, 'ok');
	deepStrictEqual(events, [
		`build cargo build --locked --profile corpus -p tsv_debug in ${w.sides[0].src}`,
		`program ${w.sides[0].name}/target/corpus/tsv_debug`,
		`build cargo build --locked --profile corpus -p tsv_debug in ${w.sides[1].src}`,
		`program ${w.sides[1].name}/target/corpus/tsv_debug`
	]);
	// each side's own binary ran, from the checkout
	deepStrictEqual(
		w.calls.filter((c) => c.program !== 'nice').map((c) => [c.program, c.args, c.cwd]),
		w.sides.map((s) => [
			`${s.name}/target/corpus/tsv_debug`,
			['type_sizes', '--json'],
			w.checkout.path
		])
	);
	// each build gets its side's own target directory
	deepStrictEqual(
		w.calls.filter((c) => c.program === 'nice').map((c) => c.env),
		w.sides.map((s) => ({ CARGO_TARGET_DIR: s.target }))
	);
	strictEqual(leg.tables[0]!.rows.length, 2);
});

test('the tool a built program runs under is handed its path beneath the cache', async () => {
	await using w = await world();
	const tool_calls: Array<FakeCall> = [];
	const time = fake_program('time', tool_calls, () => ({ stderr: '2048\n' }), {
		path: '/usr/bin/time'
	});
	const host: ReportHost = {
		...fake_host(w.cache, w.checkout, w.calls),
		tools: { ...fake_host(w.cache, w.checkout, w.calls).tools, time }
	};
	const leg = await leg_rss(host, w.sides, w.artifacts, '/corpus');
	strictEqual(leg.status, 'ok');
	deepStrictEqual(
		tool_calls.map((c) => c.args[2]),
		[0, 0, 0, 1, 1, 1].map((i) => `${w.cache.path}/${w.artifacts[i]!.cli}`)
	);
	deepStrictEqual(leg.tables[0]!.rows[0]!.base, 2048 * 1024);
	strictEqual(
		leg.commands[0],
		`/usr/bin/time -f %M ${w.cache.path}/${w.artifacts[0].cli} format --check --jobs 1 /corpus (minimum of 3)`
	);
});

/** Lay out a side's bench report the fake bench writes. */
const bench_report = (p50: number): string =>
	JSON.stringify({ entries: [{ group: 'format', name: 'tsv', runtime: 'node', p50_ns: p50 }] });

test("the bench leg runs each side's tasks with its own tree's runner, beside the shared installs", async () => {
	await using w = await world();
	// the base side from before the tasks moved, the candidate after
	await w.sides[0].src_handle.write(
		'deno.json',
		JSON.stringify({
			tasks: {
				'bench:harvest:svelte-styles': 'x',
				'build:bench': 'x',
				'bench:node:run': 'x'
			}
		})
	);
	await w.sides[1].src_handle.write(
		'package.json',
		JSON.stringify({
			scripts: { 'bench:harvest:svelte-styles': 'x', 'build:bench': 'x', 'bench:node:run': 'x' }
		})
	);
	for (const install of ['node_modules', 'benches/js/node_modules'])
		await w.checkout.mkdir(install, { recursive: true });
	for (const side of w.sides)
		await side.src_handle.mkdir('benches/js/results', { recursive: true });
	const host = fake_host(w.cache, w.checkout, w.calls, {
		build: async (call) => {
			const side = w.sides.find((s) => s.src === call.cwd)!;
			if (call.args.includes('bench:node:run')) {
				await side.src_handle.write(
					'benches/js/results/report.node.json',
					bench_report(side.slot === 'base' ? 100 : 90)
				);
			}
		},
		tools: { deno: fake_program('deno', []), npm: fake_program('npm', []) }
	});
	const leg = await leg_bench(host, w.sides, timing_options({ bench_env: { BENCH_LIMIT: '3' } }));
	strictEqual(leg.status, 'ok', leg.reason);
	const runs = w.calls.map((c) => [
		c.cwd === w.sides[0].src ? 'base' : 'cand',
		c.args.slice(5).join(' '),
		c.env
	]);
	deepStrictEqual(runs, [
		['base', 'deno task bench:harvest:svelte-styles', {}],
		['base', 'deno task build:bench', {}],
		['cand', 'npm run bench:harvest:svelte-styles', {}],
		['cand', 'npm run build:bench', {}],
		// two rounds, interleaved, each side first once
		['base', 'deno task bench:node:run --save-report', { BENCH_LIMIT: '3', BENCH_PASSES: '1' }],
		['cand', 'npm run bench:node:run -- --save-report', { BENCH_LIMIT: '3', BENCH_PASSES: '1' }],
		['cand', 'npm run bench:node:run -- --save-report', { BENCH_LIMIT: '3', BENCH_PASSES: '1' }],
		['base', 'deno task bench:node:run --save-report', { BENCH_LIMIT: '3', BENCH_PASSES: '1' }]
	]);
	for (const side of w.sides) {
		strictEqual(await side.src_handle.read_link('node_modules'), `${w.checkout.path}/node_modules`);
		strictEqual(
			await side.src_handle.read_link('benches/js/node_modules'),
			`${w.checkout.path}/benches/js/node_modules`
		);
		strictEqual(await side.dir_handle.read_link('corpora'), '/corpora');
	}
	deepStrictEqual(leg.tables[0]!.rows, [
		{ key: 'node format tsv p50', unit: 'ns', base: 100, cand: 90, exact: false }
	]);
	ok(leg.notes[0]!.startsWith('NOT A MEASUREMENT'));
	// the two eras' runners differ for every task, and the leg says so per task
	deepStrictEqual(
		leg.notes.slice(1).map((n) => n.split(' runs as')[0]),
		['bench:harvest:svelte-styles', 'build:bench', 'bench:node:run'].map(
			(t) => `RUNNERS DIFFER: ${t}`
		)
	);
});

test('the bench leg reports a task a side lacks as not run, a missing install as failed', async () => {
	await using w = await world({ 'package.json': JSON.stringify({ scripts: {} }) });
	const host = fake_host(w.cache, w.checkout, w.calls, { tools: { npm: fake_program('npm', []) } });
	const planned = await leg_bench(host, w.sides, timing_options({ dry_run: true }));
	strictEqual(planned.status, 'unavailable_on_ref');
	ok(planned.reason!.includes('has no task bench:harvest:svelte-styles'));
	const no_install = await leg_bench(host, w.sides, timing_options());
	strictEqual(no_install.status, 'failed');
	ok(no_install.reason!.includes(`${w.checkout.path}/node_modules is absent`));
	deepStrictEqual(w.calls, []);
});

test('the bench leg names no runner when both trees run a task alike, and finds a missing task before any build', async () => {
	// a tree after the tasks moved: the Deno row stays a deno.json task, the rest are npm scripts
	const files = {
		'deno.json': JSON.stringify({ tasks: { 'bench:deno:run': 'x' } }),
		'package.json': JSON.stringify({
			scripts: { 'bench:harvest:svelte-styles': 'x', 'build:bench': 'x', 'bench:node:run': 'x' }
		})
	};
	await using w = await world(files);
	for (const install of ['node_modules', 'benches/js/node_modules'])
		await w.checkout.mkdir(install, { recursive: true });
	const host = fake_host(w.cache, w.checkout, w.calls, {
		build: async (call) => {
			const side = w.sides.find((s) => s.src === call.cwd)!;
			const rt = call.args.includes('bench:deno:run') ? 'deno' : 'node';
			await side.src_handle.write(`benches/js/results/report.${rt}.json`, bench_report(100));
		},
		tools: { deno: fake_program('deno', []), npm: fake_program('npm', []) }
	});
	for (const side of w.sides)
		await side.src_handle.mkdir('benches/js/results', { recursive: true });
	const alike = await leg_bench(
		host,
		w.sides,
		timing_options({ bench_runtimes: ['deno', 'node'] })
	);
	strictEqual(alike.status, 'ok', alike.reason);
	deepStrictEqual(alike.notes, []);
	// the candidate's tree lacks the Node row: the leg stops before either side builds
	w.calls.length = 0;
	await w.sides[1].src_handle.write(
		'package.json',
		JSON.stringify({ scripts: { 'bench:harvest:svelte-styles': 'x', 'build:bench': 'x' } })
	);
	const lacking = await leg_bench(
		host,
		w.sides,
		timing_options({ bench_runtimes: ['deno', 'node'] })
	);
	strictEqual(lacking.status, 'unavailable_on_ref');
	ok(lacking.reason!.includes('cand topic has no task bench:node:run'), lacking.reason);
	deepStrictEqual(w.calls, []);
});

test('a perf:ab child that prints no report fails the leg on its exit, never on a JSON parse', async () => {
	await using w = await world();
	const host = fake_host(w.cache, w.checkout, w.calls, {
		tools: {
			perf: fake_program('perf', []),
			perf_ab: fake_program('node', w.calls, () => ({ code: 1, stdout: '' }))
		}
	});
	const leg = await leg_plain(host, w.sides, w.artifacts, timing_options());
	strictEqual(leg.status, 'failed');
	strictEqual(leg.reason, 'perf:ab exited 1 and printed no JSON report');
	deepStrictEqual(w.calls[0]!.args.slice(0, 1), ['scripts/roots/perf_ab.ts']);
	deepStrictEqual(w.calls[0]!.args.slice(-3), ['--cache', `${w.cache.path}/perf_ab`, '--json']);
});

test('a dry run plans each perf:ab child with the argv a run passes, the cache path spelled out', async () => {
	await using w = await world();
	const host = fake_host(w.cache, w.checkout, w.calls);
	const options = timing_options({ dry_run: true });
	const refs = `${'b'.repeat(40)} ${'c'.repeat(40)} --rounds 2 --iterations 10`;
	const plain = await leg_plain(host, w.sides, w.artifacts, options);
	strictEqual(plain.status, 'planned');
	strictEqual(
		plain.commands[0],
		`node scripts/roots/perf_ab.ts ${refs} --corpus /corpora/collections --cache ${w.cache.path}/perf_ab --json`
	);
	const small = await leg_small_files(host, w.sides, w.artifacts, options);
	strictEqual(small.status, 'planned');
	deepStrictEqual(small.commands, [
		`stage every corpus file under 400 bytes into ${w.cache.path}/small_corpus`,
		`node scripts/roots/perf_ab.ts ${refs} --corpus ${w.cache.path}/small_corpus --cache ${w.cache.path}/perf_ab --json`
	]);
	deepStrictEqual(w.calls, []);
});

test('a whole report is assembled from fake measurements, its gaps listed and graded', async () => {
	await using cache = await fs.temp_dir('branch_report_cache-');
	await using checkout = await fs.temp_dir('branch_report_checkout-');
	await using corpus = await fs.temp_dir('branch_report_corpus-');
	const calls: Array<FakeCall> = [];
	const tree = 'd'.repeat(40);
	const board = JSON.stringify({
		types: [{ group: 'ast', name: 'Node', size: 8, align: 8, result_size: 16 }]
	});
	const base_host = fake_host(cache, checkout, calls, { side: () => ({ stdout: board }) });
	const git = fake_program('git', calls, (call) => {
		const [verb, a1, a2, a3] = call.args;
		if (verb === 'rev-parse' && a1 === '--verify') return { stdout: `${'1'.repeat(40)}\n` };
		if (verb === 'rev-parse') return { stdout: `${tree}\n` };
		if (a1 === '-C') return a3 === 'status' ? { stdout: '' } : { code: 128 };
		void a2;
		return {};
	});
	// both sides' trees already extracted, so nothing reaches git archive
	for (const slot of ['base', 'cand'] as const) {
		await fake_side(cache, slot, { 'Cargo.toml': '', [BOARD_SOURCE]: '' }, tree);
	}
	const deps: BranchReportDeps = {
		...base_host,
		git,
		tools: {
			...base_host.tools,
			rustc: fake_program('rustc', calls, () => ({ stdout: 'rustc 1\n' })),
			cargo: fake_program('cargo', calls, () => ({ stdout: 'cargo 1\n' }))
		},
		home: '/home/nobody',
		cargo_build_jobs: undefined,
		bench_env: {},
		open_cache: async () => base_host.cache,
		open_made_dir: async () => cache,
		load_1m: () => 0.25
	};
	const args = ['main', 'main', '--sections', 'b', '--corpus', corpus.path, '--cache', cache.path];
	strictEqual(await run_branch_report_command(deps, args), 2);
	const report = JSON.parse(
		await cache.read_text(
			[...(await Array.fromAsync(cache.list('reports')))].map(
				(e) => `reports/${e.name}/report.json`
			)[0]!
		)
	);
	strictEqual(report.provenance.command_line, `npm run branch:report -- ${args.join(' ')}`);
	strictEqual(report.same_ref, true);
	deepStrictEqual(
		report.sections.map((s: { id: string; status: string }) => [s.id, s.status]),
		[
			['sizes', 'not_requested'],
			['types', 'failed'],
			['work', 'not_requested'],
			['build_wall', 'not_requested'],
			['timing', 'not_requested'],
			['store_rows', 'not_applicable']
		]
	);
	const types = report.sections[1].legs;
	strictEqual(types[0].status, 'ok');
	deepStrictEqual(types[0].tables[0].rows[0], {
		key: 'ast Node',
		unit: 'B',
		base: 8,
		cand: 8,
		exact: true
	});
	// an empty board source builds no board: a gap, so the run exits 2 unless gaps are accepted
	strictEqual(types[1].status, 'failed');
	ok(types[1].reason.includes('the board source has no'));
	strictEqual(await run_branch_report_command(deps, [...args, '--accept-gaps']), 0);
	// the scratch directory is gone
	strictEqual(await cache.exists('scratch.4242'), false);
});
