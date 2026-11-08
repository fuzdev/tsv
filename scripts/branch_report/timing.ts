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

import { existsSync } from 'node:fs';
import { copyFile, mkdir, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { dirname, join, relative } from 'node:path';

import { HarnessError, kept_build_env, ROOT } from '../git_snapshot.ts';
import { parse_perf_stat_counter } from '../perf_ab_verdict.ts';
import type { Artifacts } from './legs.ts';
import { run_leg } from './legs.ts';
import type { Leg, Row } from './report.ts';
import { CommandLog, require_tool, run_build, type Side, UnavailableOnRef } from './side.ts';

export const TIMING_LEGS = ['build-wall', 'plain', 'pgo', 'bench', 'small-files'] as const;
export type TimingLeg = (typeof TIMING_LEGS)[number];

export interface TimingOptions {
	legs: ReadonlySet<TimingLeg>;
	/** Even: each side goes first equally often. */
	rounds: number;
	iterations: number;
	corpus: string;
	cache: string;
	dry_run: boolean;
	/** The bench runtimes, for the bench leg. */
	bench_runtimes: ReadonlyArray<'deno' | 'node' | 'bun'>;
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
	binary: string,
	args: ReadonlyArray<string>,
	stat_file: string
): Promise<CliSample> {
	const started = performance.now();
	const output = await new Deno.Command('perf', {
		args: ['stat', '-x,', '-e', 'instructions:u,cycles:u', '-o', stat_file, '--', binary, ...args],
		env: kept_build_env(),
		clearEnv: true,
		stdout: 'null',
		stderr: 'piped'
	}).output();
	const wall_ms = performance.now() - started;
	if (output.code !== 0 && output.code !== 1) {
		throw new HarnessError(
			`perf stat ${binary} exited ${output.code}: ${new TextDecoder().decode(output.stderr).trim()}`
		);
	}
	const text = await readFile(stat_file, 'utf8');
	await rm(stat_file, { force: true });
	return {
		instructions: parse_perf_stat_counter(text, 'instructions'),
		cycles: parse_perf_stat_counter(text, 'cycles'),
		wall_ms
	};
}

/** Interleaved `perf stat` rounds of two CLI binaries over per-language file lists. */
async function cli_pair_rows(
	label: string,
	binaries: readonly [string, string],
	file_lists: ReadonlyArray<{ language: string; paths: ReadonlyArray<string> }>,
	options: TimingOptions,
	log: CommandLog
): Promise<Array<Row>> {
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
				const sample = await perf_stat_cli(
					binaries[i],
					['format', '--check', '--jobs', '1', ...paths],
					join(options.cache, `perf_stat.${Deno.pid}.csv`)
				);
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

/** Group a CLI's discovered files by language for the per-language CLI rows. */
async function language_lists(
	cli: string,
	corpus: string
): Promise<Array<{ language: string; paths: Array<string> }>> {
	const listing = await new Deno.Command(cli, {
		args: ['format', '--list', corpus],
		stdout: 'piped',
		stderr: 'piped'
	}).output();
	if (!listing.success) throw new HarnessError(`tsv format --list exited ${listing.code}`);
	const groups: Record<string, Array<string>> = { typescript: [], svelte: [], css: [] };
	for (const path of new TextDecoder().decode(listing.stdout).split('\n')) {
		if (path === '') continue;
		const lang = path.endsWith('.svelte') ? 'svelte' : path.endsWith('.css') ? 'css' : 'typescript';
		groups[lang]!.push(path);
	}
	return Object.entries(groups).map(([language, paths]) => ({ language, paths }));
}

/** The smallest corpus class: every listed file under `SMALL_FILE_LIMIT` bytes, staged under
 * one root so the plain driver and the CLI both read it (a staged directory, listed itself). */
async function staged_small_corpus(cli: string, corpus: string, cache: string): Promise<string> {
	const dest = join(cache, 'small_corpus');
	const listing = await new Deno.Command(cli, {
		args: ['format', '--list', corpus],
		stdout: 'piped'
	}).output();
	if (!listing.success) throw new HarnessError(`tsv format --list exited ${listing.code}`);
	await rm(dest, { recursive: true, force: true });
	await mkdir(dest, { recursive: true });
	for (const path of new TextDecoder().decode(listing.stdout).split('\n')) {
		if (path === '') continue;
		const info = await stat(path);
		if (info.size >= SMALL_FILE_LIMIT) continue;
		const to = join(dest, relative(corpus, path));
		await mkdir(dirname(to), { recursive: true });
		await copyFile(path, to);
	}
	return dest;
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

async function run_perf_ab(
	sides: readonly [Side, Side],
	corpus: string,
	options: TimingOptions,
	log: CommandLog,
	cache_name: string
): Promise<PerfAbJson> {
	const args = [
		'run',
		'--allow-read',
		'--allow-write',
		'--allow-env',
		'--allow-sys',
		'--allow-run',
		'scripts/perf_ab.ts',
		sides[0].commit,
		sides[1].commit,
		'--rounds',
		String(options.rounds),
		'--iterations',
		String(options.iterations),
		'--corpus',
		corpus,
		'--cache',
		join(options.cache, cache_name),
		'--json'
	];
	log.record(`deno ${args.join(' ')}`);
	const output = await new Deno.Command('deno', {
		args,
		cwd: ROOT,
		stdout: 'piped',
		stderr: 'inherit'
	}).output();
	// perf:ab exits 1 on a regression verdict, which this report does not grade; 2 is a harness error
	if (output.code !== 0 && output.code !== 1)
		throw new HarnessError(`perf:ab exited ${output.code}`);
	return JSON.parse(new TextDecoder().decode(output.stdout)) as PerfAbJson;
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
	sides: readonly [Side, Side],
	options: TimingOptions,
	log: CommandLog
): Promise<Array<Row>> {
	const binaries: Array<string> = [];
	for (const side of sides) {
		await run_build(
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
				env: { CARGO_TARGET_DIR: join(side.dir, 'target_phase') }
			}
		);
		binaries.push(join(side.dir, 'target_phase/release/tsv_debug'));
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
			const output = await new Deno.Command(binaries[i]!, {
				args,
				stdout: 'piped',
				stderr: 'null'
			}).output();
			if (!output.success) throw new HarnessError(`tsv_debug profile exited ${output.code}`);
			const parsed = JSON.parse(new TextDecoder().decode(output.stdout)) as { langs: Agg };
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
	sides: readonly [Side, Side],
	artifacts: readonly [Artifacts, Artifacts] | null,
	options: TimingOptions
): Promise<Leg> {
	return await run_leg(
		'plain interleaved pair (perf:ab, per-phase walls, CLI)',
		async (log, leg) => {
			if (options.dry_run) {
				planned(leg, log, [
					`deno run ... scripts/perf_ab.ts ${sides[0].commit} ${sides[1].commit} --rounds ${options.rounds} --iterations ${options.iterations} --corpus ${options.corpus} --cache <cache>/perf_ab --json`,
					`perf stat -e instructions:u,cycles:u -- <symbolized release tsv> format --check --jobs 1 <per-language files> (${options.rounds} interleaved rounds)`
				]);
				return;
			}
			await require_tool('perf');
			if (artifacts === null)
				throw new HarnessError('the artifact builds failed; no CLI to measure');
			const json = await run_perf_ab(sides, options.corpus, options, log, 'perf_ab');
			leg.tables.push({
				id: 'plain_perf_ab',
				title: `perf:ab rows, ${options.rounds} rounds (the release build, no profile)`,
				rows: perf_ab_rows(json, 'plain'),
				display: 'all'
			});
			const lists = await language_lists(artifacts[0]!.cli, options.corpus);
			leg.tables.push({
				id: 'plain_cli',
				title: 'CLI `format --check --jobs 1`, end to end, plain release',
				rows: await cli_pair_rows(
					'plain cli',
					[artifacts[0]!.cli, artifacts[1]!.cli],
					lists,
					options,
					log
				),
				display: 'all'
			});
		}
	);
}

/** Doc-build / render walls, from a side's `_internal_phase_timing` build of `tsv_debug`. */
export async function leg_phase_walls(
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
			const manifest = await readFile(join(side.src, 'crates/tsv_debug/Cargo.toml'), 'utf8');
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
			rows: await phase_wall_rows(sides, options, log),
			display: 'all'
		});
	});
}

export async function leg_small_files(
	sides: readonly [Side, Side],
	artifacts: readonly [Artifacts, Artifacts] | null,
	options: TimingOptions
): Promise<Leg> {
	return await run_leg(`small files (under ${SMALL_FILE_LIMIT} bytes)`, async (log, leg) => {
		if (options.dry_run) {
			planned(leg, log, [
				`stage every corpus file under ${SMALL_FILE_LIMIT} bytes into <cache>/small_corpus`,
				`deno run ... scripts/perf_ab.ts <base> <cand> --corpus <cache>/small_corpus --rounds ${options.rounds} --json`
			]);
			return;
		}
		if (artifacts === null) throw new HarnessError('the artifact builds failed');
		const staged = await staged_small_corpus(artifacts[0]!.cli, options.corpus, options.cache);
		const json = await run_perf_ab(sides, staged, options, log, 'perf_ab');
		leg.tables.push({
			id: 'small_perf_ab',
			title: `Small-file class, ${options.rounds} rounds (plain release)`,
			rows: perf_ab_rows(json, 'small'),
			display: 'all'
		});
	});
}

export async function leg_build_wall(
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
						`rm -rf target_wall; ${a.argv.join(' ')} (CARGO_TARGET_DIR=target_wall); touch crates/tsv_lang/src/lib.rs; rebuild (each side, ${options.rounds} interleaved rounds)`
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
					const target = join(side.dir, 'target_wall');
					await rm(target, { recursive: true, force: true });
					const env = { CARGO_TARGET_DIR: target };
					const c0 = performance.now();
					await run_build(side, artifact.argv, log, { env });
					clean[i].push(performance.now() - c0);
					const touched = join(side.src, 'crates/tsv_lang/src/lib.rs');
					const now = new Date();
					await Deno.utime(touched, now, now);
					const i0 = performance.now();
					await run_build(side, artifact.argv, log, { env });
					incremental[i].push(performance.now() - i0);
					await rm(target, { recursive: true, force: true });
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

/** The PGO pair: `pgo.ts all` regenerates each side's profile from the same corpus and gates it
 * (non-empty, applied to every crate, byte-identical output), then the CLIs race plain perf. */
export async function leg_pgo(sides: readonly [Side, Side], options: TimingOptions): Promise<Leg> {
	return await run_leg('PGO interleaved pair', async (log, leg) => {
		const corpora_root = options.corpus.replace(/\/collections\/?$/, '');
		const pgo_args = (dir: string): Array<string> => [
			'deno',
			'run',
			'--allow-read',
			'--allow-write',
			'--allow-env',
			'--allow-run',
			'--allow-sys',
			'scripts/pgo/pgo.ts',
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
				...sides.map((side) => `(cd ${side.src}; ${pgo_args(join(side.dir, 'pgo')).join(' ')})`),
				`perf stat -e instructions:u,cycles:u -- <pgo-dir>/target-use/release/tsv format --check --jobs 1 <per-language files> (${options.rounds} interleaved rounds)`,
				'the same over the small-file staged corpus, when small-files is requested'
			]);
			return;
		}
		for (const side of sides) {
			const dir = join(side.dir, 'pgo');
			await run_build(side, pgo_args(dir), log, { default_target_dir: true });
			const stamp = JSON.parse(await readFile(join(dir, 'stamp.json'), 'utf8')) as {
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
		const cli = (side: Side): string => join(side.dir, 'pgo/target-use/release/tsv');
		for (const side of sides)
			if (!existsSync(cli(side))) throw new HarnessError(`no PGO CLI at ${cli(side)}`);
		const lists = await language_lists(cli(sides[0]), options.corpus);
		const binaries: readonly [string, string] = [cli(sides[0]), cli(sides[1])];
		leg.tables.push({
			id: 'pgo_cli',
			title: 'PGO CLI `format --check --jobs 1`, end to end',
			note: 'A profile applies only to its own build world, so the PGO pair is graded on the shipped binary (the CLI), not `tsv_debug`.',
			rows: await cli_pair_rows('pgo cli', binaries, lists, options, log),
			display: 'all'
		});
		if (options.legs.has('small-files')) {
			const staged = await staged_small_corpus(binaries[0], options.corpus, options.cache);
			leg.tables.push({
				id: 'pgo_small',
				title: `PGO CLI, small-file class (under ${SMALL_FILE_LIMIT} bytes)`,
				rows: await cli_pair_rows(
					'pgo small',
					binaries,
					await language_lists(binaries[0], staged),
					options,
					log
				),
				display: 'all'
			});
		}
	});
}

/** The bench's narrowing knobs: a caller's value is passed on, and the report says so. */
const BENCH_NARROWING_ENV = ['BENCH_LIMIT', 'BENCH_FILTER', 'BENCH_DURATION', 'BENCH_WARMUP'];

export function bench_env_from_caller(): Record<string, string> {
	const env: Record<string, string> = {};
	for (const name of BENCH_NARROWING_ENV) {
		const value = Deno.env.get(name);
		if (value !== undefined) env[name] = value;
	}
	return env;
}

interface BenchEntry {
	group: string;
	name: string;
	runtime: string;
	p50_ns: number;
}

/** The bench reports: each side's `build:bench`, then `bench:<runtime>:run` per round (a single
 * pass each, interleaved, so the rounds are the passes). */
export async function leg_bench(
	sides: readonly [Side, Side],
	options: TimingOptions
): Promise<Leg> {
	return await run_leg('bench reports, per runtime', async (log, leg) => {
		const node_modules = join(ROOT, 'benches/js/node_modules');
		if (options.dry_run) {
			planned(leg, log, [
				`symlink ${node_modules} into each side's benches/js/node_modules (the shared install; a missing install fails this leg)`,
				`symlink <side>/corpora to the corpus root, which every bench path reads as ../corpora`,
				`(cd <side>/src; deno task bench:harvest:svelte-styles && deno task build:bench) for each side`,
				...options.bench_runtimes.map(
					(rt) =>
						`(cd <side>/src; rm -f benches/js/results/report.${rt}.json; BENCH_PASSES=1 deno task bench:${rt}:run --save-report) per round, ${options.rounds} interleaved rounds, then benches/js/results/report.${rt}.json is kept`
				)
			]);
			return;
		}
		if (!existsSync(node_modules))
			throw new HarnessError(
				`${node_modules} is absent: run \`deno task bench:install\` in the checkout first`
			);
		const corpora_root = options.corpus.replace(/\/collections\/?$/, '');
		if (corpora_root === options.corpus) {
			throw new HarnessError(
				`the bench leg reads a corpora snapshot's root: ${options.corpus} has no /collections to strip`
			);
		}
		const narrowed = bench_env_from_caller();
		if (Object.keys(narrowed).length > 0) {
			leg.notes.push(
				`NOT A MEASUREMENT: the bench is narrowed by ${Object.entries(narrowed)
					.map(([k, v]) => `${k}=${v}`)
					.join(' ')}`
			);
		}
		for (const side of sides) {
			const link = join(side.src, 'benches/js/node_modules');
			if (!existsSync(link)) await Deno.symlink(node_modules, link);
			const corpora_link = join(side.dir, 'corpora');
			await rm(corpora_link, { force: true });
			await Deno.symlink(corpora_root, corpora_link);
			await run_build(side, ['deno', 'task', 'bench:harvest:svelte-styles'], log, {
				default_target_dir: true
			});
			await run_build(side, ['deno', 'task', 'build:bench'], log, { default_target_dir: true });
		}
		const entries = new Map<string, [Array<number>, Array<number>]>();
		for (const runtime of options.bench_runtimes) {
			for (let round = 0; round < options.rounds; round++) {
				for (const i of round_order(round)) {
					const side = sides[i]!;
					// the extracted tree carries the committed report: a run that writes nothing must
					// find no file, not the committed numbers
					const report_path = join(side.src, `benches/js/results/report.${runtime}.json`);
					await rm(report_path, { force: true });
					// a narrowed run skips the canonical report unless told otherwise, and this tree is ours
					await run_build(side, ['deno', 'task', `bench:${runtime}:run`, '--save-report'], log, {
						env: { ...narrowed, BENCH_PASSES: '1' },
						default_target_dir: true
					});
					if (!existsSync(report_path)) {
						throw new HarnessError(`bench:${runtime}:run wrote no ${report_path}`);
					}
					const report = JSON.parse(await readFile(report_path, 'utf8')) as {
						entries: Array<BenchEntry>;
					};
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
