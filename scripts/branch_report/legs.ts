/**
 * The deterministic legs: sizes (section A), type sizes (B), work counters (C). Each leg runs a
 * side's builds through `side.ts`, reads their output with `parsers.ts`, and returns rows; a
 * missing tool, a failed build or a file a ref lacks becomes that leg's status, never an omitted
 * row, so the document always says what it did not measure.
 *
 * @module
 */

import type { SpawnOutput, SpawnProgram } from '@fuzdev/gro/spawn.ts';

import { exit_code_of, format_ending, HarnessError, sha256_hex } from '../git_snapshot.ts';
import { gzip_size } from '../gzip_size.ts';
import {
	assert_bounds_sites,
	assert_format_check_finished,
	assert_parsed_rows,
	assert_symbols,
	assert_wasm_sweep,
	BOUNDS_CALLEES,
	count_bounds_sites,
	diff_symbols,
	parse_board_text,
	parse_heaptrack_summary,
	parse_nm,
	parse_rss_kib,
	parse_shape_limits,
	parse_size_sections,
	parse_type_sizes_json,
	type SymbolSizes,
	test_run_matched,
	type TypeRowSize
} from './parsers.ts';
import type { Leg, LegStatus, Row, Table } from './report.ts';
import {
	cache_path,
	CommandLog,
	MissingTool,
	NICE_ARGS,
	type ReportHost,
	require_tool,
	run_build,
	type Side,
	UnavailableOnRef
} from './side.ts';
import {
	BOARD_SOURCE,
	board_text_from_exports,
	wasm_board_lib,
	wasm_board_manifest
} from './wasm_board.ts';
import type { WasmMemoryResult } from './wasm_memory.ts';

export const WASM_VARIANTS = ['format', 'parse', 'all'] as const;
export type WasmVariant = (typeof WASM_VARIANTS)[number];

/** How many top per-symbol movers a report table carries. */
export const TOP_MOVERS = 40;

/** A side's built artifacts, each a path beneath the cache. */
export interface Artifacts {
	/** The plain `release` CLI and `napi` addon, stripped by their profile. */
	cli: string;
	addon: string;
	/** Their symbolized builds (`strip = false`), a separate target dir: `nm` and `objdump` read
	 * these. Link layout differs a little from the plain binary's, so `.text` per symbol is a
	 * comparison between two symbolized builds, never the plain file's own bytes. */
	cli_symbols: string;
	addon_symbols: string;
	/** Each variant's `tsv_wasm_bg.wasm`, its glue `tsv_wasm.js` beside it. */
	wasm: Record<WasmVariant, string>;
}

/** A leg in progress: its name, command log and what it returns. Throwing a `HarnessError`
 * becomes the leg's status (`MissingTool` → `missing_tool`, `UnavailableOnRef` →
 * `unavailable_on_ref`, anything else → `failed`). */
export async function run_leg(
	name: string,
	body: (log: CommandLog, leg: Leg) => Promise<void>
): Promise<Leg> {
	const log = new CommandLog();
	const leg: Leg = {
		name,
		status: 'ok',
		wall_ms: null,
		commands: log.lines,
		tables: [],
		notes: []
	};
	const started = performance.now();
	console.error(`branch:report: leg ${name}`);
	try {
		await body(log, leg);
	} catch (err) {
		leg.status = status_of(err);
		leg.reason = err instanceof Error ? err.message : String(err);
		if (!(err instanceof HarnessError)) console.error(err);
		console.error(`branch:report: leg ${name}: ${leg.status}: ${leg.reason}`);
	}
	leg.wall_ms = performance.now() - started;
	return leg;
}

function status_of(err: unknown): LegStatus {
	if (err instanceof MissingTool) return 'missing_tool';
	if (err instanceof UnavailableOnRef) return 'unavailable_on_ref';
	return 'failed';
}

/** A two-sided row for each key the two maps name, keys in sorted order. */
function exact_rows(
	base: ReadonlyMap<string, number>,
	cand: ReadonlyMap<string, number>,
	unit: string,
	label: (key: string) => string = (key) => key
): Array<Row> {
	const keys = [...new Set([...base.keys(), ...cand.keys()])].sort();
	return keys.map((key) => ({
		key: label(key),
		unit,
		base: base.get(key) ?? null,
		cand: cand.get(key) ?? null,
		exact: true
	}));
}

/** `result`'s output, refusing one the capture cap cut short. */
export function whole_output<T extends { truncated: boolean }>(result: T, what: string): T {
	if (result.truncated) throw new HarnessError(`${what}: its output was cut short (truncated)`);
	return result;
}

/** A run's exit code, refusing one that ended any other way (a signal, a stop). */
function exit_code_or_throw(result: SpawnOutput, what: string): number {
	const code = exit_code_of(result);
	if (code === null)
		throw new HarnessError(`${what} ${format_ending(result)}: ${result.stderr.trim()}`);
	return code;
}

// -- artifact builds ----------------------------------------------------------------------------

/**
 * Build a side's plain artifacts: the `release` CLI and the `napi` addon as their profiles
 * build them, again with the symbol table kept (the per-symbol and bounds-site rows read that
 * pair, in its own target directory), and the three wasm packages' bundles. Each is named by
 * its path beneath the cache.
 */
export async function build_artifacts(
	host: ReportHost,
	side: Side,
	log: CommandLog
): Promise<Artifacts> {
	const wasm_pack = require_tool(host, 'wasm-pack');
	const cli_build = ['cargo', 'build', '--locked', '--release', '-p', 'tsv_cli'];
	const addon_build = ['cargo', 'build', '--locked', '--profile', 'napi', '-p', 'tsv_napi'];
	await run_build(host, side, cli_build, log);
	await run_build(host, side, addon_build, log);
	const symbolized = {
		CARGO_PROFILE_RELEASE_STRIP: 'false',
		CARGO_PROFILE_NAPI_STRIP: 'false',
		CARGO_TARGET_DIR: `${side.dir}/target_symbols`
	};
	await run_build(host, side, cli_build, log, { env: symbolized });
	await run_build(host, side, addon_build, log, { env: symbolized });
	const wasm_pack_version = await tool_stdout(host, wasm_pack, ['--version'], log);
	const wasm = {} as Record<WasmVariant, string>;
	for (const variant of WASM_VARIANTS) {
		wasm[variant] = await build_wasm(host, side, variant, wasm_pack_version, log);
	}
	const artifacts: Artifacts = {
		cli: `${side.name}/target/release/tsv`,
		addon: `${side.name}/target/napi/libtsv_napi.so`,
		cli_symbols: `${side.name}/target_symbols/release/tsv`,
		addon_symbols: `${side.name}/target_symbols/napi/libtsv_napi.so`,
		wasm
	};
	for (const name of [
		artifacts.cli,
		artifacts.addon,
		artifacts.cli_symbols,
		artifacts.addon_symbols
	]) {
		if (!(await host.cache.dir.exists(name))) {
			throw new HarnessError(`the build produced no ${cache_path(host, name)}`);
		}
	}
	return artifacts;
}

/** The output directory of a built wasm-pack bundle (a `tsv_wasm.js` + `tsv_wasm_bg.wasm`),
 * beneath the cache. */
function wasm_out_dir(side: Side, variant: WasmVariant, wasm_pack_version: string): string {
	const slug = wasm_pack_version.trim().replaceAll(/[^0-9A-Za-z.]+/g, '_');
	return `${side.name}/wasm/${variant}-${slug}`;
}

/** Build one wasm package bundle as npm ships it (`--target web`), once per side: a bundle is
 * addressed by its tree, variant and wasm-pack version, so a finished one is reused. */
async function build_wasm(
	host: ReportHost,
	side: Side,
	variant: WasmVariant,
	wasm_pack_version: string,
	log: CommandLog
): Promise<string> {
	const cache = host.cache.dir;
	const out_dir = wasm_out_dir(side, variant, wasm_pack_version);
	const bundle = `${out_dir}/tsv_wasm_bg.wasm`;
	if (await cache.exists(bundle)) {
		log.record(`[${side.slot}] reusing ${cache_path(host, bundle)}`);
		return bundle;
	}
	const features = variant === 'all' ? [] : ['--no-default-features', '--features', variant];
	const partial = `${out_dir}.partial`;
	await cache.remove(partial, { recursive: true, missing_ok: true });
	await cache.mkdir(`${side.name}/wasm`, { recursive: true });
	await run_build(
		host,
		side,
		[
			'wasm-pack',
			'build',
			`${side.src}/crates/tsv_wasm`,
			'--target',
			'web',
			'--release',
			'--out-dir',
			cache_path(host, partial),
			...(features.length > 0 ? ['--', ...features] : [])
		],
		log
	);
	await cache.rename(partial, out_dir);
	return bundle;
}

// -- A. sizes -----------------------------------------------------------------------------------

/** `program args`'s stdout, run in the checkout, recorded in `log`; fails on anything but exit 0. */
async function tool_stdout(
	host: ReportHost,
	program: SpawnProgram,
	args: ReadonlyArray<string>,
	log: CommandLog
): Promise<string> {
	const line = `${program.name} ${args.join(' ')}`;
	log.record(line);
	const result = whole_output(await program.out(args, { cwd: host.checkout }), line);
	if (exit_code_of(result) !== 0) {
		throw new HarnessError(`${line} ${format_ending(result)}: ${result.stderr.trim()}`);
	}
	return result.stdout;
}

export async function leg_wasm_sizes(
	host: ReportHost,
	name: string,
	sides: readonly [Side, Side],
	artifacts: readonly [Artifacts, Artifacts]
): Promise<Leg> {
	return await run_leg(name, async (_, leg) => {
		const gzip = require_tool(host, 'gzip');
		const rows: Array<Row> = [];
		for (const variant of WASM_VARIANTS) {
			const sizes = await Promise.all(
				sides.map(async (_side, i) => {
					const bundle = artifacts[i]!.wasm[variant];
					return {
						raw: (await host.cache.dir.stat(bundle))!.size,
						gzip: await gzip_size(gzip, cache_path(host, bundle))
					};
				})
			);
			rows.push(
				{ key: `${variant} raw`, unit: 'B', base: sizes[0]!.raw, cand: sizes[1]!.raw, exact: true },
				{
					key: `${variant} gzip`,
					unit: 'B',
					base: sizes[0]!.gzip,
					cand: sizes[1]!.gzip,
					exact: true
				}
			);
		}
		for (const variant of WASM_VARIANTS) {
			leg.notes.push(
				await identity_note(
					host,
					`${variant}.wasm`,
					artifacts[0]!.wasm[variant],
					artifacts[1]!.wasm[variant]
				)
			);
		}
		leg.tables.push({
			id: 'wasm_bytes',
			title: 'WASM bundle bytes (`wasm-pack --target web`, what npm ships)',
			note: '`deno` and `nodejs` targets build the same crate with the same features; only the wasm-bindgen glue differs.',
			rows,
			display: 'all'
		});
	});
}

/** Whether two artifacts (paths beneath the cache) are the same bytes: a stronger reading than
 * every size row agreeing. */
async function identity_note(
	host: ReportHost,
	name: string,
	base: string,
	cand: string
): Promise<string> {
	const [a, b] = await Promise.all([host.cache.dir.read(base), host.cache.dir.read(cand)]);
	const [ha, hb] = await Promise.all([sha256_hex(a), sha256_hex(b)]);
	return ha === hb
		? `${name}: byte-identical across the two sides (sha256 ${ha.slice(0, 16)})`
		: `${name}: differs (sha256 ${ha.slice(0, 16)} vs ${hb.slice(0, 16)})`;
}

async function symbol_table(
	host: ReportHost,
	nm: SpawnProgram,
	binary: string,
	log: CommandLog
): Promise<SymbolSizes> {
	return assert_symbols(
		parse_nm(await tool_stdout(host, nm, ['-S', '--radix=d', binary], log)),
		binary
	);
}

async function bounds_counts(
	host: ReportHost,
	objdump: SpawnProgram,
	binary: string,
	log: CommandLog
): Promise<Map<string, number>> {
	return assert_bounds_sites(
		count_bounds_sites(await tool_stdout(host, objdump, ['-d', '--no-show-raw-insn', binary], log)),
		binary
	);
}

/** The per-symbol diff of one artifact as a report table: the top movers, with the totals. */
function movers_table(id: string, title: string, base: SymbolSizes, cand: SymbolSizes): Table {
	const diff = diff_symbols(base, cand);
	const total_delta = diff.cand_total - diff.base_total;
	return {
		id,
		title,
		note:
			`Sum of symbol sizes: base ${diff.base_total.toLocaleString('en-US')} B, cand ${diff.cand_total.toLocaleString('en-US')} B ` +
			`(${total_delta > 0 ? '+' : ''}${total_delta.toLocaleString('en-US')} B). ` +
			`${diff.changed} changed, ${diff.added} added, ${diff.removed} removed; the widest ${Math.min(diff.movers.length, TOP_MOVERS)} movers:`,
		rows: diff.movers.slice(0, TOP_MOVERS).map((m) => ({
			key: m.key,
			unit: 'B',
			base: m.base,
			cand: m.cand,
			exact: true
		})),
		display: 'all'
	};
}

export async function leg_native_sizes(
	host: ReportHost,
	name: string,
	sides: readonly [Side, Side],
	artifacts: readonly [Artifacts, Artifacts]
): Promise<Leg> {
	return await run_leg(name, async (log, leg) => {
		const nm = require_tool(host, 'nm');
		const size = require_tool(host, 'size');
		const objdump = require_tool(host, 'objdump');
		for (const [label, a, b] of [
			['cli', artifacts[0]!.cli, artifacts[1]!.cli],
			['addon', artifacts[0]!.addon, artifacts[1]!.addon],
			['cli (symbolized)', artifacts[0]!.cli_symbols, artifacts[1]!.cli_symbols],
			['addon (symbolized)', artifacts[0]!.addon_symbols, artifacts[1]!.addon_symbols]
		] as const) {
			leg.notes.push(await identity_note(host, label, a, b));
		}
		const named = (a: Artifacts): ReadonlyArray<readonly [string, string, string]> => [
			['cli (tsv)', a.cli, a.cli_symbols],
			['addon (tsv_napi)', a.addon, a.addon_symbols]
		];
		const file_rows: Array<Row> = [];
		const section_rows: Array<Row> = [];
		const bounds_rows: Array<Row> = [];
		for (let i = 0; i < 2; i++) {
			const [name_a, path_a, symbols_path_a] = named(artifacts[0]!)[i]!;
			const [, path_b, symbols_path_b] = named(artifacts[1]!)[i]!;
			file_rows.push({
				key: `${name_a} file`,
				unit: 'B',
				base: (await host.cache.dir.stat(path_a))!.size,
				cand: (await host.cache.dir.stat(path_b))!.size,
				exact: true
			});
			const sections = await Promise.all(
				[path_a, path_b].map(async (path) =>
					parse_size_sections(
						await tool_stdout(host, size, ['-A', '-d', cache_path(host, path)], log)
					)
				)
			);
			for (const sec of ['.text', '.rodata', '.data.rel.ro', '.eh_frame']) {
				section_rows.push({
					key: `${name_a} ${sec}`,
					unit: 'B',
					base: sections[0]!.get(sec) ?? null,
					cand: sections[1]!.get(sec) ?? null,
					exact: true
				});
			}
			const [symbols_a, symbols_b] = [
				await symbol_table(host, nm, cache_path(host, symbols_path_a), log),
				await symbol_table(host, nm, cache_path(host, symbols_path_b), log)
			];
			leg.tables.push(
				movers_table(
					`symbols_${i === 0 ? 'cli' : 'addon'}`,
					`${name_a}: per-symbol \`.text\` of the symbolized build (\`nm -S\`, build hash removed, copies summed)`,
					symbols_a,
					symbols_b
				)
			);
			const [bounds_a, bounds_b] = [
				await bounds_counts(host, objdump, cache_path(host, symbols_path_a), log),
				await bounds_counts(host, objdump, cache_path(host, symbols_path_b), log)
			];
			for (const { name: callee } of BOUNDS_CALLEES) {
				bounds_rows.push({
					key: `${name_a}: ${callee}`,
					unit: 'sites',
					base: bounds_a.get(callee)!,
					cand: bounds_b.get(callee)!,
					exact: true
				});
			}
		}
		leg.tables.unshift(
			{
				id: 'native_files',
				title: 'Native artifact files (the plain release/napi recipes, stripped by their profile)',
				rows: file_rows,
				display: 'all'
			},
			{
				id: 'native_sections',
				title: 'Native sections (`size -A`)',
				rows: section_rows,
				display: 'all'
			},
			{
				id: 'bounds_sites',
				title: 'Bounds-miss call sites (`objdump -d`: `call`/`jmp` per panic edge)',
				rows: bounds_rows,
				display: 'all'
			}
		);
	});
}

// -- B. type sizes ------------------------------------------------------------------------------

function type_tables(
	id: string,
	title: string,
	base: Array<TypeRowSize>,
	cand: Array<TypeRowSize>
): Table {
	assert_parsed_rows(base, `the base side's ${title}`);
	assert_parsed_rows(cand, `the candidate side's ${title}`);
	const map = (rows: Array<TypeRowSize>): Map<string, number> => {
		const m = new Map<string, number>();
		for (const r of rows) {
			m.set(`${r.group} ${r.name}`, r.size);
			m.set(`${r.group} ${r.name} (Result)`, r.result_size);
		}
		return m;
	};
	return { id, title, rows: exact_rows(map(base), map(cand), 'B'), display: 'changed' };
}

export async function leg_type_sizes_host(
	host: ReportHost,
	sides: readonly [Side, Side]
): Promise<Leg> {
	return await run_leg('type sizes, x86-64 (tsv_debug type_sizes)', async (log, leg) => {
		const boards: Array<Array<TypeRowSize>> = [];
		for (const side of sides) {
			if (!(await side.src_handle.exists(BOARD_SOURCE))) {
				throw new UnavailableOnRef(`${side.slot} ${side.ref} has no ${BOARD_SOURCE}`);
			}
			await run_build(
				host,
				side,
				['cargo', 'build', '--locked', '--profile', 'corpus', '-p', 'tsv_debug'],
				log
			);
			const tsv_debug = host.cache.side_program(`${side.name}/target/corpus/tsv_debug`);
			log.record(`[${side.slot}] ${tsv_debug.path} type_sizes --json`);
			const result = whole_output(
				await tsv_debug.out(['type_sizes', '--json'], { cwd: host.checkout }),
				'type_sizes'
			);
			if (exit_code_of(result) !== 0) throw new HarnessError(`type_sizes ${format_ending(result)}`);
			boards.push(parse_type_sizes_json(result.stdout));
		}
		leg.tables.push(
			type_tables('types_x86_64', 'Public AST and foundation types, x86-64', boards[0]!, boards[1]!)
		);
	});
}

export async function leg_type_sizes_wasm(
	host: ReportHost,
	sides: readonly [Side, Side]
): Promise<Leg> {
	return await run_leg('type sizes, wasm32 (scratch crate over the board)', async (log, leg) => {
		const boards: Array<Array<TypeRowSize>> = [];
		for (const side of sides) {
			const src = side.src_handle;
			const dir = side.dir_handle;
			if (!(await src.exists(BOARD_SOURCE))) {
				throw new UnavailableOnRef(`${side.slot} ${side.ref} has no ${BOARD_SOURCE}`);
			}
			await dir.mkdir('type_board/src', { recursive: true });
			await dir.write('type_board/src/lib.rs', wasm_board_lib(await src.read_text(BOARD_SOURCE)));
			await dir.write('type_board/Cargo.toml', wasm_board_manifest('../src'));
			await dir.mkdir('type_board/.cargo', { recursive: true });
			for (const name of ['Cargo.lock', 'rust-toolchain.toml', '.cargo/config.toml']) {
				const bytes = await src.read_if_exists(name);
				if (bytes !== null) await dir.write(`type_board/${name}`, bytes);
			}
			await run_build(
				host,
				side,
				['cargo', 'build', '--offline', '--release', '--target', 'wasm32-unknown-unknown'],
				log,
				{ cwd: dir.dir('type_board'), env: { CARGO_TARGET_DIR: `${side.dir}/target_board` } }
			);
			const wasm = 'target_board/wasm32-unknown-unknown/release/tsv_type_board_wasm.wasm';
			const bytes = await dir.read_if_exists(wasm);
			if (bytes === null) {
				throw new HarnessError(`the board build produced no ${side.dir}/${wasm}`);
			}
			const instance = new WebAssembly.Instance(new WebAssembly.Module(new Uint8Array(bytes)), {});
			boards.push(parse_board_text(board_text_from_exports(instance.exports)));
		}
		leg.tables.push(
			type_tables('types_wasm32', 'Public AST and foundation types, wasm32', boards[0]!, boards[1]!)
		);
	});
}

// -- C. work counters --------------------------------------------------------------------------

/** The corpus a work-counter leg formats, listed by the side's own discovery into a file under
 * `scratch` (a directory beneath the cache); its absolute path. */
async function corpus_file_list(
	host: ReportHost,
	cli: SpawnProgram,
	corpus: string,
	scratch: string,
	slot: string
): Promise<string> {
	const listing = whole_output(
		await cli.out(['format', '--list', corpus], { cwd: host.checkout }),
		'tsv format --list'
	);
	if (exit_code_of(listing) !== 0) {
		throw new HarnessError(`tsv format --list ${format_ending(listing)}: ${listing.stderr.trim()}`);
	}
	const name = `${scratch}/file_list.${slot}.${crypto.randomUUID()}.txt`;
	await host.cache.dir.write(name, listing.stdout);
	return cache_path(host, name);
}

const FORMAT_CHECK_ARGS = (corpus: string): Array<string> => [
	'format',
	'--check',
	'--jobs',
	'1',
	corpus
];

/** heaptrack_print's report over a whole corpus runs long; capped well above it. */
const HEAPTRACK_PRINT_MAX_BYTES = 512 * 1024 * 1024;

export async function leg_allocations(
	host: ReportHost,
	sides: readonly [Side, Side],
	artifacts: readonly [Artifacts, Artifacts],
	corpus: string,
	scratch: string
): Promise<Leg> {
	return await run_leg('native allocation counts (heaptrack)', async (log, leg) => {
		const heaptrack = require_tool(host, 'heaptrack');
		const heaptrack_print = require_tool(host, 'heaptrack_print');
		const cache = host.cache.dir;
		const summaries = [];
		for (let i = 0; i < 2; i++) {
			const cli = host.cache.side_program(artifacts[i]!.cli);
			const prefix = `${scratch}/heaptrack.${sides[i]!.slot}.${crypto.randomUUID()}`;
			const args = [
				'--record-only',
				'-o',
				cache_path(host, prefix),
				cli.path,
				...FORMAT_CHECK_ARGS(corpus)
			];
			log.record(`heaptrack ${args.join(' ')}`);
			const recorded = await heaptrack.out(args, { cwd: host.checkout });
			assert_format_check_finished(
				exit_code_or_throw(recorded, 'heaptrack format --check'),
				recorded.stderr,
				'heaptrack format --check'
			);
			let data: string | null = null;
			for (const ext of ['.zst', '.gz']) {
				if (data === null && (await cache.exists(`${prefix}${ext}`))) data = `${prefix}${ext}`;
			}
			if (data === null) {
				throw new HarnessError(`heaptrack wrote no data file at ${cache_path(host, prefix)}.zst`);
			}
			log.record(`heaptrack_print -f ${cache_path(host, data)}`);
			const printed = whole_output(
				await heaptrack_print.out(['-f', cache_path(host, data)], {
					cwd: host.checkout,
					max_bytes: HEAPTRACK_PRINT_MAX_BYTES
				}),
				'heaptrack_print'
			);
			await cache.remove(data, { missing_ok: true });
			if (exit_code_of(printed) !== 0) {
				throw new HarnessError(`heaptrack_print ${format_ending(printed)}`);
			}
			const summary = parse_heaptrack_summary(printed.stdout);
			if (summary === null)
				throw new HarnessError(
					`heaptrack_print printed no summary:\n${printed.stdout.slice(-800)}`
				);
			summaries.push(summary);
		}
		const [a, b] = [summaries[0]!, summaries[1]!];
		leg.tables.push({
			id: 'allocations',
			title: 'CLI `format --check --jobs 1` over the corpus, native heap (heaptrack)',
			note: "WASM allocation counts are not measured: nothing instruments the wasm build's allocator; its high-water is below.",
			rows: [
				{
					key: 'allocation calls',
					unit: '',
					base: a.allocation_calls,
					cand: b.allocation_calls,
					exact: true
				},
				{
					key: 'temporary allocations',
					unit: '',
					base: a.temporary_allocations,
					cand: b.temporary_allocations,
					exact: false,
					note: 'heaptrack infers a temporary from adjacency in the merged trace, which depends on how the main and worker threads interleave'
				},
				{
					key: 'peak heap (heaptrack rounding)',
					unit: 'B',
					base: a.peak_heap_bytes,
					cand: b.peak_heap_bytes,
					exact: true
				}
			],
			display: 'all'
		});
	});
}

/** Peak RSS samples per format run; the minimum of a few, since a peak only reads high. */
const RSS_SAMPLES = 3;

export async function leg_rss(
	host: ReportHost,
	sides: readonly [Side, Side],
	artifacts: readonly [Artifacts, Artifacts],
	corpus: string
): Promise<Leg> {
	return await run_leg('native peak RSS (/usr/bin/time)', async (log, leg) => {
		const time = require_tool(host, 'time');
		const peaks: Array<number> = [];
		for (let i = 0; i < 2; i++) {
			const cli = host.cache.side_program(artifacts[i]!.cli);
			const args = ['-f', '%M', cli.path, ...FORMAT_CHECK_ARGS(corpus)];
			log.record(`${time.path} ${args.join(' ')} (minimum of ${RSS_SAMPLES})`);
			const samples: Array<number> = [];
			for (let s = 0; s < RSS_SAMPLES; s++) {
				const result = await time.out(args, { cwd: host.checkout });
				assert_format_check_finished(
					exit_code_or_throw(result, 'format --check'),
					result.stderr,
					'format --check'
				);
				const kib = parse_rss_kib(result.stderr);
				if (kib === null) throw new HarnessError(`no RSS on stderr: ${result.stderr || '(empty)'}`);
				samples.push(kib * 1024);
			}
			peaks.push(Math.min(...samples));
		}
		leg.tables.push({
			id: 'peak_rss',
			title: 'CLI `format --check --jobs 1` over the corpus, peak RSS (minimum of three runs)',
			note: "A measured row, not an exact one: ASLR and the kernel's page accounting move it by a few pages between identical runs.",
			rows: [{ key: 'peak RSS', unit: 'B', base: peaks[0]!, cand: peaks[1]!, exact: false }],
			display: 'all'
		});
	});
}

/** The probe each WASM-memory sweep runs under `node`, a path in the checkout. */
export const WASM_MEMORY_PROBE = 'scripts/roots/wasm_memory.ts';

export async function leg_wasm_memory(
	host: ReportHost,
	sides: readonly [Side, Side],
	artifacts: readonly [Artifacts, Artifacts],
	corpus: string,
	scratch: string
): Promise<Leg> {
	return await run_leg('WASM linear-memory high-water', async (log, leg) => {
		const node = require_tool(host, 'node');
		const probe = `${host.checkout.path}/${WASM_MEMORY_PROBE}`;
		const rows: Array<Row> = [];
		const surfaces = [
			['format', 'format'],
			['parse', 'parse'],
			['all', 'format'],
			['all', 'parse']
		] as const;
		const lists = await Promise.all(
			sides.map((side, i) =>
				corpus_file_list(
					host,
					host.cache.side_program(artifacts[i]!.cli),
					corpus,
					scratch,
					side.slot
				)
			)
		);
		const shared_rows = new Map<string, [number | null, number | null]>();
		for (const [variant, surface] of surfaces) {
			for (let i = 0; i < 2; i++) {
				const glue = cache_path(host, artifacts[i]!.wasm[variant]).replace(
					/tsv_wasm_bg\.wasm$/,
					'tsv_wasm.js'
				);
				const args = [probe, glue, lists[i]!, surface];
				log.record(`[${sides[i]!.slot}] node ${args.join(' ')}`);
				const result = whole_output(await node.out(args, { cwd: host.checkout }), 'wasm_memory');
				if (exit_code_of(result) !== 0) {
					throw new HarnessError(`wasm_memory ${format_ending(result)}: ${result.stderr.trim()}`);
				}
				const parsed = JSON.parse(result.stdout) as WasmMemoryResult;
				assert_wasm_sweep(parsed, `wasm_memory ${sides[i]!.slot} ${variant}/${surface}`);
				const label = `${variant} bundle, ${surface}`;
				const set = (key: string, value: number): void => {
					const pair = shared_rows.get(key) ?? [null, null];
					pair[i] = value;
					shared_rows.set(key, pair);
				};
				set(`${label}: warm high-water (whole corpus)`, parsed.warm_high_water_bytes);
				for (const [language, bytes] of Object.entries(parsed.cold_largest_bytes).sort()) {
					set(`${label}: cold, largest ${language}`, bytes);
				}
			}
		}
		for (const [key, [base, cand]] of shared_rows)
			rows.push({ key, unit: 'B', base, cand, exact: true });
		leg.tables.push({
			id: 'wasm_memory',
			title: 'WASM linear memory, pages × 64 KiB (`memory.buffer.byteLength` after the sweep)',
			rows,
			display: 'all'
		});
	});
}

/** The depth limits, read from a tree's `tsv_lang` source. */
async function depth_limit_constants(side: Side): Promise<Map<string, number>> {
	const src = side.src_handle;
	const constants = new Map<string, number>();
	const walk = async (dir: string): Promise<void> => {
		for await (const entry of src.list(dir)) {
			const path = `${dir}/${entry.name}`;
			if (entry.kind === 'directory') await walk(path);
			else if (entry.name.endsWith('.rs')) {
				const text = await src.read_text(path);
				for (const match of text.matchAll(
					/pub const (NESTING_LIMIT|SPINE_LIMIT): \w+ = ([0-9_]+);/g
				)) {
					constants.set(match[1]!, Number(match[2]!.replaceAll('_', '')));
				}
			}
		}
	};
	await walk('crates/tsv_lang/src');
	return constants;
}

/** The contract's thread in an optimized build, less the tenth the gate keeps. */
const GATE_STACK_BYTES = Math.floor((2 << 20) / 10) * 9;
const MARGIN_LOW_BYTES = 256 * 1024;
const MARGIN_STEP_BYTES = 64 * 1024;

export async function leg_depth(
	host: ReportHost,
	sides: readonly [Side, Side],
	options: { margin: boolean }
): Promise<Leg> {
	return await run_leg(
		'recursion depth (tests/depth_limit.rs, `corpus` profile)',
		async (log, leg) => {
			const shape_rows = new Map<string, [number | null, number | null]>();
			const limit_rows = new Map<string, [number | null, number | null]>();
			const margin_rows = new Map<string, [number | null, number | null]>();
			for (let i = 0; i < 2; i++) {
				const side = sides[i]!;
				const test_args = [
					'cargo',
					'test',
					'--locked',
					'--profile',
					'corpus',
					'-p',
					'tsv_tests',
					'--test',
					'depth_limit',
					'--no-run'
				];
				await run_build(host, side, test_args, log);
				const shapes_file = `${side.dir}/shape_limits.tsv`;
				await run_build(
					host,
					side,
					[
						'cargo',
						'test',
						'--locked',
						'--profile',
						'corpus',
						'-p',
						'tsv_tests',
						'--test',
						'depth_limit',
						'write_shape_limits',
						'--',
						'--ignored',
						'--exact',
						'write_shape_limits'
					],
					log,
					{ env: { TSV_DEPTH_SHAPES: shapes_file } }
				);
				for (const { name, lang, n } of assert_parsed_rows(
					parse_shape_limits(await side.dir_handle.read_text('shape_limits.tsv')),
					`the ${side.slot} shape-limit file`
				)) {
					const key = `${lang} ${name}`;
					const pair = shape_rows.get(key) ?? [null, null];
					pair[i] = n;
					shape_rows.set(key, pair);
				}
				for (const [name, value] of await depth_limit_constants(side)) {
					const pair = limit_rows.get(name) ?? [null, null];
					pair[i] = value;
					limit_rows.set(name, pair);
				}
				if (options.margin) {
					const smallest = await search_stack_margin(host, side, log);
					const key = 'smallest passing thread (64 KiB steps)';
					const pair = margin_rows.get(key) ?? [null, null];
					pair[i] = smallest;
					margin_rows.set(key, pair);
				}
			}
			const to_rows = (
				m: Map<string, [number | null, number | null]>,
				unit: string,
				exact = true
			): Array<Row> =>
				[...m]
					.sort(([a], [b]) => a.localeCompare(b))
					.map(([key, [base, cand]]) => ({ key, unit, base, cand, exact }));
			leg.tables.push({
				id: 'depth_limits',
				title: 'Depth limits (tsv_lang `NESTING_LIMIT`, `SPINE_LIMIT`)',
				rows: to_rows(limit_rows, 'levels'),
				display: 'all'
			});
			leg.tables.push({
				id: 'depth_shapes',
				title:
					'Per-shape accepted depth `n` (`write_shape_limits`, the limits the depth gate grades against)',
				note: 'The depth limits fix these; a move is a grammar or limit change, not a frame-size effect. The frame-size effect is the stack margin below.',
				rows: to_rows(shape_rows, 'n'),
				display: 'changed'
			});
			if (options.margin) {
				leg.tables.push({
					id: 'depth_margin',
					title:
						'Stack margin: the smallest thread `every_shape_holds_its_limit_on_the_contract_stack` survives, `corpus` profile',
					note: "The gate's own contract is 2 MiB in the shipped optimization; the `corpus` world's frames differ from it. Both sides are the same profile, so this is a comparison, not the contract.",
					rows: to_rows(margin_rows, 'B', false),
					display: 'all'
				});
			}
		}
	);
}

/** Bisect the smallest `TSV_DEPTH_GATE_STACK` the stack leg survives, at 64 KiB resolution. */
async function search_stack_margin(host: ReportHost, side: Side, log: CommandLog): Promise<number> {
	const passes = async (stack: number): Promise<boolean> => {
		const gate = host.build({
			CARGO_TARGET_DIR: side.target,
			TSV_DEPTH_GATE_STACK: String(stack)
		});
		const status = await gate.out(
			[
				...NICE_ARGS,
				'cargo',
				'test',
				'--locked',
				'--profile',
				'corpus',
				'-p',
				'tsv_tests',
				'--test',
				'depth_limit',
				'every_shape_holds_its_limit_on_the_contract_stack'
			],
			{ cwd: side.src_handle }
		);
		const stderr = status.stderr;
		if (exit_code_of(status) === 0) return test_run_matched(status.stdout);
		// an overflow aborts the test binary; anything else is a failed gate, not a search result
		if (
			stderr.includes('stack overflow') ||
			stderr.includes('overflowed its stack') ||
			status.kind === 'signaled'
		)
			return false;
		throw new HarnessError(
			`depth gate failed at stack ${stack} for reasons other than overflow:\n${stderr.slice(-2000)}`
		);
	};
	log.record(
		`[${side.slot}] TSV_DEPTH_GATE_STACK bisect over ${MARGIN_LOW_BYTES}..${GATE_STACK_BYTES}, ${MARGIN_STEP_BYTES} B steps (cargo test ... every_shape_holds_its_limit_on_the_contract_stack)`
	);
	if (!(await passes(GATE_STACK_BYTES)))
		throw new HarnessError('the depth gate fails at its own stack');
	let low = MARGIN_LOW_BYTES;
	let high = GATE_STACK_BYTES;
	// invariant: `high` passes; below the floor `low` is not searched, reported as the floor
	if (await passes(low)) return low;
	while (high - low > MARGIN_STEP_BYTES) {
		const mid = Math.floor((low + high) / 2 / MARGIN_STEP_BYTES) * MARGIN_STEP_BYTES;
		const probe = mid <= low ? low + MARGIN_STEP_BYTES : mid;
		if (await passes(probe)) high = probe;
		else low = probe;
	}
	return high;
}
