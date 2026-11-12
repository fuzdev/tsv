/**
 * The release driver's two program calls over fake handles: what a capture keeps and trims, its
 * stdin, a missing program, output cut short, a signal's exit code, and a stream's.
 *
 * @module
 */

import { deepStrictEqual } from 'node:assert';

import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';
import { test } from 'vitest';

import { program_capture, program_stream, to_ended } from './program.ts';

const cwd = { path: '/w' } as unknown as FsReadDir;

const fake = (result: object, calls: Array<unknown>) =>
	({
		out: async (args: ReadonlyArray<string>, options: unknown) => {
			calls.push(['out', args, options]);
			return result;
		},
		run: async (args: ReadonlyArray<string>, options: unknown) => {
			calls.push(['run', args, options]);
			return result;
		}
	}) as unknown as SpawnProgram;

test('a capture trims stdout at its end only and stderr at both, stdin /dev/null unless asked', async () => {
	const calls: Array<unknown> = [];
	const program = fake(
		{ kind: 'exited', code: 0, stdout: ' M a\n\n', stderr: '\n note \n', truncated: false },
		calls
	);
	deepStrictEqual(await program_capture(program, 'git', ['status'], { cwd }), {
		success: true,
		out: ' M a',
		err: 'note'
	});
	await program_capture(program, 'git', ['fetch'], { cwd, stdin: 'inherit' });
	deepStrictEqual(calls, [
		['out', ['status'], { cwd }],
		['out', ['fetch'], { cwd, stdin: 'inherit' }]
	]);
});

test('a non-zero exit, a signal, and output cut short each fail a capture', async () => {
	for (const result of [
		{ kind: 'exited', code: 1, stdout: 'x', stderr: '', truncated: false },
		{ kind: 'signaled', signal: 'SIGKILL', stdout: '', stderr: '', truncated: false },
		{ kind: 'exited', code: 0, stdout: 'partial', stderr: '', truncated: true }
	]) {
		deepStrictEqual((await program_capture(fake(result, []), 'npm', [], { cwd })).success, false);
	}
});

test('a missing program is a failed capture naming it, and a stream ending 127 that says so', async () => {
	deepStrictEqual(await program_capture(null, 'npm', ['whoami'], { cwd }), {
		success: false,
		out: '',
		err: 'npm: command not found'
	});
	const printed: Array<string> = [];
	deepStrictEqual(
		await program_stream(null, 'deno', ['task', 'check'], { cwd }, (line) => printed.push(line)),
		{ success: false, code: 127 }
	);
	deepStrictEqual(printed, ['deno: command not found']);
});

test('a stream runs with the terminal and returns its exit code', async () => {
	const calls: Array<unknown> = [];
	deepStrictEqual(
		await program_stream(
			fake({ kind: 'exited', code: 3 }, calls),
			'deno',
			['x'],
			{ cwd },
			() => {}
		),
		{ success: false, code: 3 }
	);
	deepStrictEqual(calls, [['run', ['x'], { cwd }]]);
});

test('an exit code for each way a call ends', () => {
	deepStrictEqual(to_ended({ kind: 'exited', code: 0 }), { success: true, code: 0 });
	deepStrictEqual(to_ended({ kind: 'signaled', signal: 'SIGTERM' }), { success: false, code: 143 });
	deepStrictEqual(to_ended({ kind: 'timed_out', code: 4, signal: null }), {
		success: false,
		code: 4
	});
	deepStrictEqual(to_ended({ kind: 'aborted', code: null, signal: 'SIGKILL' }), {
		success: false,
		code: 137
	});
	deepStrictEqual(to_ended({ kind: 'aborted', code: null, signal: null }), {
		success: false,
		code: 1
	});
});
