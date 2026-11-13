/**
 * Tests for crash classification (`is_tsv_crash`), its FAIL entry text (`crash_text`), and
 * the FAIL block (`panic_report_lines`).
 *
 * Pins the two directions that matter, since the verdict hard-fails a run:
 *  - every crash a tsv engine can throw classifies, read off the thrown value's TYPE: a
 *    caught native panic (the `Error` whose message is `panic: …`, the N-API probe's message
 *    being the one `scripts/test_napi.ts` holds the built addon to), and a WASM trap, a
 *    real `WebAssembly.RuntimeError` raised by a hand-assembled module
 *    (`wasm_trap_fakes.ts`), whose message (`unreachable`) says nothing a rejection's text
 *    can't, and
 *  - an ordinary rejection does NOT, whatever its text: a failure the engine RETURNED is a
 *    `TsvRejection`, thrown so by the binding wrapper (driven here over fake engines), even
 *    when its message is exactly `unreachable` or starts `panic: `; and no message alone,
 *    a code frame quoting a corpus file included, can fabricate a crash.
 */

import { deepStrictEqual as assertEquals, ok } from 'node:assert';
import { test } from 'vitest';
import { TsvBinding, type TsvEngine } from '../tsv_api.ts';
import type { Language } from '../types.ts';
import { raise_wasm_trap } from '../wasm_trap_fakes.ts';
import {
	crash_text,
	is_tsv_crash,
	NAPI_PANIC_PROBE_MESSAGE,
	panic_report_lines,
	TsvRejection
} from './panic_errors.ts';

/** What `run` threw. */
const thrown_by = (run: () => unknown): unknown => {
	try {
		run();
	} catch (e) {
		return e;
	}
	throw new Error('expected a throw');
};

test('a WASM trap is a crash, by its type: each trap message, through any caller', () => {
	const unreachable = thrown_by(() => raise_wasm_trap('unreachable'));
	const divide = thrown_by(() => raise_wasm_trap('divide'));
	// the trap's message is the bare instruction's, which is why a message can't classify it
	ok(unreachable instanceof WebAssembly.RuntimeError);
	assertEquals((unreachable as Error).message, 'unreachable');
	ok(divide instanceof WebAssembly.RuntimeError);
	assertEquals((divide as Error).message, 'divide by zero');
	assertEquals(is_tsv_crash(unreachable), true);
	assertEquals(is_tsv_crash(divide), true);
	assertEquals(crash_text(unreachable), 'RuntimeError: unreachable');
	assertEquals(crash_text(divide), 'RuntimeError: divide by zero');
});

const native_panics: Array<[string, string]> = [
	['the payload both native bindings report', 'panic: byte index 3 is not a char boundary'],
	['an unknown payload', 'panic: <unknown>'],
	['the napi probe message', NAPI_PANIC_PROBE_MESSAGE],
	['a payload over lines', 'panic: x\n  at a frame']
];

for (const [label, message] of native_panics) {
	test(`a thrown native panic is a crash: ${label}`, () => {
		assertEquals(is_tsv_crash(new Error(message)), true);
		assertEquals(crash_text(new Error(message)), message.split('\n')[0]);
	});
}

const not_crashes: Array<[string, unknown]> = [
	['a returned failure whose message is exactly `unreachable`', new TsvRejection('unreachable')],
	['a returned failure starting `panic: `', new TsvRejection('panic: x\n> 1 | panic: x')],
	['a plain error reading `unreachable`', new Error('unreachable')],
	["a trap's string form, as text", new Error('RuntimeError: unreachable')],
	['an argument refusal', new TypeError('panic: source must be a string')],
	['the host stack exhausted', new RangeError('Maximum call stack size exceeded')],
	['a tsv parse error', new Error('Unexpected token at 12:3')],
	['a prettier syntax error', new Error('SyntaxError: Unexpected token (3:5)')],
	// a rejection's message can quote the offending source, and corpus files hold the word
	[
		'a code frame quoting a corpus file about panics',
		new Error('SyntaxError: Unexpected token (2:1)\n> 2 | panic: at one point\n')
	],
	['the hook text Rust prints', new Error("thread '<unnamed>' panicked at src/lib.rs:42:9")],
	['a string, though it reads as a panic', 'panic: x'],
	['an error-shaped object', { name: 'RuntimeError', message: 'unreachable' }],
	['undefined', undefined],
	['null', null]
];

for (const [label, thrown] of not_crashes) {
	test(`not a crash: ${label}`, () => {
		assertEquals(is_tsv_crash(thrown), false);
	});
}

/**
 * `TsvBinding` over a fake engine whose every export does what `export_fn` does, for the
 * rows' calls exactly as the N-API and WASM wrappers make them, facade included.
 */
class FakeBinding extends TsvBinding {
	constructor(export_fn: (source: string) => unknown) {
		super();
		const each = <T>() => ({ svelte: export_fn, typescript: export_fn, css: export_fn }) as T;
		this.bind({
			parse_json: each<TsvEngine['parse_json']>(),
			format: each<TsvEngine['format']>(),
			parse_internal: each<TsvEngine['parse_internal']>()
		});
	}
	async init(): Promise<void> {}
}

/** The failure an engine returns in its product's place (`EngineFailure`), with `message`. */
const returned_failure = (message: string) => ({
	ok: false,
	error: {
		kind: 'parse',
		message,
		frame: `> 1 | ${message}`,
		start: 0,
		end: 1,
		loc: { start: { line: 1, column: 0 }, end: { line: 1, column: 1 } }
	}
});

/** What each of a binding's row calls threw, by call. */
const thrown_by_rows = (binding: TsvBinding, language: Language): unknown[] => [
	thrown_by(() => binding.parse('x', language)),
	thrown_by(() => binding.parse_with_locations('x', language)),
	thrown_by(() => binding.parse_internal('x', language)),
	thrown_by(() => binding.format('x', language))
];

test('a failure the engine RETURNED is a rejection on every row, whatever its text says', () => {
	for (const message of ['unreachable', 'panic: x', 'RuntimeError: unreachable']) {
		const binding = new FakeBinding(() => returned_failure(message));
		for (const thrown of thrown_by_rows(binding, 'css')) {
			ok(thrown instanceof TsvRejection);
			assertEquals((thrown as Error).message.split('\n')[0], message);
			assertEquals(is_tsv_crash(thrown), false);
		}
	}
});

test('what the engine THREW reaches the catch as thrown, and a crash classifies on every row', () => {
	const panicking = new FakeBinding(() => {
		throw new Error('panic: boom');
	});
	const trapping = new FakeBinding(() => raise_wasm_trap());
	for (const thrown of [
		...thrown_by_rows(panicking, 'typescript'),
		...thrown_by_rows(trapping, 'typescript')
	]) {
		assertEquals(is_tsv_crash(thrown), true);
	}
});

test('the FAIL block lists the first ten crashes, each by its label, then counts the rest', () => {
	const panics = Array.from({ length: 11 }, (_, i) => ({
		label: `tsv: f${i}.css`,
		error: 'panic: x'
	}));
	const lines = panic_report_lines(panics);
	assertEquals(lines.length, 12);
	assertEquals(
		lines[0],
		'FAIL: 11 panic(s) — tsv crashed on real code (a native panic the corpus profile caught, which a shipped artifact aborts the host on, or a WASM trap):'
	);
	assertEquals(lines[1], '  tsv: f0.css: panic: x');
	assertEquals(lines[10], '  tsv: f9.css: panic: x');
	assertEquals(lines[11], '  ... and 1 more');
	assertEquals(panic_report_lines(panics.slice(0, 10)).length, 11);
});
