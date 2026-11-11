/**
 * Line splitting over a byte stream — the NDJSON reader the `tsv_debug` clients share
 * (`tsv_debug_server.ts`, `scripts/check_loc.ts`). Node-modules-free.
 *
 * @module
 */

/**
 * Yield each `\n`-terminated line of a byte stream, decoded, plus a final unterminated line
 * when one is left. The stream is any async iterable of byte chunks: a web `ReadableStream`
 * (what Deno's child processes pipe) or a gro `SpawnChild`'s `stdout`.
 *
 * Linear in the stream's length: each chunk is scanned once, from where the previous line
 * ended within it, and a line spanning chunks is held as its parts and joined once at its
 * newline. Appending each chunk to one pending string and re-scanning it from its start made
 * a single long reply (a loc wire runs to hundreds of MB) quadratic.
 */
export async function* lines_of(stream: AsyncIterable<Uint8Array>): AsyncGenerator<string> {
	let parts: string[] = [];
	const decoder = new TextDecoder();
	// one pass per decoded chunk, the decoder's held bytes of a split character flushed last
	function* split(chunk: string): Generator<string> {
		let from = 0;
		let newline = chunk.indexOf('\n');
		while (newline !== -1) {
			parts.push(chunk.slice(from, newline));
			yield parts.length === 1 ? parts[0]! : parts.join('');
			parts = [];
			from = newline + 1;
			newline = chunk.indexOf('\n', from);
		}
		if (from < chunk.length) parts.push(from === 0 ? chunk : chunk.slice(from));
	}
	for await (const bytes of stream) yield* split(decoder.decode(bytes, { stream: true }));
	yield* split(decoder.decode());
	if (parts.length > 0) yield parts.join('');
}
