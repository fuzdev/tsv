/**
 * Line splitting over a byte stream — the NDJSON reader the `tsv_debug loc_wires` clients
 * share (`loc_wire_client.ts`, `scripts/check_loc.ts`). Node-modules-free.
 *
 * @module
 */

/**
 * Yield each `\n`-terminated line of a byte stream, decoded, plus a final unterminated line
 * when one is left.
 *
 * Linear in the stream's length: each chunk is scanned once, from where the previous line
 * ended within it, and a line spanning chunks is held as its parts and joined once at its
 * newline. Appending each chunk to one pending string and re-scanning it from its start made
 * a single long reply (a loc wire runs to hundreds of MB) quadratic.
 */
export async function* lines_of(stream: ReadableStream<Uint8Array>): AsyncGenerator<string> {
	let parts: string[] = [];
	for await (const chunk of stream.pipeThrough(new TextDecoderStream())) {
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
	if (parts.length > 0) yield parts.join('');
}
