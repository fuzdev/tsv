/**
 * Which side's file each two-sided leg reads, over fakes that stamp every answer with the side
 * whose file they were handed: a leg that read one side's bundle, CLI, or file list for both, or
 * filed a side's number under the other, puts the wrong stamp in a row.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';

import { fs, type FsDir } from '@fuzdev/gro/fs.ts';
import { test, vi } from 'vitest';

import { type Artifacts, leg_wasm_memory, leg_wasm_sizes } from './legs.ts';
import { type FakeCall, fake_host, fake_program, fake_side } from './report_fakes.ts';
import type { Side } from './side.ts';
import { leg_plain, type TimingOptions } from './timing.ts';
import { run_branch_report_command } from './branch_report.ts';

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

/** The stamp of the side whose file `text` names: 1 for base, 2 for cand. */
const stamp_of = (sides: readonly [Side, Side], text: string): number =>
	text.includes(sides[0].name) || text.includes('.base.')
		? 1
		: text.includes(sides[1].name) || text.includes('.cand.')
			? 2
			: 0;

const two_sides = async (cache: FsDir): Promise<readonly [Side, Side]> =>
	[
		await fake_side(cache, 'base', {}, 'a'.repeat(40)),
		await fake_side(cache, 'cand', {}, 'b'.repeat(40))
	] as const;

test('the WASM size rows read each side its own bundle', async () => {
	await using cache = await fs.temp_dir('branch_report_cache-');
	await using checkout = await fs.temp_dir('branch_report_checkout-');
	const sides = await two_sides(cache);
	const artifacts = [artifacts_of(sides[0]), artifacts_of(sides[1])] as const;
	for (const [i, a] of artifacts.entries()) {
		for (const bundle of Object.values(a.wasm)) {
			await cache.mkdir(bundle.slice(0, bundle.lastIndexOf('/')), { recursive: true });
			await cache.write(bundle, 'x'.repeat(10 * (i + 1)));
		}
	}
	const calls: Array<FakeCall> = [];
	const gzip = fake_program('gzip', calls, (call) => ({
		stdout: 'z'.repeat(stamp_of(sides, call.args[1]!))
	}));
	const host = fake_host(cache, checkout, calls, { tools: { gzip } });
	const leg = await leg_wasm_sizes(host, 'WASM bundle bytes', sides, artifacts);
	strictEqual(leg.status, 'ok', leg.reason);
	for (const row of leg.tables[0]!.rows) {
		deepStrictEqual([row.base, row.cand], row.key.endsWith('raw') ? [10, 20] : [1, 2], row.key);
	}
});

test('the WASM-memory rows run each side its own glue over its own file list', async () => {
	await using cache = await fs.temp_dir('branch_report_cache-');
	await using checkout = await fs.temp_dir('branch_report_checkout-');
	const sides = await two_sides(cache);
	const artifacts = [artifacts_of(sides[0]), artifacts_of(sides[1])] as const;
	await cache.mkdir('scratch.1');
	const calls: Array<FakeCall> = [];
	const node = fake_program('node', calls, (call) => {
		const [, glue, list] = call.args;
		const glue_stamp = stamp_of(sides, glue!);
		const list_stamp = stamp_of(sides, list!);
		return {
			stdout: JSON.stringify({
				surface: call.args[3],
				files: 1,
				errors: 0,
				first_error: null,
				warm_high_water_bytes: glue_stamp,
				cold_largest_bytes: { typescript: list_stamp }
			})
		};
	});
	const host = fake_host(cache, checkout, calls, {
		tools: { node },
		side: (call) => ({ stdout: `${call.program}.listed\n` })
	});
	const leg = await leg_wasm_memory(host, sides, artifacts, '/corpus', 'scratch.1');
	strictEqual(leg.status, 'ok', leg.reason);
	// each side's list was made by its own CLI
	deepStrictEqual(
		calls.filter((c) => c.args[0] === 'format').map((c) => c.program),
		artifacts.map((a) => a.cli)
	);
	for (const row of leg.tables[0]!.rows) deepStrictEqual([row.base, row.cand], [1, 2], row.key);
});

test('the plain CLI pair counts each side its own CLI', async () => {
	await using cache = await fs.temp_dir('branch_report_cache-');
	await using checkout = await fs.temp_dir('branch_report_checkout-');
	const sides = await two_sides(cache);
	const artifacts = [artifacts_of(sides[0]), artifacts_of(sides[1])] as const;
	const calls: Array<FakeCall> = [];
	const perf = fake_program('perf', calls, async (call) => {
		const out = call.args[call.args.indexOf('-o') + 1]!;
		const cli = call.args[call.args.indexOf('--') + 1]!;
		const n = stamp_of(sides, cli);
		await cache.write(
			out.slice(cache.path.length + 1),
			`${n},,instructions:u\n${n * 10},,cycles:u\n`
		);
	});
	const perf_ab = fake_program('node', calls, () => ({
		stdout: JSON.stringify({ rows: [], cycles: {} })
	}));
	const host = fake_host(cache, checkout, calls, {
		tools: { perf, perf_ab },
		side: () => ({ stdout: '/corpus/a.ts\n' })
	});
	const options: TimingOptions = {
		legs: new Set(['plain']),
		rounds: 2,
		iterations: 1,
		corpus: '/corpus',
		dry_run: false,
		bench_runtimes: ['node'],
		bench_env: {}
	};
	const leg = await leg_plain(host, sides, artifacts, options);
	strictEqual(leg.status, 'ok', leg.reason);
	const rows = leg.tables[1]!.rows.filter((r) => !r.key.endsWith('wall'));
	strictEqual(rows.length > 0, true);
	for (const row of rows) {
		deepStrictEqual([row.base, row.cand], row.key.includes('cycles') ? [10, 20] : [1, 2], row.key);
	}
});

test('a bench runtime the report does not know, or one named twice, is a usage error, never a gap', async () => {
	for (const [runtimes, refusal] of [
		['nod', "unknown bench runtime 'nod'"],
		// run twice, its samples would merge into one row behind a round count the report states
		['node,deno,node', "bench runtime 'node' named twice"]
	]) {
		await using cache = await fs.temp_dir('branch_report_cache-');
		await using checkout = await fs.temp_dir('branch_report_checkout-');
		const calls: Array<FakeCall> = [];
		const host = fake_host(cache, checkout, calls);
		const deps = {
			...host,
			home: '/home/nobody',
			cargo_build_jobs: undefined,
			bench_env: {},
			open_cache: async () => host.cache,
			open_made_dir: async () => cache,
			load_1m: () => 0
		};
		const errors: Array<string> = [];
		const spy = vi
			.spyOn(console, 'error')
			.mockImplementation((...a: unknown[]) => void errors.push(a.join(' ')));
		const args = ['a', 'b', '--corpus', checkout.path, '--bench-runtimes', runtimes!];
		const code = await run_branch_report_command(deps, args);
		spy.mockRestore();
		strictEqual(code, 2, runtimes);
		ok(
			errors.some((line) => line.includes(refusal!)),
			errors.join('\n')
		);
		// refused before any ref is resolved
		deepStrictEqual(calls, [], runtimes);
		strictEqual(await cache.exists('reports'), false, runtimes);
	}
});
