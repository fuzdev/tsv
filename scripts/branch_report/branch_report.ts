/**
 * `deno task branch:report <base-ref> <cand-ref>` — one document comparing two commits on every
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
 * Not in `deno task check`: it builds two full release trees.
 *
 * @module
 */

import { existsSync } from 'node:fs';
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';

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
import { HarnessError, ROOT, run } from './deno_host.ts';
import { corpus_identity, evict_sides, prepare_side, type Side } from './side.ts';
import {
	leg_bench,
	leg_build_wall,
	leg_plain,
	leg_pgo,
	leg_phase_walls,
	leg_small_files,
	bench_env_from_caller,
	TIMING_LEGS,
	type TimingLeg,
	type TimingOptions
} from './timing.ts';

const USAGE = `usage: deno task branch:report <base-ref> <cand-ref> [options]
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

function parse_options(args: ReadonlyArray<string>): Options {
	const positional: Array<string> = [];
	const home = Deno.env.get('HOME');
	if (!home) throw new HarnessError('HOME is not set');
	const options: Omit<Options, 'base_ref' | 'cand_ref'> = {
		out: null,
		cache: resolve(home, '.cache/tsv/branch_report'),
		corpus: resolve(ROOT, CORPORA_COLLECTIONS),
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
		if (arg === '--out') options.out = resolve(ROOT, take(arg, args[++i]));
		else if (arg === '--cache') options.cache = resolve(ROOT, take(arg, args[++i]));
		else if (arg === '--corpus') options.corpus = resolve(ROOT, take(arg, args[++i]));
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
			options.bench_runtimes = take(arg, args[++i]).split(',') as Array<'deno' | 'node' | 'bun'>;
		} else if (arg === '--aa') options.aa_paths.push(resolve(ROOT, take(arg, args[++i])));
		else if (arg === '--no-depth-margin') options.depth_margin = false;
		else if (arg === '--dry-run') options.dry_run = true;
		else if (arg === '--accept-gaps') options.accept_gaps = true;
		else if (arg === '--help' || arg === '-h') {
			console.log(USAGE);
			Deno.exit(0);
		} else if (arg.startsWith('-')) throw new HarnessError(`unknown flag ${arg}`);
		else positional.push(arg);
	}
	if (positional.length !== 2) throw new HarnessError('expected <base-ref> and <cand-ref>');
	if (options.rounds % 2 !== 0) throw new HarnessError('--rounds takes an even count');
	for (const s of options.sections) {
		if (!['a', 'b', 'c'].includes(s)) throw new HarnessError(`unknown section ${s}`);
	}
	return { base_ref: positional[0]!, cand_ref: positional[1]!, ...options };
}

async function command_version(cmd: string, args: ReadonlyArray<string>): Promise<string> {
	try {
		const result = await run(cmd, args);
		const text = (result.stdout || result.stderr).trim();
		return result.success ? text.split('\n')[0]! : `absent (exit ${result.code})`;
	} catch (err) {
		if (err instanceof Deno.errors.NotFound) return 'absent';
		throw err;
	}
}

async function gather_provenance(
	options: Options,
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
	const corpus = await corpus_identity(options.corpus);
	const cpu =
		/model name\s*:\s*(.+)/.exec(
			await command_version('grep', ['-m1', 'model name', '/proc/cpuinfo'])
		)?.[1] ?? 'unknown';
	const [hostname, uname, nproc] = await Promise.all([
		command_version('hostname', []),
		command_version('uname', ['-r']),
		command_version('nproc', [])
	]);
	const [wasm_pack, deno_v, node, bun, heaptrack, perf] = await Promise.all([
		command_version('wasm-pack', ['--version']),
		command_version('deno', ['--version']),
		command_version('node', ['--version']),
		command_version('bun', ['--version']),
		command_version('heaptrack', ['--version']),
		command_version('perf', ['--version'])
	]);
	return {
		command_line: `deno task branch:report ${Deno.args.join(' ')}`,
		base: info(sides[0]),
		cand: info(sides[1]),
		machine: {
			hostname,
			cpu,
			kernel: uname,
			threads: nproc,
			'1-minute load': Deno.loadavg()[0]!.toFixed(2)
		},
		tools: { 'wasm-pack': wasm_pack, deno: deno_v, node, bun, heaptrack, perf },
		corpus: { path: options.corpus, ...corpus },
		cargo_build_jobs: Deno.env.get('CARGO_BUILD_JOBS') ?? null,
		flags: Deno.args.filter((a) => a.startsWith('-'))
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

async function main(): Promise<number> {
	const options = parse_options(Deno.args);
	if (!existsSync(options.corpus)) {
		throw new HarnessError(
			`corpus not found: ${options.corpus} — check out fuzdev/corpora beside this repo, or pass --corpus`
		);
	}
	await mkdir(options.cache, { recursive: true });
	const sides = [
		await prepare_side(options.base_ref, 'base', options.cache),
		await prepare_side(options.cand_ref, 'cand', options.cache)
	] as const;
	await evict_sides(options.cache, sides);
	const provenance = await gather_provenance(options, sides);
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
		bench_env: bench_env_from_caller(),
		cargo_build_jobs: provenance.cargo_build_jobs
	};
	const draws: Array<{ source: FloorSource; report: BranchReport }> = [];
	for (const path of options.aa_paths) {
		const report = JSON.parse(await readFile(path, 'utf8')) as BranchReport;
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

	const scratch = join(options.cache, `scratch.${Deno.pid}`);
	await mkdir(scratch, { recursive: true });

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
				artifact = await build_artifacts(side, log);
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
				? await leg_wasm_sizes('WASM bundle bytes', sides, artifacts)
				: missing_artifacts('WASM bundle bytes'),
			artifacts
				? await leg_native_sizes('native sizes, .text, bounds sites', sides, artifacts)
				: missing_artifacts('native sizes, .text, bounds sites')
		);
	}
	const sizes = options.sections.has('a')
		? finish_section('sizes', sizes_legs, 'not_requested')
		: not_requested_section('sizes', '--sections does not include a');

	const types_legs: Array<Leg> = [];
	if (options.sections.has('b'))
		types_legs.push(await leg_type_sizes_host(sides), await leg_type_sizes_wasm(sides));
	const types = options.sections.has('b')
		? finish_section('types', types_legs, 'not_requested')
		: not_requested_section('types', '--sections does not include b');

	const work_legs: Array<Leg> = [];
	if (options.sections.has('c')) {
		work_legs.push(
			artifacts
				? await leg_allocations(sides, artifacts, options.corpus, scratch)
				: missing_artifacts('native allocation counts (heaptrack)'),
			artifacts
				? await leg_rss(sides, artifacts, options.corpus)
				: missing_artifacts('native peak RSS (/usr/bin/time)'),
			artifacts
				? await leg_wasm_memory(sides, artifacts, options.corpus, scratch)
				: missing_artifacts('WASM linear-memory high-water'),
			await leg_depth(sides, { margin: options.depth_margin })
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
		cache: options.cache,
		dry_run: options.dry_run,
		bench_runtimes: options.bench_runtimes
	};
	const timing_off = 'not requested: pass --timing (timing needs a quiet machine)';
	const build_wall_legs: Array<Leg> = [];
	if (options.timing.has('build-wall'))
		build_wall_legs.push(await leg_build_wall(sides, timing_options));
	const build_wall =
		build_wall_legs.length > 0
			? finish_section('build_wall', build_wall_legs, 'ok')
			: not_requested_section('build_wall', timing_off);

	const timing_legs: Array<Leg> = [];
	if (options.timing.has('plain')) {
		timing_legs.push(
			await leg_plain(sides, artifacts, timing_options),
			await leg_phase_walls(sides, timing_options)
		);
	}
	if (options.timing.has('small-files'))
		timing_legs.push(await leg_small_files(sides, artifacts, timing_options));
	if (options.timing.has('pgo')) timing_legs.push(await leg_pgo(sides, timing_options));
	if (options.timing.has('bench')) timing_legs.push(await leg_bench(sides, timing_options));
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
	const out_dir = options.out ?? join(options.cache, 'reports', stem);
	await mkdir(out_dir, { recursive: true });
	await writeFile(join(out_dir, 'report.md'), render_markdown(report, floors));
	await writeFile(join(out_dir, 'report.json'), `${JSON.stringify(report, null, '\t')}\n`);
	await rm(scratch, { recursive: true, force: true });

	const problems = report_problems(report);
	for (const problem of problems)
		console.error(`branch:report: ${problem.kind}: ${problem.message}`);
	console.log(`branch:report: report written to ${out_dir}`);
	return exit_code(report, options.accept_gaps);
}

if (import.meta.main) {
	try {
		Deno.exit(await main());
	} catch (err) {
		if (err instanceof HarnessError) {
			console.error(`branch:report: error: ${err.message}`);
			if (err.message.startsWith('expected') || err.message.startsWith('unknown'))
				console.error(USAGE);
			Deno.exit(2);
		}
		console.error(err);
		Deno.exit(2);
	}
}
