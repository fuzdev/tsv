/**
 * `npm run branch:report -- <base-ref> <cand-ref>` — one document comparing two commits on every
 * axis a merge read needs: artifact sizes, type widths and work counters deterministically, with
 * build wall and timing behind flags, and the store-rows section left open (it exists once the
 * indexed-store port does). Each ref is built from git in its own tree and target directory
 * (`side.ts`); the same ref on both sides is the A/A null, built twice, and its report is the
 * floor a later run's timing rows take (`--aa`).
 *
 * A missing tool, a skipped leg or a failed build is a row in the document and a non-zero exit;
 * only `--accept-gaps` accepts them, and it also accepts a floor mismatch and a confounded
 * toolchain; each stays listed under Problems. Exit codes: 0 clean, 1 a same-ref difference on a
 * deterministic row, 2 a harness error, or a gap, floor mismatch or confounded toolchain unaccepted.
 *
 * Not in `check`: it builds two full release trees.
 *
 * `run_branch_report_command` is the tool; its root, `../roots/branch_report.ts`, builds what it
 * reaches (`BranchReportDeps`).
 *
 * @module
 */

import { resolve } from 'node:path';

import type { FsDir, FsReadDir } from '@fuzdev/gro/fs.ts';
import type { FsPaths } from '@fuzdev/gro/fs_paths.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';

import { CORPORA_COLLECTIONS } from '../../benches/js/lib/corpora.ts';
import {
	type Artifacts,
	build_artifacts,
	leg_allocations,
	leg_depth,
	leg_native_sizes,
	leg_rss,
	leg_type_sizes_host,
	leg_type_sizes_wasm,
	leg_wasm_memory,
	leg_wasm_sizes,
	run_leg
} from './legs.ts';
import {
	type BranchReport,
	exit_code,
	combine_floors,
	type FloorContext,
	type FloorSource,
	type Leg,
	type Provenance,
	render_markdown,
	REPORT_VERSION,
	report_problems,
	type Section,
	type SectionId,
	section_status,
	type SideInfo
} from './report.ts';
import { exit_code_of, format_ending, HarnessError } from '../git_snapshot.ts';
import {
	corpus_identity,
	evict_sides,
	path_exists,
	prepare_side,
	type ReportCache,
	type ReportHost,
	type Side
} from './side.ts';
import {
	leg_bench,
	leg_build_wall,
	leg_plain,
	leg_pgo,
	leg_phase_walls,
	leg_small_files,
	TIMING_LEGS,
	type TimingLeg,
	type TimingOptions
} from './timing.ts';

const USAGE = `usage: npm run branch:report -- <base-ref> <cand-ref> [options]
  --out DIR         where report.md and report.json go (default <cache>/reports/<base>-<cand>-<time>/)
  --cache DIR       the build cache (default ~/.cache/tsv/branch_report)
  --corpus DIR      corpus (default ${CORPORA_COLLECTIONS})
  --sections LIST   comma list of a,b,c (default a,b,c); d and e need --timing
  --timing LIST     comma list of ${TIMING_LEGS.join(',')} (off by default; timing runs need a quiet machine)
  --rounds N        interleaved rounds for timing legs, even (default 2)
  --iterations N    iterations per file for the plain perf:ab pair (default 10)
  --bench-runtimes  comma list of deno,node,bun for the bench leg (default deno,node,bun)
  --aa FILE         an earlier same-ref report.json, repeatable: each row's floor is its largest
                    delta across the draws; a draw from another corpus, machine or setup is void
  --no-depth-margin skip the stack-margin bisect in the depth table
  --dry-run         for timing legs, print the plan and run nothing
  --accept-gaps     exit 0 despite missing tools, skips, failed legs, a floor mismatch or a
                    confounded toolchain (all stay listed under Problems)`;

interface Options {
	base_ref: string;
	cand_ref: string;
	out: string | null;
	cache: string;
	corpus: string;
	sections: ReadonlySet<string>;
	timing: ReadonlySet<TimingLeg>;
	rounds: number;
	iterations: number;
	bench_runtimes: ReadonlyArray<'deno' | 'node' | 'bun'>;
	aa_paths: Array<string>;
	depth_margin: boolean;
	dry_run: boolean;
	accept_gaps: boolean;
}

/** The options `args` give, `'help'` for `--help`; relative paths resolve against `checkout`. */
function parse_options(
	args: ReadonlyArray<string>,
	checkout: string,
	home: string | undefined
): Options | 'help' {
	const positional: Array<string> = [];
	if (!home) throw new HarnessError('HOME is not set');
	const options: Omit<Options, 'base_ref' | 'cand_ref'> = {
		out: null,
		cache: resolve(home, '.cache/tsv/branch_report'),
		corpus: resolve(checkout, CORPORA_COLLECTIONS),
		sections: new Set(['a', 'b', 'c']),
		timing: new Set(),
		rounds: 2,
		iterations: 10,
		bench_runtimes: ['deno', 'node', 'bun'],
		aa_paths: [],
		depth_margin: true,
		dry_run: false,
		accept_gaps: false
	};
	const take = (flag: string, value: string | undefined): string => {
		if (!value) throw new HarnessError(`${flag} takes a value`);
		return value;
	};
	const count = (flag: string, value: string | undefined): number => {
		const n = Number(take(flag, value));
		if (!Number.isInteger(n) || n < 1) throw new HarnessError(`${flag} takes a positive integer`);
		return n;
	};
	for (let i = 0; i < args.length; i++) {
		const arg = args[i]!;
		if (arg === '--out') options.out = resolve(checkout, take(arg, args[++i]));
		else if (arg === '--cache') options.cache = resolve(checkout, take(arg, args[++i]));
		else if (arg === '--corpus') options.corpus = resolve(checkout, take(arg, args[++i]));
		else if (arg === '--sections') options.sections = new Set(take(arg, args[++i]).split(','));
		else if (arg === '--timing') {
			const legs = take(arg, args[++i]).split(',');
			for (const leg of legs) {
				if (!(TIMING_LEGS as ReadonlyArray<string>).includes(leg))
					throw new HarnessError(`unknown timing leg ${leg}`);
			}
			options.timing = new Set(legs as Array<TimingLeg>);
		} else if (arg === '--rounds') options.rounds = count(arg, args[++i]);
		else if (arg === '--iterations') options.iterations = count(arg, args[++i]);
		else if (arg === '--bench-runtimes') {
			const runtimes = take(arg, args[++i]).split(',');
			for (const runtime of runtimes) {
				if (!['deno', 'node', 'bun'].includes(runtime))
					throw new HarnessError(`unknown bench runtime '${runtime}'`);
			}
			options.bench_runtimes = runtimes as Array<'deno' | 'node' | 'bun'>;
		} else if (arg === '--aa') options.aa_paths.push(resolve(checkout, take(arg, args[++i])));
		else if (arg === '--no-depth-margin') options.depth_margin = false;
		else if (arg === '--dry-run') options.dry_run = true;
		else if (arg === '--accept-gaps') options.accept_gaps = true;
		else if (arg === '--help' || arg === '-h') return 'help';
		else if (arg.startsWith('-')) throw new HarnessError(`unknown flag ${arg}`);
		else positional.push(arg);
	}
	if (positional.length !== 2) throw new HarnessError('expected <base-ref> and <cand-ref>');
	if (options.rounds % 2 !== 0) throw new HarnessError('--rounds takes an even count');
	for (const s of options.sections) {
		if (!['a', 'b', 'c'].includes(s)) throw new HarnessError(`unknown section ${s}`);
	}
	return { base_ref: positional[0]!, cand_ref: positional[1]!, ...options };
}

/** What `run_branch_report_command` reaches: the checkout, the paths it is given, the cache, the
 * variables it reads, and each program it runs, every one built by the root. */
export interface BranchReportDeps extends Omit<ReportHost, 'cache'> {
	/** `HOME`, beneath which the default cache sits. */
	home: string | undefined;
	/** The caller's `CARGO_BUILD_JOBS`, for the provenance table. */
	cargo_build_jobs: string | undefined;
	/** The caller's bench narrowing variables (`BENCH_NARROWING_ENV`). */
	bench_env: Readonly<Record<string, string>>;
	/** Opens the cache at `path`, absolute, made when missing. */
	open_cache: (path: string) => Promise<ReportCache>;
	/** Opens the directory at `path`, absolute, made when missing: `--out`. */
	open_made_dir: (path: string) => Promise<FsDir>;
	/** The 1-minute load average. */
	load_1m: () => number;
}

/** The first line `program args` prints, `absent` when it is not installed. */
async function command_version(
	program: SpawnProgram | null,
	args: ReadonlyArray<string>,
	cwd: FsReadDir
): Promise<string> {
	if (program === null) return 'absent';
	const result = await program.out(args, { cwd });
	const text = (result.stdout || result.stderr).trim();
	if (exit_code_of(result) === 0) return text.split('\n')[0]!;
	return result.kind === 'exited'
		? `absent (exit ${result.code})`
		: `absent (${format_ending(result)})`;
}

async function gather_provenance(
	deps: BranchReportDeps,
	options: Options,
	args: ReadonlyArray<string>,
	sides: readonly [Side, Side]
): Promise<Provenance> {
	const info = (side: Side): SideInfo => ({
		slot: side.slot,
		ref: side.ref,
		commit: side.commit,
		tree: side.tree,
		build_dir: side.dir,
		rustc: side.rustc,
		cargo: side.cargo
	});
	const { tools, checkout } = deps;
	const version = (name: keyof typeof tools, version_args: ReadonlyArray<string>) =>
		command_version(tools[name], version_args, checkout);
	const corpus = await corpus_identity(deps.git, checkout, options.corpus);
	const cpu =
		/model name\s*:\s*(.+)/.exec(
			await version('grep', ['-m1', 'model name', '/proc/cpuinfo'])
		)?.[1] ?? 'unknown';
	const [hostname, uname, nproc] = await Promise.all([
		version('hostname', []),
		version('uname', ['-r']),
		version('nproc', [])
	]);
	const [wasm_pack, deno_v, node, bun, heaptrack, perf] = await Promise.all([
		version('wasm-pack', ['--version']),
		version('deno', ['--version']),
		version('node', ['--version']),
		version('bun', ['--version']),
		version('heaptrack', ['--version']),
		version('perf', ['--version'])
	]);
	return {
		command_line: `npm run branch:report -- ${args.join(' ')}`,
		base: info(sides[0]),
		cand: info(sides[1]),
		machine: {
			hostname,
			cpu,
			kernel: uname,
			threads: nproc,
			'1-minute load': deps.load_1m().toFixed(2)
		},
		tools: { 'wasm-pack': wasm_pack, deno: deno_v, node, bun, heaptrack, perf },
		corpus: { path: options.corpus, ...corpus },
		cargo_build_jobs: deps.cargo_build_jobs ?? null,
		flags: args.filter((a) => a.startsWith('-'))
	};
}

function not_requested_section(id: SectionId, reason: string): Section {
	return { id, status: 'not_requested', reason, legs: [] };
}

/** The store-rows section: with an indexed AST, rows minted, live and orphaned, capacity and
 * growth copies per store, read from the store accessor's own counters. */
function store_rows_section(): Section {
	return {
		id: 'store_rows',
		status: 'not_applicable',
		reason:
			'the AST is arena-allocated, so there are no stores, rows or capacities to count. ' +
			'Extension point: `store_rows_section` in scripts/branch_report/branch_report.ts',
		legs: []
	};
}

function finish_section(
	id: SectionId,
	legs: Array<Leg>,
	status_when_empty: Section['status'],
	reason?: string
): Section {
	const section: Section = { id, status: status_when_empty, legs };
	if (legs.length > 0) section.status = section_status(legs);
	if (reason) section.reason = reason;
	return section;
}

/** The file at `path`, absolute, its symlinks followed, as text. */
async function read_path(fs_paths: FsPaths, path: string): Promise<string> {
	const opened = await fs_paths.open_path(path);
	return await opened.dir.read_text(opened.path);
}

/**
 * Runs `branch:report` with `args`, the arguments after the script: the document goes to
 * `report.md` and `report.json`, the progress to stderr.
 * @returns the exit code: 0 clean, 1 a same-ref difference on a deterministic row, 2 a harness
 * error, or a gap, floor mismatch or confounded toolchain unaccepted
 */
export async function run_branch_report_command(
	deps: BranchReportDeps,
	args: ReadonlyArray<string>
): Promise<number> {
	try {
		return await branch_report(deps, args);
	} catch (err) {
		if (err instanceof HarnessError) {
			console.error(`branch:report: error: ${err.message}`);
			if (err.message.startsWith('expected') || err.message.startsWith('unknown'))
				console.error(USAGE);
			return 2;
		}
		console.error(err);
		return 2;
	}
}

async function branch_report(deps: BranchReportDeps, args: ReadonlyArray<string>): Promise<number> {
	const options = parse_options(args, deps.checkout.path, deps.home);
	if (options === 'help') {
		console.log(USAGE);
		return 0;
	}
	if (!(await path_exists(deps.fs_paths, options.corpus))) {
		throw new HarnessError(
			`corpus not found: ${options.corpus} — check out fuzdev/corpora beside this repo, or pass --corpus`
		);
	}
	const host: ReportHost = { ...deps, cache: await deps.open_cache(options.cache) };
	const cache = host.cache.dir;
	const sides = [
		await prepare_side(host, options.base_ref, 'base'),
		await prepare_side(host, options.cand_ref, 'cand')
	] as const;
	await evict_sides(cache, sides);
	const provenance = await gather_provenance(deps, options, args, sides);
	const same_ref = sides[0].commit === sides[1].commit;

	const floor_context: FloorContext = {
		corpus_tree: provenance.corpus.tree,
		corpus_relative_path: provenance.corpus.relative_path,
		corpus_dirty: provenance.corpus.dirty,
		cpu: provenance.machine.cpu!,
		hostname: provenance.machine.hostname!,
		rounds: options.rounds,
		iterations: options.iterations,
		timing_legs: [...options.timing].sort(),
		bench_env: { ...deps.bench_env },
		cargo_build_jobs: provenance.cargo_build_jobs
	};
	const draws: Array<{ source: FloorSource; report: BranchReport }> = [];
	for (const path of options.aa_paths) {
		const report = JSON.parse(await read_path(deps.fs_paths, path)) as BranchReport;
		draws.push({
			report,
			source: {
				path,
				generated: report.generated,
				commit: report.provenance.base.commit,
				context: report.floor_context ?? null
			}
		});
	}
	const { floors, mismatches: floor_mismatches } = combine_floors(draws, floor_context);

	const scratch = `scratch.${deps.pid}`;
	await cache.mkdir(scratch, { recursive: true });

	const needs_artifacts =
		options.sections.has('a') ||
		options.sections.has('c') ||
		options.timing.has('plain') ||
		options.timing.has('small-files');
	const sizes_legs: Array<Leg> = [];
	const build_failures: Array<string> = [];
	let artifacts: readonly [Artifacts, Artifacts] | null = null;
	if (needs_artifacts) {
		const built: Array<Artifacts | null> = [];
		for (const side of sides) {
			let artifact: Artifacts | null = null;
			const leg = await run_leg(`build artifacts (${side.slot})`, async (log) => {
				artifact = await build_artifacts(host, side, log);
			});
			sizes_legs.push(leg);
			if (artifact === null) {
				build_failures.push(`${side.slot} ${side.ref}: ${leg.reason ?? leg.status}`);
			}
			built.push(artifact);
		}
		if (built[0] !== null && built[1] !== null) artifacts = [built[0]!, built[1]!];
	}

	const missing_artifacts = (name: string): Leg => ({
		name,
		status: 'failed',
		reason: `the artifact builds failed (${build_failures.join('; ')}), so this leg has nothing to read`,
		wall_ms: null,
		commands: [],
		tables: [],
		notes: []
	});

	if (options.sections.has('a')) {
		sizes_legs.push(
			artifacts
				? await leg_wasm_sizes(host, 'WASM bundle bytes', sides, artifacts)
				: missing_artifacts('WASM bundle bytes'),
			artifacts
				? await leg_native_sizes(host, 'native sizes, .text, bounds sites', sides, artifacts)
				: missing_artifacts('native sizes, .text, bounds sites')
		);
	}
	const sizes = options.sections.has('a')
		? finish_section('sizes', sizes_legs, 'not_requested')
		: not_requested_section('sizes', '--sections does not include a');

	const types_legs: Array<Leg> = [];
	if (options.sections.has('b'))
		types_legs.push(await leg_type_sizes_host(host, sides), await leg_type_sizes_wasm(host, sides));
	const types = options.sections.has('b')
		? finish_section('types', types_legs, 'not_requested')
		: not_requested_section('types', '--sections does not include b');

	const work_legs: Array<Leg> = [];
	if (options.sections.has('c')) {
		work_legs.push(
			artifacts
				? await leg_allocations(host, sides, artifacts, options.corpus, scratch)
				: missing_artifacts('native allocation counts (heaptrack)'),
			artifacts
				? await leg_rss(host, sides, artifacts, options.corpus)
				: missing_artifacts('native peak RSS (/usr/bin/time)'),
			artifacts
				? await leg_wasm_memory(host, sides, artifacts, options.corpus, scratch)
				: missing_artifacts('WASM linear-memory high-water'),
			await leg_depth(host, sides, { margin: options.depth_margin })
		);
	}
	const work = options.sections.has('c')
		? finish_section('work', work_legs, 'not_requested')
		: not_requested_section('work', '--sections does not include c');

	const timing_options: TimingOptions = {
		legs: options.timing,
		rounds: options.rounds,
		iterations: options.iterations,
		corpus: options.corpus,
		dry_run: options.dry_run,
		bench_runtimes: options.bench_runtimes,
		bench_env: deps.bench_env
	};
	const timing_off = 'not requested: pass --timing (timing needs a quiet machine)';
	const build_wall_legs: Array<Leg> = [];
	if (options.timing.has('build-wall'))
		build_wall_legs.push(await leg_build_wall(host, sides, timing_options));
	const build_wall =
		build_wall_legs.length > 0
			? finish_section('build_wall', build_wall_legs, 'ok')
			: not_requested_section('build_wall', timing_off);

	const timing_legs: Array<Leg> = [];
	if (options.timing.has('plain')) {
		timing_legs.push(
			await leg_plain(host, sides, artifacts, timing_options),
			await leg_phase_walls(host, sides, timing_options)
		);
	}
	if (options.timing.has('small-files'))
		timing_legs.push(await leg_small_files(host, sides, artifacts, timing_options));
	if (options.timing.has('pgo')) timing_legs.push(await leg_pgo(host, sides, timing_options));
	if (options.timing.has('bench')) timing_legs.push(await leg_bench(host, sides, timing_options));
	const timing =
		timing_legs.length > 0
			? finish_section('timing', timing_legs, 'ok')
			: not_requested_section('timing', timing_off);

	const report: BranchReport = {
		version: REPORT_VERSION,
		generated: new Date().toISOString(),
		same_ref,
		provenance,
		floor_context,
		aa_sources: draws.map((d) => d.source),
		floor_mismatches,
		sections: [sizes, types, work, build_wall, timing, store_rows_section()]
	};

	const stem = `${sides[0].commit.slice(0, 8)}-${sides[1].commit.slice(0, 8)}-${report.generated.replaceAll(/[:.]/g, '-')}`;
	let out_dir: FsDir;
	if (options.out === null) {
		await cache.mkdir(`reports/${stem}`, { recursive: true });
		out_dir = cache.dir(`reports/${stem}`);
	} else {
		out_dir = await deps.open_made_dir(options.out);
	}
	await out_dir.write('report.md', render_markdown(report, floors));
	await out_dir.write('report.json', `${JSON.stringify(report, null, '\t')}\n`);
	await cache.remove(scratch, { recursive: true, missing_ok: true });

	const problems = report_problems(report);
	for (const problem of problems)
		console.error(`branch:report: ${problem.kind}: ${problem.message}`);
	console.log(`branch:report: report written to ${out_dir.path}`);
	return exit_code(report, options.accept_gaps);
}
