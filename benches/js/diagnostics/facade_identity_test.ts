/**
 * `grade_identity`, `facade_probe.ts`'s byte-identity gate, over fake arms: agreeing arms keep
 * every file the row accepts, a different product or a one-sided throw is a mismatch, a file the
 * row rejects leaves the timed set, and a crash in the row or in any other arm is listed rather
 * than dropped. Package-free, so it gates in `npm run test:node`.
 */

import { deepStrictEqual, strictEqual } from 'node:assert';
import { describe, test } from 'vitest';

import { TsvRejection } from '../lib/divergence/panic_errors.ts';
import type { SourceFile } from '../lib/types.ts';
import { raise_wasm_trap } from '../lib/wasm_trap_fakes.ts';
import { type Call, grade_identity, type IdentityCalls, product_text } from './facade_identity.ts';

const FILES = ['a.css', 'b.css', 'c.css'].map(
	(path) => ({ path, content: path, language: 'css', bytes: path.length }) as SourceFile
);

/** A call that returns its source, except where `on` names it. */
const call =
	(on: Record<string, () => unknown> = {}): Call =>
	(source) =>
		source in on ? on[source]!() : source;

const panic = (): never => {
	throw new Error('panic: boom\n  at a frame');
};
const reject = (): never => {
	throw new TsvRejection('panic: a rejection reading as one\n  at 1:1');
};

const cells = (row: Call, others: Record<string, Call> = { engine: call() }): IdentityCalls[] => [
	{
		operation: 'format',
		row,
		others: Object.entries(others).map(([arm, c]) => ({ arm, call: c }))
	}
];

const kept = (files: SourceFile[]): string[] => files.map((f) => f.path);

describe('grade_identity', () => {
	test('agreeing arms keep every file, with nothing to report', () => {
		const grade = grade_identity(
			'napi/css',
			cells(call(), { engine: call(), package: call() }),
			FILES
		);
		deepStrictEqual(kept(grade.kept), ['a.css', 'b.css', 'c.css']);
		deepStrictEqual([grade.mismatches, grade.ungraded, grade.crashes], [0, 0, []]);
	});

	test('a rejection in the row drops its file, a `panic: ` text included', () => {
		const grade = grade_identity('napi/css', cells(call({ 'b.css': reject })), FILES);
		deepStrictEqual(kept(grade.kept), ['a.css', 'c.css']);
		deepStrictEqual([grade.mismatches, grade.crashes], [0, []]);
	});

	test('a crash in the row is listed, on the first input or the last, and not kept', () => {
		const grade = grade_identity(
			'wasm/css',
			cells(call({ 'a.css': () => raise_wasm_trap(), 'c.css': panic })),
			FILES
		);
		deepStrictEqual(kept(grade.kept), ['b.css']);
		deepStrictEqual(grade.crashes, [
			{ label: 'wasm/css format row: a.css', error: 'RuntimeError: unreachable' },
			{ label: 'wasm/css format row: c.css', error: 'panic: boom' }
		]);
		strictEqual(grade.mismatches, 0);
	});

	test('a crash in another arm, where the row accepted, is listed', () => {
		const grade = grade_identity(
			'wasm/css',
			cells(call(), {
				engine: call(),
				package: call({ 'b.css': () => raise_wasm_trap('divide') })
			}),
			FILES
		);
		deepStrictEqual(grade.crashes, [
			{ label: 'wasm/css format package: b.css', error: 'RuntimeError: divide by zero' }
		]);
		strictEqual(grade.mismatches, 0);
	});

	test('a different product, or a throw where the row accepted, is a mismatch', () => {
		const grade = grade_identity(
			'napi/css',
			cells(call(), { engine: call({ 'a.css': () => 'other', 'c.css': reject }) }),
			FILES
		);
		strictEqual(grade.mismatches, 2);
		deepStrictEqual(grade.crashes, []);
	});

	test("a row's crash on one operation skips the file's later ones", () => {
		const later: string[] = [];
		const grade = grade_identity(
			'napi/css',
			[
				{ operation: 'parse', row: call({ 'a.css': panic }), others: [] },
				{
					operation: 'format',
					row: (source) => {
						later.push(source);
						return source;
					},
					others: []
				}
			],
			FILES
		);
		deepStrictEqual(later, ['b.css', 'c.css']);
		deepStrictEqual(
			grade.crashes.map((c) => c.label),
			['napi/css parse row: a.css']
		);
	});

	test('a product the serializer cannot render is ungraded, and its file still kept', () => {
		const cyclic: Record<string, unknown> = {};
		cyclic.self = cyclic;
		strictEqual(product_text(cyclic), null);
		const grade = grade_identity('napi/css', cells(call({ 'a.css': () => cyclic })), FILES);
		deepStrictEqual([grade.ungraded, grade.mismatches, kept(grade.kept).length], [1, 0, 3]);
	});
});
