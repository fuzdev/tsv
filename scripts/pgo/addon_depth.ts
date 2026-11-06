/**
 * The depth limits' stack contract in the N-API addon AS STAGED — the profile-guided
 * binary that ships, loaded through the `@fuzdev/tsv` loader — on a thread the contract's
 * size less the tenth the native gate keeps in hand (`GATE_STACK_MB`): every shape
 * `tests/depth_limit.rs` generates (`write_shape_limits`), at its accepted depth, through
 * the parse (object, `{locations: true}`, `_json`), the format, the format's output parsed
 * again and formatted to itself (F1), the import records and the type strip, each of which
 * must return `{ok: true, …}` (a strip's refusal is a verdict too), and at one level deeper
 * the parse must return the depth refusal.
 *
 * Why the staged binary, not a test build: the profile is matched to functions by their
 * symbol names, which carry the crate's build hash, and a test build compiles the language
 * crates with the test graph's features — another hash, so a `-Cprofile-use` test build
 * finds no profile for them at all and lays its frames out as no build that ships does.
 * Only the shipped artifact has the shipped frames.
 *
 * The addon runs on its caller's thread, so the shapes run on `worker_threads` workers
 * whose `resourceLimits.stackSizeMb` sizes that thread (V8's own limit sits below it, at
 * the thread's size less Node's buffer, so a JS frame never reaches the native frames'
 * room). A native overflow is process-fatal, so each pass runs in a child process, whose
 * workers note the shape each starts in a progress file under `--dir` — an overflow names
 * its shape. The run first shows the knob is a live limit where it runs: the CONTROL pass
 * grades every shape (`--filter` narrows only the gate) on a thread far below the contract
 * (`CONTROL_STACK_MB`), which must die — a control that survives, or returns failures
 * instead, means the setting did not shrink the thread below what the deepest shape needs,
 * and the gate would be vacuous. The control bounds the thread from below only; that the
 * gate's thread is no LARGER than asked is the platform's rule, which the run prints:
 * pthread sizes a thread to the request (Linux, macOS), and on Windows libuv asks for the
 * size as the thread's COMMIT, whose reserve is the running node.exe's own
 * (`SizeOfStackReserve` in its PE header) unless the commit is not smaller, then the
 * commit rounded up to a MiB — so there the run reads the reserve of the executable each
 * pass runs on. `pgo.ts depth` runs it under a scratch copy of node.exe whose reserve sits
 * just above the gate's request, and the control pass under a second copy
 * (`--control-node`) whose reserve sits just above the control's, so each thread is within
 * 64 KiB of what its pass asks for.
 * On Windows alone it exits `CANNOT_GRADE_EXIT` (3) with `CANNOT_GRADE_LINE` on stderr,
 * which `pgo.ts depth` reads as "fall back to the plain-profile native gate", in two cases:
 * the gate's thread would land above the contract's size (read before anything runs), or
 * the control survives on a thread the reserve, not its request, sized (the deepest shape
 * fit that reserve, so no worker there can show the setting is a live limit). Every
 * other surviving control fails: off Windows, on a thread its request sized, or returning
 * failures. Then the gate pass shards the shapes over a few workers. Either pass still
 * running at `PASS_TIMEOUT_MS` is killed and fails as hung, never a fallback: a control
 * that overflows dies at its first deep shape and one that survives finishes well inside
 * the bound, so one still running is wedged — an overflow that did not end the process,
 * or a stuck worker — and nothing it shows reads as a fallback reason.
 *
 * Exit codes: 0 graded and held, 1 a failure, 2 usage, 3 cannot grade here (above).
 *
 * Usage: node scripts/pgo/addon_depth.ts --loader <loader index.js> --shapes <file>
 *          --dir <scratch dir> [--jobs <n>] [--filter <substring>] [--control-node <exe>]
 *
 * @module
 */

import { spawnSync } from 'node:child_process';
import {
	closeSync,
	existsSync,
	mkdirSync,
	openSync,
	readFileSync,
	readSync,
	rmSync,
	writeSync
} from 'node:fs';
import { Buffer } from 'node:buffer';
import { availableParallelism } from 'node:os';
import { join, resolve } from 'node:path';
import process from 'node:process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { parseArgs } from 'node:util';
import { isMainThread, parentPort, Worker, workerData } from 'node:worker_threads';

import {
	CANNOT_GRADE_EXIT,
	CANNOT_GRADE_LINE,
	CONTRACT_STACK_MB,
	CONTROL_STACK_MB,
	GATE_STACK_MB,
	MIB,
	PASS_TIMEOUT_MS,
	pe_stack_sizes,
	spawn_timed_out,
	stack_request_bytes,
	timeout_minutes,
	windows_thread_bytes
} from './depth_stack.ts';
import {
	call_passes,
	is_depth_refusal,
	make_shape,
	read_shapes,
	type ShapeLimit,
	shape_calls
} from '../depth_shapes.ts';

/** `bytes` in MiB, for the log. */
const mib = (bytes: number): string => `${(bytes / MIB).toFixed(3).replace(/\.?0+$/, '')} MiB`;

/** Exit `CANNOT_GRADE_EXIT` with `CANNOT_GRADE_LINE` and the reason — the one way this
 * run says it cannot grade here, which `pgo.ts depth` reads on Windows alone. */
function cannot_grade(reason: string): never {
	console.error(`✗ ${CANNOT_GRADE_LINE}: ${reason} (exit ${CANNOT_GRADE_EXIT})`);
	process.exit(CANNOT_GRADE_EXIT);
}

/**
 * The size, in bytes, of the thread a worker asking for `stack_mb` MiB runs on in a process
 * of the executable `exe`, where the platform's rule can be read: on Windows libuv passes
 * the size as the thread's commit, and the reserve — the thread's real size — is the
 * executable's own reserve, or the commit rounded up to a MiB when that is not smaller
 * (`windows_thread_bytes`). Elsewhere (pthread) the thread is the request rounded up to a
 * page, so the request is returned.
 */
function effective_stack_bytes(stack_mb: number, exe: string): { bytes: number; rule: string } {
	const request = stack_request_bytes(stack_mb);
	if (process.platform !== 'win32') {
		return { bytes: request, rule: 'pthread: the thread is the request, page-rounded' };
	}
	const head = Buffer.alloc(64 * 1024);
	const fd = openSync(exe, 'r');
	const read = readSync(fd, head, 0, head.length, 0);
	closeSync(fd);
	const reserve = pe_stack_sizes(head.subarray(0, read))?.reserve;
	if (reserve === undefined) {
		console.error(`✗ could not read the stack reserve from ${exe}'s PE header`);
		process.exit(1);
	}
	const bytes = windows_thread_bytes(request, reserve);
	return {
		bytes,
		rule:
			bytes === reserve
				? `${exe}'s own ${mib(reserve)} reserve, above the commit`
				: `the commit is at least ${exe}'s ${mib(reserve)} reserve, so the reserve is the commit rounded up to a MiB`
	};
}

interface Failure {
	shape: string;
	phase: string;
	detail: string;
}

interface WorkerInput {
	loader: string;
	shapes: string;
	shard: number;
	shards: number;
	filter: string;
	progress: string;
}

/** A worker: grade one shard's shapes on this thread, noting each one before it runs. */
async function run_worker(input: WorkerInput): Promise<void> {
	const tsv = await import(pathToFileURL(input.loader).href);
	const shapes = read_shapes(input.shapes).filter(
		(shape, i) => i % input.shards === input.shard && shape.name.includes(input.filter)
	);
	const progress = openSync(input.progress, 'w');
	const failures: Array<Failure> = [];
	const fail = (shape: ShapeLimit, phase: string, detail: string): void => {
		failures.push({ shape: shape.name, phase, detail });
	};
	for (const shape of shapes) {
		writeSync(progress, `${shape.name}\n`);
		for (const [phase, call] of shape_calls(tsv, shape, make_shape(shape, shape.n))) {
			try {
				const result = call();
				if (!call_passes(phase, result)) {
					fail(shape, phase, `returned ${JSON.stringify(result).slice(0, 300)}`);
				}
			} catch (error) {
				fail(shape, phase, `threw ${String(error).slice(0, 300)}`);
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
		}
	}
	closeSync(progress);
	parentPort!.postMessage({ graded: shapes.length, failures });
}

/** Grade the shapes over `jobs` workers on `stack_mb` threads, in this process. */
function run_workers(
	options: { loader: string; shapes: string; dir: string; filter: string },
	jobs: number,
	stack_mb: number
): Promise<{ graded: number; failures: Array<Failure> }> {
	const self = fileURLToPath(import.meta.url);
	return Promise.all(
		Array.from(
			{ length: jobs },
			(_, shard) =>
				new Promise<{ graded: number; failures: Array<Failure> }>((done, failed) => {
					const input: WorkerInput = {
						loader: options.loader,
						shapes: options.shapes,
						shard,
						shards: jobs,
						filter: options.filter,
						progress: join(options.dir, `progress-${shard}.txt`)
					};
					const worker = new Worker(self, {
						workerData: input,
						resourceLimits: { stackSizeMb: stack_mb }
					});
					let report: { graded: number; failures: Array<Failure> } | undefined;
					worker.on('message', (m) => (report = m));
					worker.on('error', failed);
					worker.on('exit', (code) =>
						report && code === 0 ? done(report) : failed(new Error(`a worker exited ${code}`))
					);
				})
		)
	).then((reports) => ({
		graded: reports.reduce((sum, r) => sum + r.graded, 0),
		failures: reports.flatMap((r) => r.failures)
	}));
}

/** The last shape each progress file under `dir` names — the ones running when a process died. */
const last_started = (dir: string, jobs: number): string[] =>
	Array.from({ length: jobs }, (_, k) => join(dir, `progress-${k}.txt`))
		.filter((p) => existsSync(p))
		.map((p) => readFileSync(p, 'utf8').trimEnd().split('\n').at(-1) ?? '')
		.filter(Boolean);

if (!isMainThread) {
	await run_worker(workerData as WorkerInput);
} else {
	const { values } = parseArgs({
		options: {
			loader: { type: 'string' },
			shapes: { type: 'string' },
			dir: { type: 'string' },
			jobs: { type: 'string' },
			filter: { type: 'string', default: '' },
			'control-node': { type: 'string' },
			'stack-mb': { type: 'string' }
		}
	});
	if (!values.loader || !values.shapes || !values.dir) {
		console.error(
			'usage: node scripts/pgo/addon_depth.ts --loader <index.js> --shapes <file> --dir <dir> ' +
				'[--jobs <n>] [--filter <s>] [--control-node <exe>]'
		);
		process.exit(2);
	}
	const options = {
		loader: resolve(values.loader),
		shapes: resolve(values.shapes),
		dir: resolve(values.dir),
		filter: values.filter!
	};
	// the gate runs on this process's executable, the control on `--control-node` (default
	// the same) — on Windows two node.exe copies whose stack reserves differ
	const gate_node = process.execPath;
	const control_node = values['control-node'] ? resolve(values['control-node']) : gate_node;
	const jobs = Number(values.jobs ?? Math.max(1, Math.min(4, availableParallelism() - 1)));
	if (values['stack-mb'] !== undefined) {
		// a pass: the workers in this process, the report as the last stdout line
		mkdirSync(options.dir, { recursive: true });
		const report = await run_workers(options, jobs, Number(values['stack-mb']));
		process.stdout.write(JSON.stringify(report) + '\n');
		process.exit(0);
	}

	/** Run one pass in a child process of `exe`, killed past `PASS_TIMEOUT_MS`: its report,
	 * or how it died (or that it was killed as hung) and what was running. */
	const pass = (
		name: string,
		exe: string,
		pass_jobs: number,
		stack_mb: number,
		filter: string
	): {
		report?: { graded: number; failures: Array<Failure> };
		died?: string;
		overflowed?: boolean;
		hung?: boolean;
	} => {
		const dir = join(options.dir, name);
		rmSync(dir, { recursive: true, force: true });
		const r = spawnSync(
			exe,
			[
				fileURLToPath(import.meta.url),
				...['--loader', options.loader, '--shapes', options.shapes, '--dir', dir],
				...['--filter', filter, '--jobs', String(pass_jobs)],
				...['--stack-mb', String(stack_mb)]
			],
			{
				encoding: 'utf8',
				stdio: ['ignore', 'pipe', 'pipe'],
				maxBuffer: 1 << 28,
				// SIGKILL: a process wedged in its own fault handling may not answer a SIGTERM
				// (Windows terminates the process whatever the signal)
				timeout: PASS_TIMEOUT_MS,
				killSignal: 'SIGKILL'
			}
		);
		if (spawn_timed_out(r.error)) {
			return {
				hung: true,
				died:
					`killed after ${timeout_minutes(PASS_TIMEOUT_MS)}, running ` +
					`${last_started(dir, pass_jobs).join(', ') || '(none noted)'}` +
					(r.stderr?.trim() ? `\n${r.stderr.trim().slice(-2000)}` : '')
			};
		}
		if (r.error) {
			console.error(`✗ could not run the ${name} pass: ${r.error.message}`);
			process.exit(1);
		}
		const last = r.stdout.trim().split('\n').at(-1) ?? '';
		if (r.status === 0 && last.startsWith('{')) return { report: JSON.parse(last) };
		const running = last_started(dir, pass_jobs);
		return {
			// a native overflow is a signal on Unix and STATUS_STACK_OVERFLOW on Windows; a JS
			// error (a worker that could not start, a thrown setup error) exits 1
			overflowed: running.length > 0 && (r.signal !== null || (r.status !== 0 && r.status !== 1)),
			died:
				`exit ${r.status ?? r.signal}, running ${running.join(', ') || '(none noted)'}` +
				(r.stderr.trim() ? `\n${r.stderr.trim().slice(-2000)}` : '')
		};
	};

	// what thread each pass's setting gives, where the platform's rule can be read
	const gate_stack = effective_stack_bytes(GATE_STACK_MB, gate_node);
	const gate_asked = stack_request_bytes(GATE_STACK_MB);
	const control_stack = effective_stack_bytes(CONTROL_STACK_MB, control_node);
	console.log(
		`stack (${process.platform}): the gate asks ${GATE_STACK_MB} MiB and runs on ${mib(gate_stack.bytes)} ` +
			`(${gate_stack.rule}); the control asks ${CONTROL_STACK_MB} MiB and runs on ` +
			`${mib(control_stack.bytes)} (${control_stack.rule})`
	);
	if (gate_stack.bytes < gate_asked) {
		console.error(
			`✗ the gate's thread is ${mib(gate_stack.bytes)}, under the ${GATE_STACK_MB} MiB asked`
		);
		process.exit(1);
	}
	if (gate_stack.bytes > CONTRACT_STACK_MB * MIB) {
		// only Windows' rule can land here (a reserve above the contract); the caller
		// (`pgo.ts depth`) falls back to the plain-profile native gate on this exit
		cannot_grade(
			`the gate's thread would be ${mib(gate_stack.bytes)}, above the ${CONTRACT_STACK_MB} MiB contract — ` +
				'Node cannot give a worker the thread the gate asks for here'
		);
	}
	if (gate_stack.bytes > gate_asked) {
		console.log(
			`  note: graded at ${mib(gate_stack.bytes)}, ${mib(CONTRACT_STACK_MB * MIB - gate_stack.bytes)} under ` +
				`the contract — less margin than the ${mib(CONTRACT_STACK_MB * MIB - gate_asked)} asked`
		);
	}

	// the control: a thread far below the contract must overflow. It runs EVERY shape —
	// `--filter` narrows only the gate — since a narrowed set may hold no shape that needs
	// more than the control's thread
	if (control_node !== gate_node) console.log(`control: runs under ${control_node}`);
	if (options.filter) console.log(`control: runs every shape (--filter narrows only the gate)`);
	const control = pass('control', control_node, 1, CONTROL_STACK_MB, '');
	if (control.hung) {
		// not a slow control: one that overflows dies at its first deep shape, and one that
		// survives finishes well inside the bound — this one did neither
		console.error(
			`✗ the control pass neither overflowed nor finished on its ${CONTROL_STACK_MB} MiB worker ` +
				`thread (${control.died}) — an overflow that did not end the process, or a wedged ` +
				'worker — so the gate cannot show the setting is a live limit'
		);
		process.exit(1);
	}
	if (control.report) {
		const { graded, failures } = control.report;
		if (
			failures.length === 0 &&
			process.platform === 'win32' &&
			control_stack.bytes > stack_request_bytes(CONTROL_STACK_MB)
		) {
			// node.exe's reserve, not the request, sized the control's thread, and the deepest
			// shape fit it: no worker this node.exe starts can be small enough to prove the
			// setting a live limit, so the gate is unprovable here — not a broken setting
			cannot_grade(
				`the control survived ${graded} shapes on its ${mib(control_stack.bytes)} thread, which ` +
					`${control_node}'s PE stack reserve sized (the control asked ${CONTROL_STACK_MB} MiB), ` +
					'so no worker here is small enough to show the setting is a live limit'
			);
		}
		console.error(
			failures.length > 0
				? `✗ the control pass returned ${failures.length} failures over ${graded} shapes on a ` +
						`${CONTROL_STACK_MB} MiB worker thread instead of overflowing (first: ${failures[0]!.phase} — ` +
						`${failures[0]!.shape}: ${failures[0]!.detail}) — the setting did not overflow the thread, ` +
						'so the gate could not be trusted to'
				: `✗ the control pass survived ${graded} shapes on a ${CONTROL_STACK_MB} MiB worker thread — ` +
						'resourceLimits.stackSizeMb did not shrink the native thread below what the deepest ' +
						'shape needs, so the gate would be vacuous'
		);
		process.exit(1);
	}
	if (!control.overflowed) {
		console.error(`✗ the control pass failed, but not by overflowing its stack: ${control.died}`);
		process.exit(1);
	}
	console.log(
		`control: a ${CONTROL_STACK_MB} MiB worker thread overflowed (${control.died!.split('\n')[0]}), ` +
			'so the setting is a live limit on the native thread'
	);

	// the gate
	const started = performance.now();
	const gate = pass('gate', gate_node, jobs, GATE_STACK_MB, options.filter);
	if (gate.hung) {
		console.error(
			`✗ the gate pass did not finish on its ${GATE_STACK_MB} MiB worker threads within its ` +
				`timeout (${gate.died})`
		);
		process.exit(1);
	}
	if (!gate.report) {
		console.error(`✗ the gate pass died on its ${GATE_STACK_MB} MiB worker threads: ${gate.died}`);
		process.exit(1);
	}
	const report = gate.report;
	const seconds = ((performance.now() - started) / 1000).toFixed(0);
	console.log(
		`gate: ${report.graded} shapes graded on ${mib(gate_stack.bytes)} worker threads in ${seconds} s (${jobs} workers)`
	);
	if (report.graded === 0) {
		console.error('✗ no shape was graded');
		process.exit(1);
	}
	if (report.failures.length > 0) {
		console.error(`\n✗ ${report.failures.length} failures in the addon at the depth limits:`);
		for (const f of report.failures) console.error(`  ${f.phase} — ${f.shape}: ${f.detail}`);
		process.exit(1);
	}
	console.log('✓ every shape holds its limit in the staged addon');
}
