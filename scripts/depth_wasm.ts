/**
 * The depth limits' stack contract in the WASM engine: every shape `tests/depth_limit.rs`
 * generates, at its accepted depth, through the `@fuzdev/tsv-wasm` package on a Node MAIN
 * thread — the tightest host the packages run on (`docs/cli.md` §The other surfaces).
 *
 * A WASM build spends two stacks: its spilled frames on a 1 MiB shadow stack in linear
 * memory, and its call frames on the engine's own, about 1 MiB on a V8 main thread. The
 * native gate (`npm run test:depth:napi`) grades neither, so this one runs the package
 * itself: for every shape, at its accepted `n`, the parse (the object form, with and
 * without `{locations: true}`, and the `_json` string), the format, the format's output
 * parsed again and formatted to itself (F1), the import records — TypeScript and Svelte,
 * whose walk descends into what the strip refuses — and — TypeScript — the type strip must
 * each return `{ok: true, …}` (a strip's refusal is a verdict too), never
 * throw a trap or a `RangeError`; and at `n + 1` the parse must return the depth refusal.
 * The engine's own stack is cut to nine tenths of V8's default for the run
 * (`GATE_STACK_KIB`), the margin the native gate keeps; the shadow stack is the shipped
 * build's.
 *
 * **The engine's tier decides the frames**, so every shape runs twice, each pass in fresh
 * processes: once on V8's baseline compiler alone (`--liftoff-only`, a cold process), and
 * once optimized — every function the run makes hot tiered up synchronously
 * (`--wasm-sync-tier-up`) after a warm-up pass over every shape at a shallow depth, so a
 * long-lived process's frames are the ones graded, not a fresh one's. The two can differ
 * by more than the limits' margin in either direction.
 *
 * The shapes come from the native test (`write_shape_limits`, run under the `napi` profile
 * — the build `test:depth:napi` already made, so CI compiles nothing for it): one spec
 * line per shape at its accepted `n`, which this script regenerates sources from, so the
 * WASM leg grades exactly the shapes and depths the native one does. Each pass is sharded
 * over a few processes (every process a main thread of its own); within one, the shapes
 * run in a fixed order, the instance reinstantiated before each (`reinstantiate()`, from
 * the retained compiled module, so tiering carries across shapes), and after any throw.
 *
 * Not in `npm run check`, which builds no package: it runs in CI's `artifacts` job
 * after the package build and the shipped-build depth gate, and in publish's artifact
 * validation (Step 6).
 *
 * Usage: node scripts/depth_wasm.ts [--shapes <file>] [--jobs <n>] [--filter <substring>]
 *          [--pass cold|warm] [--node-flag <flag>]...
 *   (`npm run test:depth:wasm`; PKG_DIR overrides the package, default the staged `all`)
 * Prerequisite: npm run build:npm:all
 */

import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { availableParallelism, tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { assert_staged_fresh, wasm_package_checks } from './check_staged_freshness.ts';
import {
	call_passes,
	is_depth_refusal,
	make_shape,
	read_shapes,
	type ShapeLimit,
	shape_calls,
	write_shape_limits
} from './depth_shapes.ts';

const ROOT = fileURLToPath(new URL('..', import.meta.url));

/**
 * The engine stack every pass runs on, in KiB: nine tenths of V8's default main-thread
 * stack (984 KiB), the tenth the native gate keeps in hand too — so an engine frame that
 * grows fails here while every accepted input still fits a default main thread.
 */
const GATE_STACK_KIB = 885;

/** The two tiers a shape is graded in, each its own processes' node flags. */
const PASSES = {
	cold: ['--liftoff-only', `--stack-size=${GATE_STACK_KIB}`],
	warm: ['--wasm-sync-tier-up', `--stack-size=${GATE_STACK_KIB}`]
} as const;
type Pass = keyof typeof PASSES;

/** A failure a child reports: the shape, the pass, the phase and what happened. */
interface Failure {
	shape: string;
	pass: string;
	phase: string;
	detail: string;
}

/** A child: grade the shapes of one shard in this process, printing failures as JSON lines. */
async function run_child(args: Map<string, string>): Promise<void> {
	const pkg_dir = resolve(ROOT, process.env.PKG_DIR ?? 'crates/tsv_wasm/pkg/all/npm');
	// deno-lint-ignore no-explicit-any
	const tsv: any = await import(pathToFileURL(join(pkg_dir, 'index.js')).href);
	const [shard, shards] = args.get('shard')!.split('/').map(Number) as [number, number];
	const pass = args.get('pass')!;
	const filter = args.get('filter') ?? '';
	const shapes = read_shapes(args.get('shapes')!).filter(
		(shape, i) => i % shards === shard && shape.name.includes(filter)
	);
	const failures: Array<Failure> = [];
	const fail = (shape: ShapeLimit, phase: string, detail: string): void => {
		failures.push({ shape: shape.name, pass, phase, detail });
	};
	if (pass === 'warm') {
		// every call over every shape at a shallow depth, so the functions the run makes hot
		// are tiered up (synchronously, by the pass's flag) before any shape is graded
		for (const shape of shapes) {
			for (const [, call] of shape_calls(tsv, shape, make_shape(shape, Math.min(shape.n, 4))))
				call();
		}
	}
	for (const shape of shapes) {
		tsv.reinstantiate();
		for (const [phase, call] of shape_calls(tsv, shape, make_shape(shape, shape.n))) {
			try {
				// deno-lint-ignore no-explicit-any
				const result: any = call();
				if (!call_passes(phase, result)) {
					fail(shape, phase, `returned ${JSON.stringify(result).slice(0, 300)}`);
				}
			} catch (error) {
				fail(shape, phase, `threw ${String(error).slice(0, 300)}`);
				tsv.reinstantiate();
			}
		}
		try {
			const over = tsv[`parse_${shape.language}_json`](make_shape(shape, shape.n + 1));
			if (over?.ok !== false || !is_depth_refusal(String(over.error?.message))) {
				fail(
					shape,
					'past the limit',
					`n + 1 was not refused by a depth limit: ${JSON.stringify(over).slice(0, 300)}`
				);
			}
		} catch (error) {
			fail(shape, 'past the limit', `threw ${String(error).slice(0, 300)}`);
			tsv.reinstantiate();
		}
	}
	process.stdout.write(JSON.stringify({ graded: shapes.length, failures }) + '\n');
}

/** Run one child to completion, returning its report. */
function run_shard(
	node_flags: ReadonlyArray<string>,
	args: ReadonlyArray<string>
): Promise<{ graded: number; failures: Array<Failure> }> {
	return new Promise((done, failed) => {
		const child = spawn(
			process.execPath,
			[...node_flags, fileURLToPath(import.meta.url), '--child', ...args],
			{ cwd: ROOT, stdio: ['ignore', 'pipe', 'inherit'] }
		);
		let out = '';
		child.stdout.on('data', (chunk) => (out += chunk));
		child.on('error', failed);
		child.on('close', (code, signal) => {
			const last = out.trim().split('\n').at(-1) ?? '';
			if (code === 0 && last.startsWith('{')) done(JSON.parse(last));
			else failed(new Error(`a shard exited ${code ?? signal}: ${out.slice(-500)}`));
		});
	});
}

function parse_args(argv: ReadonlyArray<string>): {
	flags: Map<string, string>;
	node_flags: string[];
} {
	const flags = new Map<string, string>();
	const node_flags: string[] = [];
	for (let i = 0; i < argv.length; i++) {
		const arg = argv[i]!;
		if (arg === '--child') flags.set('child', '1');
		else if (arg === '--node-flag') node_flags.push(argv[++i]!);
		else if (arg.startsWith('--')) flags.set(arg.slice(2), argv[++i]!);
		else throw new Error(`unknown argument ${arg}`);
	}
	return { flags, node_flags };
}

const { flags, node_flags } = parse_args(process.argv.slice(2));
if (flags.has('child')) {
	await run_child(flags);
} else {
	if ('Deno' in globalThis) {
		console.error('✗ run under Node (`npm run test:depth:wasm`), whose main thread is graded');
		process.exit(1);
	}
	if (process.env.PKG_DIR === undefined) await assert_staged_fresh(wasm_package_checks('all'));
	let shapes_path = flags.get('shapes');
	let scratch: string | undefined;
	if (shapes_path === undefined) {
		scratch = mkdtempSync(join(tmpdir(), 'tsv-depth-wasm-'));
		shapes_path = join(scratch, 'shapes.tsv');
		if (!write_shape_limits(shapes_path, { cwd: ROOT, profile: 'napi' })) {
			console.error('✗ write_shape_limits failed');
			process.exit(1);
		}
	}
	const jobs = Number(flags.get('jobs') ?? Math.max(1, Math.min(4, availableParallelism() - 1)));
	const passes: Array<Pass> = flags.has('pass') ? [flags.get('pass') as Pass] : ['cold', 'warm'];
	const failures: Array<Failure> = [];
	let graded = 0;
	try {
		for (const pass of passes) {
			const started = performance.now();
			const reports = await Promise.all(
				Array.from({ length: jobs }, (_, shard) =>
					run_shard(
						[...PASSES[pass], ...node_flags],
						[
							'--shapes',
							shapes_path,
							'--shard',
							`${shard}/${jobs}`,
							'--pass',
							pass,
							...(flags.has('filter') ? ['--filter', flags.get('filter')!] : [])
						]
					)
				)
			);
			const count = reports.reduce((sum, report) => sum + report.graded, 0);
			graded += count;
			for (const report of reports) failures.push(...report.failures);
			const seconds = ((performance.now() - started) / 1000).toFixed(0);
			console.log(`${pass}: ${count} shapes graded in ${seconds} s (${jobs} processes)`);
		}
	} finally {
		if (scratch !== undefined) rmSync(scratch, { recursive: true, force: true });
	}
	if (graded === 0) {
		console.error('✗ no shape was graded');
		process.exit(1);
	}
	if (failures.length > 0) {
		console.error(`\n✗ ${failures.length} failures in the WASM engine at the depth limits:`);
		for (const failure of failures) {
			console.error(`  ${failure.pass} ${failure.phase} — ${failure.shape}: ${failure.detail}`);
		}
		process.exit(1);
	}
	console.log('✓ every shape holds its limit in the WASM engine, cold and warm');
}
