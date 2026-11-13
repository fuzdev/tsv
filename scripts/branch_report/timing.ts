/**
 * The timing legs (sections D and E), each off unless `--timing` names it: build wall, the plain
 * interleaved pair, the PGO interleaved pair and the bench reports. A leg's whole command plan
 * is built before anything runs, so `--dry-run` prints it without touching the machine; every
 * row here is measured, so `report.ts` grades it against an A/A floor or marks it unfloored.
 *
 * Interleaving follows the machine protocol: each round the two sides run back to back, the
 * first side swapped every round, an even round count.
 *
 * @module
 */

import { dirname } from 'node:path';

import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';

import { exit_code_of, format_ending, HarnessError } from '../git_snapshot.ts';
import { parse_perf_stat_counter } from '../perf_ab_verdict.ts';
import type { Artifacts } from './legs.ts';
import { run_leg, whole_output } from './legs.ts';
import type { Leg, Row } from './report.ts';
import {
	cache_path,
	CommandLog,
	open_dir_path,
	type ReportHost,
	require_tool,
	run_build,
	run_side_task,
	type Side,
	side_task,
	UnavailableOnRef
} from './side.ts';

export const TIMING_LEGS = ['build-wall', 'plain', 'pgo', 'bench', 'small-files'] as const;
export type TimingLeg = (typeof TIMING_LEGS)[number];

export interface TimingOptions {
	legs: ReadonlySet<TimingLeg>;
	/** Even: each side goes first equally often. */
	rounds: number;
	iterations: number;
	corpus: string;
	dry_run: boolean;
	/** The bench runtimes, for the bench leg. */
	bench_runtimes: ReadonlyArray<'deno' | 'node' | 'bun'>;
	/** The caller's bench narrowing variables (`BENCH_NARROWING_ENV`), passed on to the bench. */
	bench_env: Readonly<Record<string, string>>;
}

/** Sub-400-byte sources: the class the arc tracks separately. */
const SMALL_FILE_LIMIT = 400;

/** The median, or null for no samples, so a one-sided row reads added / removed and never NaN. */
export function median(values: ReadonlyArray<number>): number | null {
	if (values.length === 0) return null;
	const sorted = [...values].sort((a, b) => a - b);
	const mid = Math.floor(sorted.length / 2);
	return sorted.length % 2 ? sorted[mid]! : (sorted[mid - 1]! + sorted[mid]!) / 2;
}

/** The sides in the round's order (base first on even rounds, cand first on odd). */
function round_order(round: number): readonly [0 | 1, 0 | 1] {
	return round % 2 === 0 ? [0, 1] : [1, 0];
}

/** Paired rows from per-side per-round samples, medians on each side. */
export function measured_rows(
	key: string,
	unit: string,
	samples: readonly [ReadonlyArray<number>, ReadonlyArray<number>]
): Row {
	return { key, unit, base: median(samples[0]), cand: median(samples[1]), exact: false };
}

/** A row needs its instruction count on both sides: without one, `perf` counted nothing there. */
export function assert_instruction_samples(
	samples: readonly [ReadonlyArray<number>, ReadonlyArray<number>],
	label: string
): void {
	if (samples[0].length === 0 || samples[1].length === 0) {
		throw new HarnessError(
			`${label}: perf stat counted no instructions on the ${samples[0].length === 0 ? 'base' : 'candidate'} side`
		);
	}
}

/** The plan for a leg under `--dry-run`: record its commands and mark it planned. */
function planned(leg: Leg, log: CommandLog, plan: ReadonlyArray<string>): void {
	for (const line of plan) log.record(line);
	leg.status = 'planned';
	leg.reason = 'dry run: nothing was executed';
}

// -- plain and PGO pairs ------------------------------------------------------------------------

interface CliSample {
	instructions: number | null;
	cycles: number | null;
	wall_ms: number;
}

async function perf_stat_cli(
	host: ReportHost,
	perf: SpawnProgram,
	cli: SpawnProgram,
	args: ReadonlyArray<string>
): Promise<CliSample> {
	const stat_file = `perf_stat.${host.pid}.csv`;
	const started = performance.now();
	const output = await perf.out(
		[
			'stat',
			'-x,',
			'-e',
			'instructions:u,cycles:u',
			'-o',
			cache_path(host, stat_file),
			'--',
			cli.path,
			...args
		],
		{ cwd: host.checkout }
	);
	const wall_ms = performance.now() - started;
	const code = exit_code_of(output);
	if (code !== 0 && code !== 1) {
		throw new HarnessError(
			`perf stat ${cli.path} ${format_ending(output)}: ${output.stderr.trim()}`
		);
	}
	const text = await host.cache.dir.read_text(stat_file);
	await host.cache.dir.remove(stat_file, { missing_ok: true });
	return {
		instructions: parse_perf_stat_counter(text, 'instructions'),
		cycles: parse_perf_stat_counter(text, 'cycles'),
		wall_ms
	};
}

/** Interleaved `perf stat` rounds of two CLI binaries over per-language file lists. */
async function cli_pair_rows(
	host: ReportHost,
	label: string,
	clis: readonly [SpawnProgram, SpawnProgram],
	file_lists: ReadonlyArray<{ language: string; paths: ReadonlyArray<string> }>,
	options: TimingOptions,
	log: CommandLog
): Promise<Array<Row>> {
	const perf = require_tool(host, 'perf');
	const rows: Array<Row> = [];
	for (const { language, paths } of file_lists) {
		if (paths.length === 0) continue;
		const instr: [Array<number>, Array<number>] = [[], []];
		const cycles: [Array<number>, Array<number>] = [[], []];
		const wall: [Array<number>, Array<number>] = [[], []];
		log.record(
			`[${label}/${language}] perf stat -e instructions:u,cycles:u -- <cli> format --check --jobs 1 <${paths.length} files>, ${options.rounds} interleaved rounds`
		);
		for (let round = 0; round < options.rounds; round++) {
			for (const i of round_order(round)) {
				const sample = await perf_stat_cli(host, perf, clis[i], [
					'format',
					'--check',
					'--jobs',
					'1',
					...paths
				]);
				if (sample.instructions !== null) instr[i].push(sample.instructions);
				if (sample.cycles !== null) cycles[i].push(sample.cycles);
				wall[i].push(sample.wall_ms);
			}
		}
		assert_instruction_samples(instr, `${label} ${language}`);
		rows.push(measured_rows(`${label} ${language} instructions:u`, '', instr));
		if (cycles[0].length > 0 || cycles[1].length > 0) {
			rows.push(measured_rows(`${label} ${language} cycles:u`, '', cycles));
		}
		rows.push(measured_rows(`${label} ${language} wall`, 'ms', wall));
	}
	return rows;
}

/** The files a CLI discovers under `corpus`, one path per entry. */
async function list_files(
	host: ReportHost,
	cli: SpawnProgram,
	corpus: string
): Promise<Array<string>> {
	const listing = whole_output(
		await cli.out(['format', '--list', corpus], { cwd: host.checkout }),
		'tsv format --list'
	);
	if (exit_code_of(listing) !== 0) {
		throw new HarnessError(`tsv format --list ${format_ending(listing)}`);
	}
	return listing.stdout.split('\n').filter((path) => path !== '');
}

/** Group a CLI's discovered files by language for the per-language CLI rows. */
async function language_lists(
	host: ReportHost,
	cli: SpawnProgram,
	corpus: string
): Promise<Array<{ language: string; paths: Array<string> }>> {
	const groups: Record<string, Array<string>> = { typescript: [], svelte: [], css: [] };
	for (const path of await list_files(host, cli, corpus)) {
		const lang = path.endsWith('.svelte') ? 'svelte' : path.endsWith('.css') ? 'css' : 'typescript';
		groups[lang]!.push(path);
	}
	return Object.entries(groups).map(([language, paths]) => ({ language, paths }));
}

/** Where `staged_small_corpus` stages, beneath the cache. */
const SMALL_CORPUS = 'small_corpus';

/** The smallest corpus class: every listed file under `SMALL_FILE_LIMIT` bytes, staged under
 * one root so the plain driver and the CLI both read it (a staged directory, listed itself);
 * its absolute path. Each file is read beneath the corpus's handle. */
async function staged_small_corpus(
	host: ReportHost,
	cli: SpawnProgram,
	corpus: string
): Promise<string> {
	const files = await list_files(host, cli, corpus);
	const source: FsReadDir = await open_dir_path(host.fs_paths, corpus);
	const cache = host.cache.dir;
	await cache.remove(SMALL_CORPUS, { recursive: true, missing_ok: true });
	await cache.mkdir(SMALL_CORPUS, { recursive: true });
	const prefix = `${corpus.replace(/\/+$/, '')}/`;
	for (const path of files) {
		if (!path.startsWith(prefix)) {
			throw new HarnessError(`tsv format --list printed ${path}, which is not under ${corpus}`);
		}
		const relative = path.slice(prefix.length);
		const info = await source.stat(relative);
		if (info === null) throw new HarnessError(`listed but missing: ${path}`);
		if (info.size >= SMALL_FILE_LIMIT) continue;
		const to = `${SMALL_CORPUS}/${relative}`;
		await cache.mkdir(dirname(to), { recursive: true });
		await cache.write(to, await source.read(relative));
	}
	return cache_path(host, SMALL_CORPUS);
}

// -- perf:ab plain legs -------------------------------------------------------------------------

interface PerfAbJson {
	rows: Array<{
		lang: string;
		phase: string;
		base_wall_us: number;
		cand_wall_us: number;
		base_instructions: number | null;
		cand_instructions: number | null;
	}>;
	cycles: Record<string, { base: number; cand: number }>;
}

/** The checkout's own `perf:ab` entry, whichever refs the sides are. */
export const PERF_AB_ENTRY = 'scripts/roots/perf_ab.ts';

/** Every chunk of `stream`, decoded. */
async function read_text(stream: AsyncIterable<Uint8Array>): Promise<string> {
	const decoder = new TextDecoder();
	let text = '';
	for await (const chunk of stream) text += decoder.decode(chunk, { stream: true });
	return text + decoder.decode();
}

async function run_perf_ab(
	host: ReportHost,
	sides: readonly [Side, Side],
	corpus: string,
	options: TimingOptions,
	log: CommandLog
): Promise<PerfAbJson> {
	const node = require_tool(host, 'perf_ab');
	const args = [
		PERF_AB_ENTRY,
		sides[0].commit,
		sides[1].commit,
		'--rounds',
		String(options.rounds),
		'--iterations',
		String(options.iterations),
		'--corpus',
		corpus,
		'--cache',
		cache_path(host, 'perf_ab'),
		'--json'
	];
	log.record(`node ${args.join(' ')}`);
	const child = await node.start(args, {
		cwd: host.checkout,
		stdio: ['ignore', 'pipe', 'inherit']
	});
	const stdout = await read_text(child.stdout!);
	const result = await child.closed;
	const code = exit_code_of(result);
	// perf:ab exits 1 on a regression verdict, which this report does not grade; 2 is a harness error
	if (code !== 0 && code !== 1) throw new HarnessError(`perf:ab ${format_ending(result)}`);
	try {
		return JSON.parse(stdout) as PerfAbJson;
	} catch {
		// a run that never started (no `node_modules`, say) exits 1 too, with nothing on stdout
		throw new HarnessError(`perf:ab ${format_ending(result)} and printed no JSON report`);
	}
}

function perf_ab_rows(json: PerfAbJson, label: string): Array<Row> {
	const rows: Array<Row> = [];
	for (const row of json.rows) {
		const name = `${label} ${row.lang} ${row.phase}`;
		rows.push({
			key: `${name} wall`,
			unit: 'us',
			base: row.base_wall_us,
			cand: row.cand_wall_us,
			exact: false
		});
		if (row.base_instructions !== null) {
			rows.push({
				key: `${name} instructions:u`,
				unit: '',
				base: row.base_instructions,
				cand: row.cand_instructions,
				exact: false
			});
		}
		const cycles = json.cycles[`${row.lang} ${row.phase}`];
		if (cycles)
			rows.push({
				key: `${name} cycles:u`,
				unit: '',
				base: cycles.base,
				cand: cycles.cand,
				exact: false
			});
	}
	return rows;
}

/** Phase walls via `_internal_phase_timing`: doc-build vs render (instructions split it no further). */
async function phase_wall_rows(
	host: ReportHost,
	sides: readonly [Side, Side],
	options: TimingOptions,
	log: CommandLog
): Promise<Array<Row>> {
	const binaries: Array<SpawnProgram> = [];
	for (const side of sides) {
		await run_build(
			host,
			side,
			[
				'cargo',
				'build',
				'--locked',
				'--release',
				'-p',
				'tsv_debug',
				'--features',
				'_internal_phase_timing'
			],
			log,
			{
				env: { CARGO_TARGET_DIR: `${side.dir}/target_phase` }
			}
		);
		binaries.push(host.cache.side_program(`${side.name}/target_phase/release/tsv_debug`));
	}
	type Agg = Record<string, { parse_us: number; doc_build_us: number; render_us: number }>;
	const by_lang: Record<string, Record<string, [Array<number>, Array<number>]>> = {};
	for (let round = 0; round < options.rounds; round++) {
		for (const i of round_order(round)) {
			const args = [
				'profile',
				options.corpus,
				'--json',
				'--iterations',
				String(options.iterations)
			];
			log.record(`[${sides[i]!.slot}] tsv_debug ${args.join(' ')}`);
			const output = whole_output(
				await binaries[i]!.out(args, { cwd: host.checkout }),
				'tsv_debug profile'
			);
			if (exit_code_of(output) !== 0) {
				throw new HarnessError(`tsv_debug profile ${format_ending(output)}`);
			}
			const parsed = JSON.parse(output.stdout) as { langs: Agg };
			for (const [lang, agg] of Object.entries(parsed.langs)) {
				const slot = (by_lang[lang] ??= {});
				for (const phase of ['parse_us', 'doc_build_us', 'render_us'] as const) {
					(slot[phase] ??= [[], []])[i]!.push(agg[phase]);
				}
			}
		}
	}
	const rows: Array<Row> = [];
	for (const [lang, phases] of Object.entries(by_lang)) {
		for (const [phase, samples] of Object.entries(phases)) {
			rows.push(
				measured_rows(`phase-timed ${lang} ${phase.replace('_us', '')} wall`, 'us', samples)
			);
		}
	}
	return rows;
}

// -- legs -----------------------------------------------------------------------------------------

export async function leg_plain(
	host: ReportHost,
	sides: readonly [Side, Side],
	artifacts: readonly [Artifacts, Artifacts] | null,
	options: TimingOptions
): Promise<Leg> {
	return await run_leg(
		'plain interleaved pair (perf:ab, per-phase walls, CLI)',
		async (log, leg) => {
			if (options.dry_run) {
				planned(leg, log, [
					`node ${PERF_AB_ENTRY} ${sides[0].commit} ${sides[1].commit} --rounds ${options.rounds} --iterations ${options.iterations} --corpus ${options.corpus} --cache <cache>/perf_ab --json`,
					`perf stat -e instructions:u,cycles:u -- <symbolized release tsv> format --check --jobs 1 <per-language files> (${options.rounds} interleaved rounds)`
				]);
				return;
			}
			require_tool(host, 'perf');
			if (artifacts === null)
				throw new HarnessError('the artifact builds failed; no CLI to measure');
			const json = await run_perf_ab(host, sides, options.corpus, options, log);
			leg.tables.push({
				id: 'plain_perf_ab',
				title: `perf:ab rows, ${options.rounds} rounds (the release build, no profile)`,
				rows: perf_ab_rows(json, 'plain'),
				display: 'all'
			});
			const clis = [
				host.cache.side_program(artifacts[0]!.cli),
				host.cache.side_program(artifacts[1]!.cli)
			] as const;
			const lists = await language_lists(host, clis[0], options.corpus);
			leg.tables.push({
				id: 'plain_cli',
				title: 'CLI `format --check --jobs 1`, end to end, plain release',
				rows: await cli_pair_rows(host, 'plain cli', clis, lists, options, log),
				display: 'all'
			});
		}
	);
}

/** Doc-build / render walls, from a side's `_internal_phase_timing` build of `tsv_debug`. */
export async function leg_phase_walls(
	host: ReportHost,
	sides: readonly [Side, Side],
	options: TimingOptions
): Promise<Leg> {
	return await run_leg('phase-timed doc-build / render walls', async (log, leg) => {
		if (options.dry_run) {
			planned(leg, log, [
				`cargo build --locked --release -p tsv_debug --features _internal_phase_timing (target_phase/, each side)`,
				`<each side>/target_phase/release/tsv_debug profile ${options.corpus} --json --iterations ${options.iterations} (${options.rounds} interleaved rounds)`
			]);
			return;
		}
		for (const side of sides) {
			const manifest = await side.src_handle.read_text('crates/tsv_debug/Cargo.toml');
			if (!manifest.includes('_internal_phase_timing')) {
				throw new UnavailableOnRef(
					`${side.slot} ${side.ref} has no _internal_phase_timing feature`
				);
			}
		}
		leg.tables.push({
			id: 'plain_phases',
			title: 'Doc-build / render wall (`_internal_phase_timing`, release `tsv_debug`)',
			note: 'Instructions are split parse / format by `perf:ab`; a doc-build / render instruction split has no entry point yet.',
			rows: await phase_wall_rows(host, sides, options, log),
			display: 'all'
		});
	});
}

export async function leg_small_files(
	host: ReportHost,
	sides: readonly [Side, Side],
	artifacts: readonly [Artifacts, Artifacts] | null,
	options: TimingOptions
): Promise<Leg> {
	return await run_leg(`small files (under ${SMALL_FILE_LIMIT} bytes)`, async (log, leg) => {
		if (options.dry_run) {
			planned(leg, log, [
				`stage every corpus file under ${SMALL_FILE_LIMIT} bytes into <cache>/${SMALL_CORPUS}`,
				`node ${PERF_AB_ENTRY} <base> <cand> --corpus <cache>/${SMALL_CORPUS} --rounds ${options.rounds} --json`
			]);
			return;
		}
		if (artifacts === null) throw new HarnessError('the artifact builds failed');
		const staged = await staged_small_corpus(
			host,
			host.cache.side_program(artifacts[0]!.cli),
			options.corpus
		);
		const json = await run_perf_ab(host, sides, staged, options, log);
		leg.tables.push({
			id: 'small_perf_ab',
			title: `Small-file class, ${options.rounds} rounds (plain release)`,
			rows: perf_ab_rows(json, 'small'),
			display: 'all'
		});
	});
}

/** The file the incremental build touches, in each side's tree. */
const TOUCHED = 'crates/tsv_lang/src/lib.rs';

export async function leg_build_wall(
	host: ReportHost,
	sides: readonly [Side, Side],
	options: TimingOptions
): Promise<Leg> {
	const artifacts: ReadonlyArray<{ name: string; argv: Array<string> }> = [
		{
			name: 'cli release',
			argv: ['cargo', 'build', '--locked', '--release', '-p', 'tsv_cli']
		},
		{
			name: 'addon napi',
			argv: ['cargo', 'build', '--locked', '--profile', 'napi', '-p', 'tsv_napi']
		},
		{
			name: 'tsv_debug corpus',
			argv: ['cargo', 'build', '--locked', '--profile', 'corpus', '-p', 'tsv_debug']
		}
	];
	return await run_leg('build wall, clean and incremental', async (log, leg) => {
		if (options.dry_run) {
			planned(
				leg,
				log,
				artifacts.map(
					(a) =>
						`rm -rf target_wall; ${a.argv.join(' ')} (CARGO_TARGET_DIR=target_wall); touch ${TOUCHED}; rebuild (each side, ${options.rounds} interleaved rounds)`
				)
			);
			return;
		}
		const rows: Array<Row> = [];
		for (const artifact of artifacts) {
			const clean: [Array<number>, Array<number>] = [[], []];
			const incremental: [Array<number>, Array<number>] = [[], []];
			for (let round = 0; round < options.rounds; round++) {
				for (const i of round_order(round)) {
					const side = sides[i]!;
					await side.dir_handle.remove('target_wall', { recursive: true, missing_ok: true });
					const env = { CARGO_TARGET_DIR: `${side.dir}/target_wall` };
					const c0 = performance.now();
					await run_build(host, side, artifact.argv, log, { env });
					clean[i].push(performance.now() - c0);
					await side.src_handle.set_times(TOUCHED, { mtime_ms: Date.now() });
					const i0 = performance.now();
					await run_build(host, side, artifact.argv, log, { env });
					incremental[i].push(performance.now() - i0);
					await side.dir_handle.remove('target_wall', { recursive: true, missing_ok: true });
				}
			}
			rows.push(
				measured_rows(`${artifact.name} clean`, 'ms', clean),
				measured_rows(`${artifact.name} incremental (touch tsv_lang)`, 'ms', incremental)
			);
		}
		leg.tables.push({
			id: 'build_wall',
			title: `Build wall, ${options.rounds} interleaved rounds each`,
			rows,
			display: 'all'
		});
	});
}

/** The PGO pipeline's entry in a side's tree, run under `node` for every side. */
const PGO_ENTRY = 'scripts/pgo/pgo.ts';

/** The PGO pair: `pgo.ts all` regenerates each side's profile from the same corpus and gates it
 * (non-empty, applied to every crate, byte-identical output), then the CLIs race plain perf. */
export async function leg_pgo(
	host: ReportHost,
	sides: readonly [Side, Side],
	options: TimingOptions
): Promise<Leg> {
	return await run_leg('PGO interleaved pair', async (log, leg) => {
		const corpora_root = options.corpus.replace(/\/collections\/?$/, '');
		const pgo_args = (dir: string): Array<string> => [
			'node',
			PGO_ENTRY,
			'all',
			'--ffi',
			'--no-depth',
			'--reuse',
			'--dir',
			dir,
			'--corpora',
			corpora_root
		];
		if (options.dry_run) {
			planned(leg, log, [
				...sides.map((side) => `(cd ${side.src}; ${pgo_args(`${side.dir}/pgo`).join(' ')})`),
				`perf stat -e instructions:u,cycles:u -- <pgo-dir>/target-use/release/tsv format --check --jobs 1 <per-language files> (${options.rounds} interleaved rounds)`,
				'the same over the small-file staged corpus, when small-files is requested'
			]);
			return;
		}
		require_tool(host, 'perf');
		for (const side of sides) {
			if (!(await side.src_handle.exists(PGO_ENTRY))) {
				throw new UnavailableOnRef(`${side.slot} ${side.ref} has no ${PGO_ENTRY}`);
			}
		}
		for (const side of sides) {
			await run_build(host, side, pgo_args(`${side.dir}/pgo`), log, { default_target_dir: true });
			const stamp = JSON.parse(await side.dir_handle.read_text('pgo/stamp.json')) as {
				artifacts: Record<string, { sha256: string; profile_sha256: string }>;
			};
			leg.notes.push(
				`${side.slot}: profile applied per the pipeline's gates (stamp.json): ${Object.entries(
					stamp.artifacts
				)
					.map(([w, a]) => `${w} profile ${a.profile_sha256.slice(0, 12)}`)
					.join(', ')}`
			);
		}
		const cli_name = (side: Side): string => `${side.name}/pgo/target-use/release/tsv`;
		for (const side of sides) {
			if (!(await host.cache.dir.exists(cli_name(side)))) {
				throw new HarnessError(`no PGO CLI at ${cache_path(host, cli_name(side))}`);
			}
		}
		const clis = [
			host.cache.side_program(cli_name(sides[0])),
			host.cache.side_program(cli_name(sides[1]))
		] as const;
		const lists = await language_lists(host, clis[0], options.corpus);
		leg.tables.push({
			id: 'pgo_cli',
			title: 'PGO CLI `format --check --jobs 1`, end to end',
			note: 'A profile applies only to its own build world, so the PGO pair is graded on the shipped binary (the CLI), not `tsv_debug`.',
			rows: await cli_pair_rows(host, 'pgo cli', clis, lists, options, log),
			display: 'all'
		});
		if (options.legs.has('small-files')) {
			const staged = await staged_small_corpus(host, clis[0], options.corpus);
			leg.tables.push({
				id: 'pgo_small',
				title: `PGO CLI, small-file class (under ${SMALL_FILE_LIMIT} bytes)`,
				rows: await cli_pair_rows(
					host,
					'pgo small',
					clis,
					await language_lists(host, clis[0], staged),
					options,
					log
				),
				display: 'all'
			});
		}
	});
}

/** The bench's narrowing knobs: a caller's value is passed on, and the report says so. */
export const BENCH_NARROWING_ENV = [
	'BENCH_LIMIT',
	'BENCH_FILTER',
	'BENCH_DURATION',
	'BENCH_WARMUP'
];

interface BenchEntry {
	group: string;
	name: string;
	runtime: string;
	p50_ns: number;
}

/** The installs a side's bench reads, linked from the checkout: the root's (the scripts' own
 * packages, gro among them) and the bench's. A side has neither, as git keeps no install. */
const SHARED_INSTALLS = ['node_modules', 'benches/js/node_modules'];

/** The bench reports: each side's `build:bench`, then `bench:<runtime>:run` per round (a single
 * pass each, interleaved, so the rounds are the passes). Each task runs as the side's own tree
 * runs it (`side_task`): `deno task` where its `deno.json` declares it, else `npm run`. */
export async function leg_bench(
	host: ReportHost,
	sides: readonly [Side, Side],
	options: TimingOptions
): Promise<Leg> {
	return await run_leg('bench reports, per runtime', async (log, leg) => {
		const checkout = host.checkout.path;
		const run_task = (rt: string): string => `bench:${rt}:run`;
		if (options.dry_run) {
			const plan: Array<string> = [
				`symlink ${SHARED_INSTALLS.map((p) => `${checkout}/${p}`).join(' and ')} into each side's tree (the shared installs; a missing install fails this leg)`,
				`symlink <side>/corpora to the corpus root, which every bench path reads as ../corpora`
			];
			for (const side of sides) {
				const harvest = await side_task(side, 'bench:harvest:svelte-styles');
				const build = await side_task(side, 'build:bench');
				plan.push(
					`[${side.slot}] (cd <side>/src; ${harvest.argv.join(' ')} && ${build.argv.join(' ')})`
				);
				for (const rt of options.bench_runtimes) {
					const task = await side_task(side, run_task(rt), ['--save-report']);
					plan.push(
						`[${side.slot}] (cd <side>/src; rm -f benches/js/results/report.${rt}.json; BENCH_PASSES=1 ${task.argv.join(' ')}) per round, ${options.rounds} interleaved rounds, then benches/js/results/report.${rt}.json is kept`
					);
				}
			}
			planned(leg, log, plan);
			return;
		}
		for (const install of SHARED_INSTALLS) {
			if (!(await host.checkout.exists(install))) {
				throw new HarnessError(
					`${checkout}/${install} is absent: install it in the checkout first (\`npm ci\`, \`npm run bench:install\`)`
				);
			}
		}
		const corpora_root = options.corpus.replace(/\/collections\/?$/, '');
		if (corpora_root === options.corpus) {
			throw new HarnessError(
				`the bench leg reads a corpora snapshot's root: ${options.corpus} has no /collections to strip`
			);
		}
		const narrowed = options.bench_env;
		if (Object.keys(narrowed).length > 0) {
			leg.notes.push(
				`NOT A MEASUREMENT: the bench is narrowed by ${Object.entries(narrowed)
					.map(([k, v]) => `${k}=${v}`)
					.join(' ')}`
			);
		}
		const task_names = [
			'bench:harvest:svelte-styles',
			'build:bench',
			...options.bench_runtimes.map(run_task)
		];
		for (const name of task_names) {
			const [base, cand] = [await side_task(sides[0], name), await side_task(sides[1], name)];
			if (base.runner !== cand.runner) {
				leg.notes.push(
					`RUNNERS DIFFER: ${name} runs as \`${base.argv.slice(0, 2).join(' ')}\` on base and \`${cand.argv.slice(0, 2).join(' ')}\` on cand, so the leg's rows also measure the runner's environment`
				);
			}
		}
		for (const side of sides) {
			for (const install of SHARED_INSTALLS) {
				await side.src_handle.symlink(install, `${checkout}/${install}`);
			}
			await side.dir_handle.remove('corpora', { missing_ok: true });
			await side.dir_handle.symlink('corpora', corpora_root);
			const in_tree = { default_target_dir: true };
			await run_side_task(host, side, 'bench:harvest:svelte-styles', [], log, in_tree);
			await run_side_task(host, side, 'build:bench', [], log, in_tree);
		}
		const entries = new Map<string, [Array<number>, Array<number>]>();
		for (const runtime of options.bench_runtimes) {
			for (let round = 0; round < options.rounds; round++) {
				for (const i of round_order(round)) {
					const side = sides[i]!;
					// the extracted tree carries the committed report: a run that writes nothing must
					// find no file, not the committed numbers
					const report_path = `benches/js/results/report.${runtime}.json`;
					await side.src_handle.remove(report_path, { missing_ok: true });
					// a narrowed run skips the canonical report unless told otherwise, and this tree is ours
					await run_side_task(host, side, run_task(runtime), ['--save-report'], log, {
						env: { ...narrowed, BENCH_PASSES: '1' },
						default_target_dir: true
					});
					const text = await side.src_handle.read_text_if_exists(report_path);
					if (text === null) {
						throw new HarnessError(`${run_task(runtime)} wrote no ${side.src}/${report_path}`);
					}
					const report = JSON.parse(text) as { entries: Array<BenchEntry> };
					for (const entry of report.entries) {
						const key = `${runtime} ${entry.group} ${entry.name} p50`;
						const pair = entries.get(key) ?? [[], []];
						pair[i].push(entry.p50_ns);
						entries.set(key, pair);
					}
				}
			}
		}
		const rows = [...entries]
			.map(([key, pair]) => measured_rows(key, 'ns', pair))
			.sort((a, b) => a.key.localeCompare(b.key));
		leg.tables.push({
			id: 'bench',
			title: `Bench report rows, p50, ${options.rounds} interleaved single-pass rounds per runtime`,
			rows,
			display: 'all'
		});
	});
}
