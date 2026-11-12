/**
 * Tests for the `conformance` driver (`conformance.ts`) over fake legs: the legs' order, the
 * first failing leg's exit code ending the run, a thrown leg failing it with 1, and the
 * refusal of arguments. The legs themselves are their tools' suites'. Package-free, so it
 * gates in `npm run test:node`.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { afterEach, test, vi } from 'vitest';

import {
	type ConformanceDeps,
	type ConformanceLeg,
	conformance_legs,
	run_conformance,
	run_conformance_legs
} from './conformance.ts';

afterEach(() => {
	vi.restoreAllMocks();
});

const capture_console = (): string[] => {
	const lines: string[] = [];
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void lines.push(a.join(' ')));
	return lines;
};

/** Deps no leg may reach: these tests run legs of their own. */
const unreachable = new Proxy({} as ConformanceDeps, {
	get: () => {
		throw new Error('a leg reached its deps');
	}
});

test('the legs run in the aggregate order', () => {
	deepStrictEqual(
		conformance_legs(unreachable).map(([name]) => name),
		[
			'conformance:svelte-fixtures',
			'conformance:ts-fixtures',
			'conformance:ts-repo',
			'corpus:compare:parse --all',
			'corpus:compare:parse tests/fixtures --fixtures',
			'corpus:compare:format --all',
			'render:audit (pinned checkouts)'
		]
	);
});

test('every leg green is a green run', async () => {
	const lines = capture_console();
	const ran: string[] = [];
	const leg = (name: string): ConformanceLeg => [name, async () => (ran.push(name), 0)];
	strictEqual(await run_conformance_legs([leg('a'), leg('b')]), 0);
	deepStrictEqual(ran, ['a', 'b']);
	ok(lines.at(-1)!.includes('all 2 legs green'));
});

test("the first failing leg's code ends the run, and no later leg runs", async () => {
	const lines = capture_console();
	const ran: string[] = [];
	const code = await run_conformance_legs([
		['a', async () => (ran.push('a'), 0)],
		['b', async () => (ran.push('b'), 3)],
		['c', async () => (ran.push('c'), 0)]
	]);
	strictEqual(code, 3);
	deepStrictEqual(ran, ['a', 'b']);
	ok(!lines.some((l) => l.includes('b OK')));
	ok(!lines.some((l) => l.includes('legs green')));
});

test('a leg failing with 1, as every gate does, ends the run with 1', async () => {
	capture_console();
	const ran: string[] = [];
	const code = await run_conformance_legs([
		['a', async () => (ran.push('a'), 1)],
		['b', async () => (ran.push('b'), 0)]
	]);
	strictEqual(code, 1);
	deepStrictEqual(ran, ['a']);
});

test('a leg that throws fails the run with 1 and names the leg', async () => {
	const lines = capture_console();
	const code = await run_conformance_legs([
		['a', () => Promise.reject(new Error('loader down'))],
		['b', async () => 0]
	]);
	strictEqual(code, 1);
	ok(lines.includes('\nFAIL: a errored: loader down'));
	ok(!lines.some((l) => l.includes('════ b')));
});

test('the driver takes no arguments', async () => {
	const lines = capture_console();
	strictEqual(await run_conformance(unreachable, ['extra']), 1);
	ok(lines[0]!.startsWith('The conformance driver takes no arguments'));
});
