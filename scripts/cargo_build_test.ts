/**
 * Which artifacts `cargo_build.ts` stamps: a leaf target's final file, and never one
 * cargo dates a dependent against, and only beneath the target directory; and the cargo run
 * around it, over a fake cargo.
 */

import { deepStrictEqual, rejects } from 'node:assert';
import { test } from 'vitest';

import {
	type CargoBuildDeps,
	type CargoExit,
	run_cargo_build,
	stamp_paths,
	to_exit_code,
	to_stamp_path,
	type StampTimes
} from './cargo_build.ts';

const artifact = (kind: string[], filenames: string[], executable: string | null = null) => ({
	reason: 'compiler-artifact',
	target: { kind },
	filenames,
	executable,
	fresh: true
});

test('a bin stamps its executable', () => {
	deepStrictEqual(stamp_paths(artifact(['bin'], ['/t/corpus/tsv'], '/t/corpus/tsv')), [
		'/t/corpus/tsv'
	]);
	deepStrictEqual(
		stamp_paths(
			artifact(['bin'], ['/t/release/tsv.exe', '/t/release/tsv.pdb'], '/t/release/tsv.exe')
		),
		['/t/release/tsv.exe']
	);
});

test('a cdylib stamps its dynamic library, not the files beside it', () => {
	deepStrictEqual(stamp_paths(artifact(['cdylib'], ['/t/corpus/libtsv_ffi.so'])), [
		'/t/corpus/libtsv_ffi.so'
	]);
	deepStrictEqual(stamp_paths(artifact(['cdylib'], ['/t/napi/libtsv_napi.dylib'])), [
		'/t/napi/libtsv_napi.dylib'
	]);
	deepStrictEqual(
		stamp_paths(
			artifact(
				['cdylib'],
				['/t/release/tsv_ffi.dll', '/t/release/tsv_ffi.dll.lib', '/t/release/tsv_ffi.pdb']
			)
		),
		['/t/release/tsv_ffi.dll']
	);
});

test('a target with dependents is never stamped', () => {
	deepStrictEqual(
		stamp_paths(
			artifact(['lib'], ['/t/corpus/libtsv_cli.rlib', '/t/corpus/deps/libtsv_cli-0.rmeta'])
		),
		[]
	);
	// one unit, two outputs: dating the library forward dates the rlib's unit forward
	deepStrictEqual(
		stamp_paths(
			artifact(['cdylib', 'rlib'], ['/t/release/libtsv_wasm.so', '/t/release/libtsv_wasm.rlib'])
		),
		[]
	);
	deepStrictEqual(
		stamp_paths(artifact(['proc-macro'], ['/t/corpus/deps/libnapi_derive-0.so'])),
		[]
	);
	deepStrictEqual(
		stamp_paths(artifact(['custom-build'], ['/t/corpus/build/tsv_html-0/build-script-build'])),
		[]
	);
});

test('a message that is not an artifact stamps nothing', () => {
	deepStrictEqual(stamp_paths({ reason: 'build-finished', success: true }), []);
	deepStrictEqual(
		stamp_paths({ reason: 'build-script-executed', out_dir: '/t/corpus/build/x/out' }),
		[]
	);
	deepStrictEqual(stamp_paths(null), []);
	deepStrictEqual(stamp_paths('compiler-artifact'), []);
});

test('a stamp path is relative to the stamped directory, and only beneath it', () => {
	deepStrictEqual(to_stamp_path('/r/target', '/r/target/corpus/tsv'), 'corpus/tsv');
	deepStrictEqual(
		to_stamp_path('/r/target', '/r/target/ffi-format/release/libtsv_ffi.so'),
		'ffi-format/release/libtsv_ffi.so'
	);
	deepStrictEqual(to_stamp_path('/r/target', '/r/target'), null);
	deepStrictEqual(to_stamp_path('/r/target', '/r/elsewhere/tsv'), null);
	deepStrictEqual(to_stamp_path('/r/target', '/r/target-other/tsv'), null);
	deepStrictEqual(to_stamp_path('/r/target', '/tmp/cargo/release/tsv'), null);
	// a name that merely starts with dots is beneath it
	deepStrictEqual(to_stamp_path('/r/target', '/r/target/..cache/tsv'), '..cache/tsv');
});

/** A cargo whose stdout is `lines` and whose run ends as `result`, recording its calls. */
const fake_cargo = (lines: string[], result: CargoExit) => {
	const calls: Array<{ args: ReadonlyArray<string>; cwd: string }> = [];
	const start_cargo: CargoBuildDeps['start_cargo'] = (args, cwd) => {
		calls.push({ args, cwd });
		return {
			stdout: (async function* () {
				for (const line of lines) yield new TextEncoder().encode(`${line}\n`);
			})(),
			closed: Promise.resolve(result)
		};
	};
	return { start_cargo, calls };
};

const checkout = '/r';

const run = async (lines: string[], result: CargoExit, args: string[] = []) => {
	const { start_cargo, calls } = fake_cargo(lines, result);
	const stamped: Array<{ path: string; times: StampTimes }> = [];
	const printed: string[] = [];
	const code = await run_cargo_build(
		{
			start_cargo,
			checkout,
			target: {
				path: '/r/target',
				set_times: async (path, times) => {
					stamped.push({ path, times });
				}
			},
			print: (line) => printed.push(line)
		},
		args
	);
	return { code, calls, stamped, printed };
};

const bin = (path: string) => JSON.stringify(artifact(['bin'], [path], path)).replaceAll('\n', '');

test('cargo builds in the checkout with the JSON message format and the caller args after it', async () => {
	const { code, calls } = await run([], { kind: 'exited', code: 0 }, [
		'-p',
		'tsv_ffi',
		'--release'
	]);
	deepStrictEqual(code, 0);
	deepStrictEqual(calls, [
		{
			args: ['build', '--message-format=json-render-diagnostics', '-p', 'tsv_ffi', '--release'],
			cwd: '/r'
		}
	]);
});

test('a built leaf artifact is stamped beneath the target, its two times one now', async () => {
	const { code, stamped } = await run([bin('/r/target/corpus/tsv'), bin('/r/target/corpus/tsv')], {
		kind: 'exited',
		code: 0
	});
	deepStrictEqual(code, 0);
	deepStrictEqual(
		stamped.map((s) => s.path),
		['corpus/tsv']
	);
	const { mtime_ms, atime_ms } = stamped[0]!.times;
	deepStrictEqual(atime_ms, mtime_ms);
	deepStrictEqual(typeof mtime_ms, 'number');
});

test("a line that isn't a cargo message is passed through", async () => {
	const { printed, stamped } = await run(['warning: plain text', bin('/r/target/corpus/tsv')], {
		kind: 'exited',
		code: 0
	});
	deepStrictEqual(printed, ['warning: plain text']);
	deepStrictEqual(stamped.length, 1);
});

test("a failed build stamps nothing and exits with cargo's code, 128 + N for signal N", async () => {
	const failed = await run([bin('/r/target/corpus/tsv')], { kind: 'exited', code: 101 });
	deepStrictEqual([failed.code, failed.stamped.length], [101, 0]);
	const interrupted = await run([bin('/r/target/corpus/tsv')], {
		kind: 'signaled',
		signal: 'SIGINT'
	});
	deepStrictEqual([interrupted.code, interrupted.stamped.length], [130, 0]);
	const terminated = await run([bin('/r/target/corpus/tsv')], {
		kind: 'signaled',
		signal: 'SIGTERM'
	});
	deepStrictEqual([terminated.code, terminated.stamped.length], [143, 0]);
});

test('an exit code is the code cargo exited with, or 128 + the signal number', () => {
	deepStrictEqual(to_exit_code({ kind: 'exited', code: 0 }), 0);
	deepStrictEqual(to_exit_code({ kind: 'exited', code: 101 }), 101);
	deepStrictEqual(to_exit_code({ kind: 'signaled', signal: 'SIGKILL' }), 137);
	deepStrictEqual(to_exit_code({ kind: 'signaled', signal: 'SIGNOTHING' }), 1);
});

test('a cargo that could not start rejects the run, and nothing is stamped', async () => {
	const stamped: string[] = [];
	await rejects(
		run_cargo_build(
			{
				start_cargo: () => {
					const closed = Promise.reject(new Error('spawn cargo ENOENT'));
					closed.catch(() => {});
					return { stdout: (async function* () {})(), closed };
				},
				checkout,
				target: {
					path: '/r/target',
					set_times: async (path) => {
						stamped.push(path);
					}
				},
				print: () => {}
			},
			[]
		),
		/spawn cargo ENOENT/
	);
	deepStrictEqual(stamped, []);
});

test('an artifact outside the target is refused, and nothing is stamped', async () => {
	const { start_cargo } = fake_cargo([bin('/r/target/corpus/tsv'), bin('/elsewhere/release/tsv')], {
		kind: 'exited',
		code: 0
	});
	const stamped: string[] = [];
	await rejects(
		run_cargo_build(
			{
				start_cargo,
				checkout,
				target: {
					path: '/r/target',
					set_times: async (path) => {
						stamped.push(path);
					}
				},
				print: () => {}
			},
			[]
		),
		/refusing to stamp \/elsewhere\/release\/tsv: it is not beneath \/r\/target/
	);
	deepStrictEqual(stamped, []);
});
