/**
 * The tape route of the parse family: an engine's raw `parse_tape_<lang>` exports, decoded
 * into the object `parse_<lang>` returns — the `engine.parse` table `create_parse_api`
 * (`api_parse.js`) takes. The binary parse wire is specified in `docs/tape_format.md`;
 * the decoder (`tape_decode.js`) is generated from the AST schema table.
 *
 * A raw tape export returns the tape as a `Uint32Array` — padded to a whole word, its exact
 * bytes the header's length (`tape_bytes`) — or the `{ok: false, error}` every parse export
 * returns, which passes through untouched for the facade's `read_failure` to check. The
 * `Uint32Array` is 4-aligned by its type: over the native addon it is a copy the engine
 * owns, over the WASM engine a view of the instance's linear memory, valid only until the
 * next call into the instance — so the tape is decoded here, at once, calling nothing in
 * between.
 *
 * The tape's offsets index the text the wire's do: the caller's source for TypeScript, the
 * source less a leading U+FEFF for Svelte and CSS, whose wires elide it as Svelte's
 * `parse` and `parseCss` do (the rule `locations.js` reads a source by too). A decode that
 * fails is an engine bug, never the caller's source: it throws a plain `Error` naming
 * that, the decoder's own error as its `cause`.
 *
 * Its own module, beside the decoder it imports, so only an entry that publishes the parse
 * family loads either (the packages' `./parse` entries).
 *
 * @module
 */

import { decode_tape } from './tape_decode.js';

/** The tape header's length in bytes (`docs/tape_format.md` §The tape). */
const HEADER_BYTES = 32;

/** U+FEFF, the byte-order mark a Svelte or CSS wire elides at index 0. */
const BOM = 0xfeff;

/**
 * Build the `engine.parse` table over an engine's raw tape exports, keyed by language.
 *
 * @param {Record<string, (source: string, source_type?: string) => unknown>} parse_tape -
 * 	each language's raw `parse_tape_<lang>`
 * @returns {Record<string, (source: string, source_type?: string) => unknown>} each
 * 	language's `(source, source_type?) => {ok: true, ast} | failure`
 */
export function create_tape_parse(parse_tape) {
	/** @type {Record<string, (source: string, source_type?: string) => unknown>} */
	const engine = {};
	for (const [language, raw] of Object.entries(parse_tape)) {
		engine[language] = (source, source_type) => {
			const returned = raw(source, source_type);
			if (!(returned instanceof Uint32Array)) return returned;
			let ast;
			try {
				ast = decode_tape(tape_bytes(returned), indexed_text(source, language));
			} catch (cause) {
				throw new Error('internal error: the engine returned a tape that does not decode', {
					cause
				});
			}
			return { ok: true, ast };
		};
	}
	return engine;
}

/**
 * The tape's exact bytes over the words an engine returned: the header, the body's words
 * and the pool's bytes (header words 3 and 4), the last word's padding left out.
 *
 * @param {Uint32Array} words
 * @returns {Uint8Array}
 * @throws Error when the words are too few for the length their header names
 */
function tape_bytes(words) {
	const length = words.length < 8 ? -1 : HEADER_BYTES + words[3] * 4 + words[4];
	if (length < HEADER_BYTES || length > words.byteLength) {
		throw new Error(`tape: ${words.byteLength} bytes cannot hold the tape its header names`);
	}
	return new Uint8Array(words.buffer, words.byteOffset, length);
}

/**
 * The string a language's wire offsets index (`locations.js`'s rule): `source` for
 * TypeScript, `source` without a leading U+FEFF for Svelte and CSS.
 *
 * @param {string} source
 * @param {string} language
 * @returns {string}
 */
function indexed_text(source, language) {
	if (language !== 'typescript' && source.charCodeAt(0) === BOM) return source.slice(1);
	return source;
}
