/**
 * Tests for `lines_of`: lines split at every chunk boundary the stream can present —
 * a newline at a chunk's start or end, a line spanning several chunks, a multi-byte
 * character split across two chunks — and a final unterminated line kept.
 */

import { deepStrictEqual } from 'node:assert';
import { lines_of } from './text_lines.ts';

const encoder = new TextEncoder();

function stream_of(chunks: Uint8Array[]): ReadableStream<Uint8Array> {
	return new ReadableStream({
		start(controller) {
			for (const chunk of chunks) controller.enqueue(chunk);
			controller.close();
		}
	});
}

async function collect(chunks: Uint8Array[]): Promise<string[]> {
	const lines: string[] = [];
	for await (const line of lines_of(stream_of(chunks))) lines.push(line);
	return lines;
}

/** Every split of `text` into two chunks yields the same lines as the whole. */
async function assert_split_invariant(text: string, expected: string[]): Promise<void> {
	const bytes = encoder.encode(text);
	deepStrictEqual(await collect([bytes]), expected, 'one chunk');
	for (let at = 0; at <= bytes.length; at++) {
		deepStrictEqual(
			await collect([bytes.slice(0, at), bytes.slice(at)]),
			expected,
			`split at byte ${at}`
		);
	}
	deepStrictEqual(
		await collect([...bytes].map((byte) => Uint8Array.of(byte))),
		expected,
		'one byte per chunk'
	);
}

Deno.test('lines_of: terminated lines, empty ones included', async () => {
	await assert_split_invariant('a\n\nbc\n', ['a', '', 'bc']);
});

Deno.test('lines_of: a final unterminated line is kept', async () => {
	await assert_split_invariant('a\nbc', ['a', 'bc']);
});

Deno.test('lines_of: multi-byte characters split across chunks', async () => {
	await assert_split_invariant('é\u{1F600}x\n\u{1F600}', ['é\u{1F600}x', '\u{1F600}']);
});

Deno.test('lines_of: an empty stream yields nothing', async () => {
	deepStrictEqual(await collect([]), []);
	deepStrictEqual(await collect([encoder.encode('')]), []);
});
