/**
 * Tests for panic classification (`is_native_panic_error`).
 *
 * Pins the two directions that matter, since the verdict hard-fails a corpus run:
 *  - every shape a crossed binding boundary can produce classifies (the FFI
 *    `panic:` payload is the live one; the hook text and the WASM trap are the
 *    napi/WASM boundaries), and
 *  - an ordinary rejection does NOT — including one whose source code frame
 *    quotes a corpus file that itself talks about panics, the only way file
 *    content could otherwise fabricate a crash verdict.
 */

import { deepStrictEqual as assertEquals } from 'node:assert';
import { is_native_panic_error } from './panic_errors.ts';

const panics: Array<[string, string]> = [
	['ffi catch_unwind payload', 'panic: byte index 3 is not a char boundary'],
	['ffi unknown payload', 'panic: <unknown>'],
	['rust panic hook line', "thread '<unnamed>' panicked at crates/tsv_ts/src/lexer/mod.rs:42:9"],
	['wasm abort trap', 'RuntimeError: unreachable']
];

for (const [label, message] of panics) {
	Deno.test(`panic: ${label}`, () => {
		assertEquals(is_native_panic_error(message), true);
	});
}

const rejections: Array<[string, string]> = [
	['tsv parse error', 'Unexpected token at 12:3'],
	['prettier syntax error', 'SyntaxError: Unexpected token (3:5)'],
	['empty', ''],
	// The reason the check reads the first line only: a rejection's message can
	// quote the offending source, and corpus files do contain the word.
	[
		'code frame quoting a corpus file about panics',
		'SyntaxError: Unexpected token (2:1)\n  1 | // the formatter panicked at one point\n> 2 | }\n'
	],
	['a file path that merely contains the word', "Cannot read '/repo/src/panic_recovery.ts'"]
];

for (const [label, message] of rejections) {
	Deno.test(`not a panic: ${label}`, () => {
		assertEquals(is_native_panic_error(message), false);
	});
}

Deno.test('absent message is not a panic', () => {
	assertEquals(is_native_panic_error(undefined), false);
	assertEquals(is_native_panic_error(null), false);
});
