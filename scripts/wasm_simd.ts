/**
 * A minimal WebAssembly binary reader for one question: does a bundle use SIMD? The wasm32
 * build's `+simd128` comes from `.cargo/config.toml`'s `target.wasm32-unknown-unknown.rustflags`,
 * which a host `RUSTFLAGS` or `CARGO_ENCODED_RUSTFLAGS` (even set empty) replaces, and a build
 * without it succeeds and ships, only slower. A SIMD build's code section has functions that
 * declare a `v128` local (valtype `0x7b`); a build without `simd128` has none, unless code enables
 * `simd128` per function (`#[target_feature]`), which nothing in the graph does. Most SIMD
 * functions keep their `v128` values on the stack; the census reads the minority that spill to a
 * local, which every SIMD build has had.
 *
 * It reads the module's framing (the header, each section's id and size) and, in the code
 * section, each body's local declarations, skipping every instruction. Anything malformed or
 * truncated throws rather than reading as "no SIMD".
 *
 * Spec: the WebAssembly core binary format (modules, sections, the code section, value types).
 *
 * @module
 */

/** The code section's id. */
const SECTION_CODE = 10;

/** The `v128` value type. */
export const VALTYPE_V128 = 0x7b;

/** The reference-type prefixes (`(ref ht)`, `(ref null ht)`) whose heap type follows as an s33. */
const VALTYPE_REF = 0x64;
const VALTYPE_REF_NULL = 0x63;

/** What `wasm_simd_census` counts in a module's code section. */
export interface WasmSimdCensus {
	/** The function bodies in the code section (imports have none). */
	functions: number;
	/** Those declaring at least one `v128` local. */
	v128_functions: number;
}

/** A forward cursor over the bytes, every read bounds-checked against `end`. */
class Reader {
	pos: number;
	readonly bytes: Uint8Array;
	readonly end: number;

	constructor(bytes: Uint8Array, pos: number, end: number) {
		this.bytes = bytes;
		this.pos = pos;
		this.end = end;
	}

	byte(): number {
		if (this.pos >= this.end) throw Error(`wasm: unexpected end at byte ${this.pos}`);
		return this.bytes[this.pos++]!;
	}

	/** An unsigned LEB128 of at most 32 bits. */
	u32(): number {
		const start = this.pos;
		let result = 0;
		for (let shift = 0; shift < 35; shift += 7) {
			const b = this.byte();
			// the fifth byte carries the top 4 bits alone
			if (shift === 28 && b > 0x0f) throw Error(`wasm: u32 LEB128 overflows at byte ${start}`);
			result += (b & 0x7f) * 2 ** shift;
			if ((b & 0x80) === 0) return result;
		}
		throw Error(`wasm: u32 LEB128 longer than 5 bytes at byte ${start}`);
	}

	/** Skips a signed LEB128 of at most 33 bits (a heap type). */
	skip_s33(): void {
		const start = this.pos;
		for (let i = 0; i < 5; i++) {
			if ((this.byte() & 0x80) === 0) return;
		}
		throw Error(`wasm: s33 LEB128 longer than 5 bytes at byte ${start}`);
	}

	/** Moves to `to`, which must lie within this reader. */
	seek(to: number): void {
		if (to > this.end) throw Error(`wasm: a size runs past its container at byte ${this.pos}`);
		this.pos = to;
	}
}

/**
 * Counts the code section's function bodies and those declaring a `v128` local.
 * @param bytes - a WebAssembly module's binary
 * @returns the census; zero functions when the module has no code section
 * @throws Error - when the bytes are not a well-framed version-1 module
 */
export function wasm_simd_census(bytes: Uint8Array): WasmSimdCensus {
	const r = new Reader(bytes, 0, bytes.length);
	const header = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
	for (const expected of header) {
		if (r.byte() !== expected) throw Error('wasm: not a version-1 WebAssembly module');
	}
	const census: WasmSimdCensus = { functions: 0, v128_functions: 0 };
	let seen_code = false;
	while (r.pos < r.end) {
		const id = r.byte();
		const size = r.u32();
		const section_end = r.pos + size;
		if (section_end > r.end) throw Error(`wasm: section ${id} runs past the end of the module`);
		if (id === SECTION_CODE) {
			if (seen_code) throw Error('wasm: a second code section');
			seen_code = true;
			const section = new Reader(bytes, r.pos, section_end);
			const count = section.u32();
			for (let i = 0; i < count; i++) {
				const body_size = section.u32();
				const body_end = section.pos + body_size;
				if (body_end > section_end) throw Error(`wasm: function body ${i} runs past its section`);
				const body = new Reader(bytes, section.pos, body_end);
				let v128 = false;
				const decls = body.u32();
				for (let d = 0; d < decls; d++) {
					body.u32(); // how many locals this declaration names
					const valtype = body.byte();
					if (valtype === VALTYPE_V128) v128 = true;
					else if (valtype === VALTYPE_REF || valtype === VALTYPE_REF_NULL) body.skip_s33();
				}
				census.functions++;
				if (v128) census.v128_functions++;
				section.seek(body_end);
			}
			if (section.pos !== section_end) throw Error('wasm: the code section has trailing bytes');
		}
		r.seek(section_end);
	}
	return census;
}
