/**
 * Tests for `TsvDebugServer` over a fake `tsv_debug`: a Node script that plays the protocol,
 * run through gro's spawn as the corpus tools run the real one, so the pipes, the line
 * splitting, and the process's end are the real ones. Each mode is the `<command>` the
 * server sends: an answer per request, an answer holding U+2028 and U+2029 inside a string,
 * a partial line and an exit, an exit before answering, a stdin closed while the process
 * runs on, no answer at all, and an exit code or signal on close. And over an in-memory child,
 * the refusal of one started without a piped stdin or stdout.
 */

import { deepStrictEqual, rejects, strictEqual } from 'node:assert';
import { fileURLToPath } from 'node:url';
import { afterAll, beforeAll, test, vi } from 'vitest';

import { fs, type FsDir } from '@fuzdev/gro/fs.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

import type { ArtifactCheck } from './check_artifact_freshness.ts';
import { TSV_DEBUG_CORPUS, type TsvDebugProgram, TsvDebugServer } from './tsv_debug_server.ts';

/** The checks `start` hands the freshness guard, which passes them all. */
const guarded = vi.hoisted((): ArtifactCheck[][] => []);
vi.mock('./check_artifact_freshness.ts', async (original) => ({
	...(await original<typeof import('./check_artifact_freshness.ts')>()),
	check_artifact_freshness: async (checks: ArtifactCheck[]) => void guarded.push(checks)
}));

/** The fake: `node fake.mjs <mode> --stdin`, one request line read at a time. */
const FAKE = String.raw`
const mode = process.argv[2];
const reply = (value) => process.stdout.write(JSON.stringify(value) + '\n');
// split on LF alone, as tsv_debug does: readline would also break at U+2028 and U+2029
let pending = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', (chunk) => {
	pending += chunk;
	let newline;
	while ((newline = pending.indexOf('\n')) !== -1) {
		const request = JSON.parse(pending.slice(0, newline));
		pending = pending.slice(newline + 1);
		if (mode === 'echo') reply({ echo: request.source });
		else if (mode === 'separators') process.stdout.write('{"text":"a\u2028b\u2029c"}\n');
		else if (mode === 'partial') process.stdout.write('{"text":"cut', () => process.exit(0));
		else if (mode === 'exit') process.exit(0);
	}
});
process.stdin.on('end', () => {
	if (mode === 'code') process.exit(3);
	if (mode === 'signal') process.kill(process.pid, 'SIGKILL');
});
`;

/** A `tsv_debug` that reads one request, closes its stdin, says so on stdout, and runs on,
 * answering nothing more: a request written after that line can't be read. */
const DEAF = String.raw`read -r request; exec 0<&-; printf '{"stdin":"closed"}\n'; exec sleep 60`;

let dir: FsDir & AsyncDisposable;
let program: TsvDebugProgram;
let deaf: TsvDebugProgram;

beforeAll(async () => {
	dir = await fs.temp_dir('tsv_debug_server_test_');
	await dir.write('fake.mjs', FAKE);
	const node = spawn.program('node');
	const fake = `${dir.path}/fake.mjs`;
	program = { start: (args, options) => node.start([fake, ...args], options) };
	const sh = spawn.program('sh');
	deaf = { start: (args, options) => sh.start(['-c', DEAF, 'tsv_debug', ...args], options) };
});
afterAll(async () => {
	await dir[Symbol.asyncDispose]();
});

const server = (mode: string) => TsvDebugServer.spawn<{ source: string }, unknown>(mode, program);

test('each request gets its own reply, and a clean exit closes', async () => {
	const s = await server('echo');
	deepStrictEqual(await s.ask({ source: 'one' }), { echo: 'one' });
	deepStrictEqual(await s.ask({ source: 'two\u2028three' }), { echo: 'two\u2028three' });
	await s.close();
});

test('a U+2028 or U+2029 inside a reply string stays in its line', async () => {
	const s = await server('separators');
	deepStrictEqual(await s.ask({ source: 'x' }), { text: 'a\u2028b\u2029c' });
	deepStrictEqual(await s.ask({ source: 'y' }), { text: 'a\u2028b\u2029c' });
	await s.close();
});

test('a partial line and then an exit is a reply that fails to parse', async () => {
	const s = await server('partial');
	await rejects(s.ask({ source: 'x' }), SyntaxError);
});

test('an exit before answering throws, naming the command', async () => {
	const s = await server('exit');
	await rejects(s.ask({ source: 'x' }), {
		message: 'target/corpus/tsv_debug exit --stdin exited without answering — see its stderr above'
	});
	// a request after the end throws the same, never waiting on a reply
	await rejects(s.ask({ source: 'y' }), /exited without answering/);
});

test('a process that closed its stdin is not waited on', async () => {
	const s = await TsvDebugServer.spawn<{ source: string }, unknown>('deaf', deaf);
	deepStrictEqual(await s.ask({ source: 'x' }), { stdin: 'closed' });
	await rejects(s.ask({ source: 'y' }), /deaf --stdin exited without answering/);
	s.kill();
});

test('a process that never answers leaves the request waiting, until it ends', async () => {
	const s = await server('silent');
	const asked = s.ask({ source: 'x' }).then(
		() => 'answered',
		(error: Error) => error.message
	);
	const waited = await Promise.race([
		asked,
		new Promise<string>((resolve) => setTimeout(() => resolve('waiting'), 300))
	]);
	strictEqual(waited, 'waiting');
	s.kill();
	strictEqual(
		await asked,
		'target/corpus/tsv_debug silent --stdin exited without answering — see its stderr above'
	);
});

test('a non-zero exit on close throws its code', async () => {
	const s = await server('code');
	await rejects(s.close(), { message: 'target/corpus/tsv_debug code --stdin exited 3 on close' });
});

test('a signal on close throws 128 + its number', async () => {
	const s = await server('signal');
	await rejects(s.close(), {
		message: 'target/corpus/tsv_debug signal --stdin exited 137 on close'
	});
});

test('a process started without a piped stdin or stdout is refused, naming the command', async () => {
	const closed = new Promise<never>(() => {});
	const stdin = { write: async () => true, close: async () => {} };
	const stdout = (async function* () {})();
	for (const child of [
		{ stdin: null, stdout, closed, kill: () => true },
		{ stdin, stdout: null, closed, kill: () => true }
	]) {
		const piped: TsvDebugProgram = { start: async () => child };
		await rejects(TsvDebugServer.spawn('echo', piped), {
			message: 'target/corpus/tsv_debug echo --stdin started without piped stdio'
		});
	}
});

test('start dates the binary beneath this checkout, found from the module, whatever the working directory', async () => {
	const started = await TsvDebugServer.start<{ source: string }, unknown>('echo', () => program);
	await started.close();
	deepStrictEqual(
		guarded.map((checks) => checks.map((check) => check.path)),
		[[fileURLToPath(new URL(`../../../${TSV_DEBUG_CORPUS}`, import.meta.url))]]
	);
});
