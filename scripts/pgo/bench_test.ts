/**
 * `run_pgo_bench` over a temp checkout and a fake `node` that plays the PGO pipeline, writing
 * the files a run leaves in `.pgo/`: the argv and working directory it gets, the copies'
 * bytes and modes, the bench stamp, and each way it refuses to stage. No pipeline runs.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { createHash } from 'node:crypto';
import node_process from 'node:process';

import { fs, type FsDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram, SpawnResult } from '@fuzdev/gro/spawn.ts';
import { test } from 'vitest';

import { CORPORA_ROOT } from '../../benches/js/lib/corpora.ts';
import { GATE_CHECKOUT_IDS } from '../../benches/js/lib/gate_counts.ts';
import {
	PGO_BENCH_DIR,
	PGO_BENCH_STAMP_VERSION,
	type PgoBenchStamp,
	pgo_bench_library_name
} from '../../benches/js/lib/pgo_bench.ts';
import { node_host_triple } from '../napi_host.ts';
import { pgo_bench_pipeline_args, run_pgo_bench } from './bench.ts';
import { cargo_artifacts } from './layout.ts';

const report = node_process.report?.getReport() as { header?: { glibcVersionRuntime?: string } };
const TRIPLE = node_host_triple({
	platform: node_process.platform,
	arch: node_process.arch,
	glibc: Boolean(report?.header?.glibcVersionRuntime)
});
const BUILT = cargo_artifacts('target-use', TRIPLE);
const NOW = new Date(Date.UTC(2026, 0, 2, 3, 4, 5));
const LIBRARY = { ffi: 'the ffi library bytes', napi: 'the napi addon bytes' };
const PROFILE = { ffi: 'the ffi profile', napi: 'the napi profile' };

const sha256 = (text: string): string => createHash('sha256').update(text).digest('hex');

/** What a pipeline run leaves in `.pgo/`; `recorded` overrides the sha256 its stamp records
 * for a world's library. */
const write_pipeline_output = async (
	checkout: FsDir,
	recorded: Partial<Record<'ffi' | 'napi', string>> = {}
): Promise<void> => {
	await checkout.mkdir('.pgo/target-use/release', { recursive: true });
	await checkout.mkdir('.pgo/target-use/napi', { recursive: true });
	const pgo = checkout.dir('.pgo');
	await pgo.write(BUILT.ffi, LIBRARY.ffi);
	await pgo.set_mode(BUILT.ffi, 0o750);
	await pgo.write(BUILT.addon, LIBRARY.napi);
	await pgo.set_mode(BUILT.addon, 0o755);
	await pgo.write('ffi.profdata', PROFILE.ffi);
	await pgo.write('napi.profdata', PROFILE.napi);
	await pgo.write('corpus.json', JSON.stringify({ own: ['own_a', 'own_b'], gitignore: null }));
	await pgo.write(
		'stamp.json',
		JSON.stringify({
			artifacts: {
				ffi: { sha256: recorded.ffi ?? sha256(LIBRARY.ffi), profile_sha256: sha256(PROFILE.ffi) },
				napi: {
					sha256: recorded.napi ?? sha256(LIBRARY.napi),
					profile_sha256: sha256(PROFILE.napi)
				}
			}
		})
	);
};

interface Call {
	args: ReadonlyArray<string>;
	cwd: string | undefined;
}

/** A `node` that records each `run`, plays the pipeline with `play`, and ends with `result`. */
const fake_node = (
	play: () => Promise<void>,
	result: SpawnResult = { kind: 'exited', code: 0 }
): { program: SpawnProgram; calls: Array<Call> } => {
	const calls: Array<Call> = [];
	const unused = (): never => {
		throw new Error('node: only run is faked');
	};
	const program: SpawnProgram = {
		name: 'node',
		path: '/fake/node',
		run: async (args, options) => {
			calls.push({ args, cwd: options?.cwd?.path });
			await play();
			return result;
		},
		out: unused,
		out_bytes: unused,
		start: unused
	};
	return { program, calls };
};

const run = async (
	checkout: FsDir,
	node: SpawnProgram,
	force = false
): Promise<{ code: number; out: Array<string>; err: Array<string> }> => {
	const out: Array<string> = [];
	const err: Array<string> = [];
	const code = await run_pgo_bench({
		checkout,
		node,
		host_triple: TRIPLE,
		force,
		now: () => NOW,
		print: (line) => out.push(line),
		print_error: (line) => err.push(line)
	});
	return { code, out, err };
};

test('the pipeline runs in the checkout, then both libraries and the stamp are staged', async () => {
	await using checkout = await fs.temp_dir('tsv_pgo_bench_test_');
	const node = fake_node(() => write_pipeline_output(checkout));
	const { code, out, err } = await run(checkout, node.program);
	deepStrictEqual([code, err], [0, []]);
	deepStrictEqual(node.calls, [
		{
			args: ['scripts/pgo/pgo.ts', 'all', '--ffi', '--no-depth', '--reuse', '--dir', '.pgo'],
			cwd: checkout.path
		}
	]);
	deepStrictEqual(out, [
		`build:bench:pgo: staged the PGO FFI library and N-API addon in ${PGO_BENCH_DIR}`
	]);
	const bench = checkout.dir('.pgo/bench');
	for (const [world, mode] of [
		['ffi', 0o750],
		['napi', 0o755]
	] as const) {
		const name = pgo_bench_library_name(world);
		strictEqual(await bench.read_text(name), LIBRARY[world]);
		strictEqual((await bench.stat(name))!.mode, mode);
	}
	const stamp = JSON.parse(await bench.read_text('stamp.json')) as PgoBenchStamp;
	deepStrictEqual(stamp, {
		version: PGO_BENCH_STAMP_VERSION,
		generated: NOW.toISOString(),
		training: {
			origin: 'own',
			collections: ['own_a', 'own_b'],
			snapshot_tree: GATE_CHECKOUT_IDS[CORPORA_ROOT]!.hash
		},
		artifacts: {
			ffi: {
				sha256: sha256(LIBRARY.ffi),
				profile_sha256: sha256(PROFILE.ffi),
				trained_through: stamp.artifacts.ffi.trained_through
			},
			napi: {
				sha256: sha256(LIBRARY.napi),
				profile_sha256: sha256(PROFILE.napi),
				trained_through: stamp.artifacts.napi.trained_through
			}
		}
	});
	ok(stamp.artifacts.ffi.trained_through.includes('under Deno'));
	ok(stamp.artifacts.napi.trained_through.includes('cli.js'));
	strictEqual(
		await bench.read_text('stamp.json'),
		JSON.stringify(stamp, null, '\t') + '\n',
		'tab-indented, one trailing newline'
	);
});

test('TSV_BUILD_FORCE drops --reuse', async () => {
	await using checkout = await fs.temp_dir('tsv_pgo_bench_test_');
	const node = fake_node(() => write_pipeline_output(checkout));
	strictEqual((await run(checkout, node.program, true)).code, 0);
	deepStrictEqual(node.calls[0]!.args, pgo_bench_pipeline_args(true));
	strictEqual(node.calls[0]!.args.includes('--reuse'), false);
});

test('a failed pipeline stages nothing, naming its exit code', async () => {
	for (const [result, shown] of [
		[{ kind: 'exited', code: 3 }, 'exit 3'],
		[{ kind: 'signaled', signal: 'SIGTERM' }, 'exit 143']
	] as const) {
		await using checkout = await fs.temp_dir('tsv_pgo_bench_test_');
		const node = fake_node(() => write_pipeline_output(checkout), result);
		const { code, out, err } = await run(checkout, node.program);
		deepStrictEqual([code, out], [1, []]);
		deepStrictEqual(err, [`build:bench:pgo: the PGO pipeline failed (${shown}); nothing staged`]);
		strictEqual(await checkout.exists('.pgo/bench'), false);
	}
});

test('a library the pipeline stamp does not name is not staged, and the old stamp is gone', async () => {
	await using checkout = await fs.temp_dir('tsv_pgo_bench_test_');
	const node = fake_node(async () => {
		await write_pipeline_output(checkout, { napi: sha256('other bytes') });
		await checkout.mkdir('.pgo/bench', { recursive: true });
		await checkout.write('.pgo/bench/stamp.json', '{"version": 1}');
	});
	const { code, out, err } = await run(checkout, node.program);
	deepStrictEqual([code, out], [1, []]);
	deepStrictEqual(err, [
		'build:bench:pgo: the napi library or profile on disk is not what .pgo/stamp.json ' +
			'records the gates passing — nothing staged'
	]);
	strictEqual(await checkout.exists('.pgo/bench/stamp.json'), false);
	strictEqual(await checkout.exists(`.pgo/bench/${pgo_bench_library_name('napi')}`), false);
});

test('a profile the pipeline stamp does not name is not staged', async () => {
	await using checkout = await fs.temp_dir('tsv_pgo_bench_test_');
	const node = fake_node(async () => {
		await write_pipeline_output(checkout);
		await checkout.write('.pgo/ffi.profdata', 'another ffi profile');
	});
	const { code, out, err } = await run(checkout, node.program);
	deepStrictEqual([code, out], [1, []]);
	deepStrictEqual(err, [
		'build:bench:pgo: the ffi library or profile on disk is not what .pgo/stamp.json ' +
			'records the gates passing — nothing staged'
	]);
	strictEqual(await checkout.exists(`.pgo/bench/${pgo_bench_library_name('ffi')}`), false);
	strictEqual(await checkout.exists('.pgo/bench/stamp.json'), false);
});

test('a world the pipeline stamp does not name is not staged', async () => {
	await using checkout = await fs.temp_dir('tsv_pgo_bench_test_');
	const node = fake_node(async () => {
		await write_pipeline_output(checkout);
		const stamp = JSON.parse(await checkout.read_text('.pgo/stamp.json')) as {
			artifacts: Record<string, unknown>;
		};
		delete stamp.artifacts.napi;
		await checkout.write('.pgo/stamp.json', JSON.stringify(stamp));
	});
	const { code, out, err } = await run(checkout, node.program);
	deepStrictEqual([code, out], [1, []]);
	deepStrictEqual(err, [
		'build:bench:pgo: the napi library or profile on disk is not what .pgo/stamp.json ' +
			'records the gates passing — nothing staged'
	]);
	strictEqual(await checkout.exists('.pgo/bench/stamp.json'), false);
});
