/**
 * The tape's generated code, from the tape schema (`tape_schema.ts`): the JS decoder the npm
 * facade ships (`crates/tsv_wasm/npm/tape_decode.js`) and the Rust tables the transcoder and
 * `tsv_debug`'s printer read (`crates/tsv_lang/src/tape/shapes.rs`).
 *
 * Both are output: `npm run ast-schema:update` regenerates them with the `.d.ts`, and
 * `npm run check:ast-schema` holds each byte-for-byte to the committed file. The decoder is laid
 * out the way `tsv format` lays it out, so `format:audit` holds it to the formatter's fixed
 * point like every other JS file; the Rust file is `#[rustfmt::skip]` where its module is
 * declared, its tables one entry per line.
 *
 * The decoder's shape: one small function per fast shape, building the record as one object
 * literal in wire key order, dispatched through a table by the record's header word — never one
 * large `switch`; and one iterative interpreter over generated descriptors (`slow`) for every
 * other record, which also re-decodes a whole tape when the recursive fast path overflows the
 * stack (`docs/tape_format.md` §Decoding).
 *
 * @module
 */

import {
	Enc,
	ENC_NAMES,
	TAPE_FORMAT_VERSION,
	TAPE_HEADER_WORDS,
	TAPE_MAGIC,
	TAPE_MAX_ENTRIES,
	pre_type_max,
	TAPE_HOT_PATH,
	TAPE_SHAPES_PATH,
	type TapeEntry,
	type TapeSchema
} from './tape_schema.ts';
import { NAME_LOC_KINDS, STAMP_HOSTS } from '../crates/tsv_wasm/npm/locations_core.js';

/** The AST schema table's path (`ast_schema.ts`'s `AST_SCHEMA_PATH`, which imports this module). */
const AST_SCHEMA_PATH = 'crates/tsv_wasm/types/tsv_ast_schema.json';

/** The decoder's path, from the checkout root. */
export const TAPE_DECODER_PATH = 'crates/tsv_wasm/npm/tape_decode.js';

/** The Rust tape tables' path, from the checkout root. */
export const TAPE_RUST_TABLES_PATH = 'crates/tsv_lang/src/tape/shapes.rs';

const hex = (n: number): string => `0x${n.toString(16).padStart(8, '0')}`;

/** A JS string literal, single-quoted (`tsv format`'s preference). */
const js_string = (text: string): string => {
	const json = JSON.stringify(text);
	const body = json.slice(1, -1).replace(/\\"/g, '"').replace(/'/g, "\\'");
	return `'${body}'`;
};

/** A JS property name: bare when it is an identifier, else quoted. */
const js_key = (key: string): string =>
	/^[A-Za-z_$][A-Za-z0-9_$]*$/.test(key) ? key : js_string(key);

/** The keyword vocabularies, each distinct list once, and each list's index by its JSON. */
function keyword_tables(schema: TapeSchema): { lists: string[][]; index: Map<string, number> } {
	const lists: string[][] = [];
	const index = new Map<string, number>();
	for (const entry of schema.entries) {
		for (const field of entry.fields) {
			if (field.values === undefined) continue;
			const key = JSON.stringify(field.values);
			if (!index.has(key)) {
				index.set(key, lists.length);
				lists.push([...field.values]);
			}
		}
	}
	return { lists, index };
}

/**
 * An array literal: one line when it fits, else broken as `tsv format` breaks it — numbers
 * filled onto as few lines as the width allows, anything else one item per line.
 */
function js_array(prefix: string, items: string[], suffix: string, indent = ''): string[] {
	const width = (line: string): number => line.length + (line.match(/\t/g)?.length ?? 0);
	const one = `${indent}${prefix}[${items.join(', ')}]${suffix}`;
	if (width(one) <= 100) return [one];
	const inner = `${indent}\t`;
	const lines: string[] = [];
	if (items.every((item) => /^-?\d+$/.test(item))) {
		let line = '';
		items.forEach((item, i) => {
			const piece = i < items.length - 1 ? `${item},` : item;
			const next = line === '' ? piece : `${line} ${piece}`;
			if (line !== '' && width(`${inner}${next}`) > 100) {
				lines.push(`${inner}${line}`);
				line = piece;
			} else line = next;
		});
		lines.push(`${inner}${line}`);
	} else {
		items.forEach((item, i) => lines.push(`${inner}${item}${i < items.length - 1 ? ',' : ''}`));
	}
	return [`${indent}${prefix}[`, ...lines, `${indent}]${suffix}`];
}

/** One descriptor table: an array per entry, one line each when it fits. */
function entry_table(name: string, doc: string, rows: string[][]): string[] {
	const out = [`/** ${doc} */`, `const ${name} = [`];
	rows.forEach((row, n) => out.push(...js_array('', row, n < rows.length - 1 ? ',' : '', '\t')));
	out.push('];');
	return out;
}

/**
 * One descriptor table as a `JSON.parse` of a string literal: a JS engine scans a string far
 * faster than it parses the same data as an array literal, and the table is read once at load.
 */
function json_table(name: string, doc: string, value: unknown): string[] {
	return [
		`/** ${doc} */`,
		`const ${name} = JSON.parse(`,
		`\t${js_string(JSON.stringify(value))}`,
		');'
	];
}

/**
 * One fast shape's function: `s<id>`, its plain record, or with `loc` its loc twin `q<id>` —
 * the loc wire's record: `loc` right after `end`, a Svelte `name_loc` right after `name`,
 * nested reads through the loc-mode readers, and a record `stamp_character_locs` re-stamps
 * handed to the decode's host list.
 */
function shape_function(
	schema: TapeSchema,
	shape: TapeSchema['shapes'][number],
	loc: boolean,
	keyword_of: (values: ReadonlyArray<string>) => number
): string[] {
	const entry = schema.entries[shape.entry]!;
	const present = entry.fields.filter((f) => f.bit === -1 || (shape.mask & (1 << f.bit)) !== 0);
	const own = present.some((f) => f.enc === Enc.own);
	const spanned = loc && entry.loc;
	const named = spanned && entry.name_loc;
	const host = loc && entry.type !== null && STAMP_HOSTS.has(entry.type);
	const bind = own || spanned;
	const props: string[] = [];
	for (const f of present) {
		let ex: string;
		switch (f.enc) {
			case Enc.type:
				ex = js_string(entry.type!);
				break;
			case Enc.offset:
				ex = bind && (f.key === 'start' || f.key === 'end') ? `(${f.key[0]} = u[p++])` : 'u[p++]';
				break;
			case Enc.boolean:
				ex = 'u[p++] !== 0';
				break;
			case Enc.number:
				ex = 'N()';
				break;
			case Enc.null:
				ex = 'null';
				break;
			case Enc.own:
				ex = 'src.slice(s, e)';
				break;
			case Enc.keyword:
				ex = `K${keyword_of(f.values!)}[u[p++]]`;
				break;
			case Enc.ref:
				ex = 'S()';
				break;
			case Enc.ref_nullable:
				ex = 'SN()';
				break;
			case Enc.strings:
				ex = 'L()';
				break;
			case Enc.record:
				ex = loc ? 'RL()' : 'R()';
				break;
			case Enc.list:
				ex = loc ? 'AL()' : 'A()';
				break;
			default:
				ex = loc ? 'VL()' : 'V()';
		}
		if (named && f.key === 'name') ex = `(n = ${ex})`;
		props.push(`${js_key(f.key)}: ${ex}`);
		if (spanned && f.key === 'end') props.push('loc: LOC(s, e)');
		if (named && f.key === 'name') {
			props.push(`name_loc: name_loc_of(${js_string(name_loc_kind(entry))}, s, e, n, src, ls, lh)`);
		}
	}
	const out = [
		`/** ${entry.name}, mask ${shape.mask}${loc ? ', with its loc-wire fields' : ''} */`,
		`function ${loc ? 'q' : 's'}${shape.id}() {`
	];
	if (named) out.push('\tlet s, e, n;');
	else if (bind) out.push('\tlet s, e;');
	out.push(host ? '\tconst o = {' : '\treturn {');
	props.forEach((prop, n) => out.push(`\t\t${prop}${n < props.length - 1 ? ',' : ''}`));
	out.push('\t};');
	if (host) out.push('\tif (hosts !== null) hosts.push(o);', '\treturn o;');
	out.push('}');
	return out;
}

/** A `name_loc` entry's kind (`locations_core.js`'s `NAME_LOC_KINDS`, keyed by `type`). */
function name_loc_kind(entry: TapeEntry): string {
	const kind = entry.type === null ? undefined : NAME_LOC_KINDS.get(entry.type);
	if (kind === undefined) throw new Error(`tape_gen: ${entry.name} has no name_loc kind`);
	return kind;
}

/**
 * Whether the loc-wire fields the table gives each entry are the ones `locations_core.js`
 * builds: a `name_loc` exactly on the entries whose `type` `NAME_LOC_KINDS` names (every such
 * entry with a `name`), so the decoder's entry-generic paths may ask `NAME_LOC_KINDS` by `type`,
 * as `locations.js` does.
 */
function loc_wire_errors(schema: TapeSchema): string[] {
	const errors: string[] = [];
	for (const entry of schema.entries) {
		const kind = entry.type === null ? undefined : NAME_LOC_KINDS.get(entry.type);
		const named = entry.fields.some((f) => f.key === 'name');
		if (entry.name_loc && kind === undefined) {
			errors.push(`${entry.name}: a name_loc, but locations_core.js names no kind for its type`);
		} else if (!entry.name_loc && kind !== undefined && named) {
			errors.push(`${entry.name}: no name_loc, but locations_core.js names a kind for its type`);
		}
	}
	return errors;
}

/**
 * The generated JS decoder's text. `hot` names the fast shapes whose loc twin it writes: every
 * other shape's loc-mode record is read by `X`, the entry-generic loc reader (or by its plain
 * function, where its twin would be the plain function's body).
 */
export function generate_tape_decoder(schema: TapeSchema, hot: ReadonlySet<number>): string {
	const loc_errors = loc_wire_errors(schema);
	if (loc_errors.length > 0) throw new Error(`tape_gen: ${loc_errors.join('; ')}`);
	const { lists, index } = keyword_tables(schema);
	const keyword_of = (values: ReadonlyArray<string>): number => index.get(JSON.stringify(values))!;
	const out: string[] = [
		`// GENERATED by ` + '`npm run ast-schema:update`' + ` from ${AST_SCHEMA_PATH},`,
		`// ${TAPE_SHAPES_PATH} and ${TAPE_HOT_PATH} — edit those, never this file.`,
		...HEAD.split('\n'),
		`export const TAPE_FORMAT_VERSION = ${TAPE_FORMAT_VERSION};`,
		'',
		'/** The hash of the schema and shape list this decoder was generated from. */',
		`export const SHAPES_HASH = ${hex(schema.hash)};`,
		'',
		`const MAGIC = ${hex(TAPE_MAGIC)};`,
		`const HEADER_WORDS = ${TAPE_HEADER_WORDS};`,
		`const HEADER_BYTES = ${TAPE_HEADER_WORDS * 4};`,
		'const NULL_REF = -0x80000000;',
		`const MAX_ENTRIES = ${TAPE_MAX_ENTRIES};`,
		...STATE.split('\n'),
		...json_table('KW', "The keyword vocabularies, indexed by a keyword field's word.", lists)
	];
	lists.forEach((_, k) => out.push(`const K${k} = KW[${k}];`));
	out.push(
		...json_table(
			'ENTRY_TYPE',
			'Per tape entry: its `type` (`null` when it has none).',
			schema.entries.map((e) => e.type)
		),
		...json_table(
			'ENTRY_KEYS',
			'Per tape entry: its keys, in wire order.',
			schema.entries.map((e) => e.fields.map((f) => f.key))
		),
		...json_table(
			'ENTRY_ENCODINGS',
			`Per tape entry: each field's encoding (${ENC_NAMES.map((name, code) => `${code} ${name}`).join(', ')}).`,
			schema.entries.map((e) => e.fields.map((f) => f.enc))
		),
		...json_table(
			'ENTRY_BITS',
			"Per tape entry: each field's mask bit, -1 when every record carries it.",
			schema.entries.map((e) => e.fields.map((f) => f.bit))
		),
		...json_table(
			'ENTRY_KEYWORDS',
			"Per tape entry: each keyword field's vocabulary in KW, -1 for any other field.",
			schema.entries.map((e) =>
				e.fields.map((f) => (f.values === undefined ? -1 : keyword_of(f.values)))
			)
		),
		...json_table('SHAPE_ENTRY', 'Per fast shape id: its entry.', [
			0,
			...schema.shapes.map((s) => s.entry)
		]),
		...json_table('SHAPE_MASK', 'Per fast shape id: its mask.', [
			0,
			...schema.shapes.map((s) => s.mask)
		]),
		...STATIC.split('\n')
	);
	for (const shape of schema.shapes) {
		out.push('', ...shape_function(schema, shape, false, keyword_of));
	}
	out.push(
		'',
		'/** The fast shapes, by header word. */',
		...js_array('const SHAPES = ', ['null', ...schema.shapes.map((s) => `s${s.id}`)], ';'),
		...STATIC_LOC.split('\n')
	);
	// a hot shape gets its twin `q<id>`; a shape whose twin would be its plain body (no span, no
	// nested read) reuses `s<id>`; every other shape's loc-mode record is read by `X`
	const names: string[] = ['null'];
	for (const shape of schema.shapes) {
		const plain = shape_function(schema, shape, false, keyword_of);
		const twin = shape_function(schema, shape, true, keyword_of);
		if (twin.slice(2).join('\n') === plain.slice(2).join('\n')) names.push(`s${shape.id}`);
		else if (hot.has(shape.id)) {
			out.push('', ...twin);
			names.push(`q${shape.id}`);
		} else names.push('X');
	}
	out.push(
		'',
		'/** The fast shapes in loc mode, by header word: a twin, a plain function, or `X`. */',
		...js_array('const SHAPES_LOC = ', names, ';')
	);
	return `${out.join('\n')}\n`;
}

/** The decoder's module doc. */
const HEAD = `
/**
 * The tape decoder: the binary parse wire (\`docs/tape_format.md\`) to the same tree
 * \`JSON.parse\` makes of the wire's JSON, key order included.
 *
 * One small function per fast shape builds its record as one object literal; every other
 * record — an entry-generic one, a self-describing one — goes through \`slow\`, an iterative
 * interpreter over the descriptor tables below. The fast path recurses, one call per nested
 * record, and a tree deep enough to overflow the stack throws a \`RangeError\` out of it: the
 * decoder then decodes the same tape again from its start with \`slow\`, which recurses
 * nowhere. Every module-level variable is set from the tape before either pass reads it, so
 * nothing an abandoned fast pass left behind reaches the second one.
 *
 * Generated, with the descriptor tables and one function per fast shape, by
 * \`scripts/tape_gen.ts\`, where the hand-written part below lives.
 *
 * **Loc mode.** Handed a language, a decode builds the \`loc\`-bearing wire instead — the tree
 * \`JSON.parse\` makes of \`tsv parse --locations\`'s JSON, key order included: \`loc\` right after
 * \`end\` on every record with offsets, a Svelte \`name_loc\` right after \`name\`, and Svelte's
 * \`character\`-bearing positions re-stamped once the tree is whole. The rules are
 * \`locations_core.js\`'s, the ones \`locations.js\` reconstructs by. The hottest fast shapes
 * (\`tsv_tape_hot.json\`) have a generated twin that builds the record with its \`loc\` in one
 * literal; every other record is read by \`X\`, which reads its entry's descriptors as \`slow\`
 * does, or by its plain function where nothing in it moves.
 *
 * @module
 */

import {
	build_line_starts,
	line_index_near,
	NAME_LOC_KINDS,
	name_loc_of,
	STAMP_HOSTS,
	stamp_character_locs,
	stamp_in_tag_comment_locs
} from './locations_core.js';

/** The tape format's version, which the header's second word must carry. */`;

/** The decoder's module state. */
const STATE = `
/** \`ignoreBOM: true\` *keeps* a leading U+FEFF (the option name is inverted): the pool is
 * text, not a file, and a document whose first pooled value starts with U+FEFF must not lose it. */
const utf8 = new TextDecoder('utf-8', { ignoreBOM: true });
const F64 = new Float64Array(1);
const F64_WORDS = new Uint32Array(F64.buffer);
const NO_WORDS = new Uint32Array(0);
const NO_INTS = new Int32Array(0);
/** The tape's words are little-endian; the typed-array views read the host's order. */
const LITTLE_ENDIAN = new Uint8Array(new Uint32Array([1]).buffer)[0] === 1;

/** The tape body as unsigned words. */
let u = NO_WORDS;
/** The same words, signed (string refs and headers). */
let i = NO_INTS;
/** The read position, in words from the body start. */
let p = 0;
/** The source the tape was written against. */
let src = '';
/** The tape's string pool, decoded once. */
let pool = '';

const NO_LINES = [0];
/** Loc mode: whether this decode builds the loc wire's fields. */
let lm = false;
/** Loc mode: the line-start offsets of the source. */
let ls = NO_LINES;
/** Loc mode: the line of the last \`start\` looked up, where the next lookup starts. */
let lh = 0;
/**
 * Loc mode over a Svelte tape: the records \`stamp_character_locs\` re-stamps once the tree is
 * whole; \`null\` in every other decode.
 */
let hosts = null;
`;

/** The decoder's hand-written part: the entry points, the readers and the slow path. */
const STATIC = `
/**
 * Decodes a tape against \`source\`, the exact string the parse read.
 * @param {Uint8Array} bytes - the tape
 * @param {string} source
 * @param {'typescript' | 'svelte' | 'css'} [language] - the document's language, to decode the
 * \`loc\`-bearing wire under its line rule (loc mode, the module doc); unset, the span-only wire
 * @returns {unknown} the tree \`JSON.parse\` makes of the same wire's JSON
 * @throws {Error} when the header names another format, schema or shape list, or the tape is
 * malformed, or \`language\` names no language
 */
export function decode_tape(bytes, source, language) {
	const end = open(bytes, source, language);
	try {
		let tree;
		try {
			tree = lm ? RL() : R();
		} catch (error) {
			if (!(error instanceof RangeError)) throw error;
			p = 0;
			lh = 0;
			if (hosts !== null) hosts = [];
			tree = slow();
		}
		if (p !== end) throw new Error(\`tape: \${end - p} words left after the root record\`);
		if (hosts !== null) restamp(tree);
		return tree;
	} finally {
		close();
	}
}

/**
 * Decodes a tape with the iterative interpreter alone — what \`decode_tape\` falls back to on a
 * stack overflow, exposed so a gate can run every tape through it.
 * @param {Uint8Array} bytes - the tape
 * @param {string} source
 * @param {'typescript' | 'svelte' | 'css'} [language] - as \`decode_tape\` takes it
 * @returns {unknown}
 */
export function decode_tape_iterative(bytes, source, language) {
	const end = open(bytes, source, language);
	try {
		const tree = slow();
		if (p !== end) throw new Error(\`tape: \${end - p} words left after the root record\`);
		if (hosts !== null) restamp(tree);
		return tree;
	} finally {
		close();
	}
}

/**
 * Reads the header and points the module state at the body, the pool and, in loc mode, the
 * source's line table.
 * @param {Uint8Array} bytes
 * @param {string} source
 * @param {string | undefined} language
 * @returns {number} the body's length in words
 */
function open(bytes, source, language) {
	if (
		language !== undefined &&
		language !== 'typescript' &&
		language !== 'svelte' &&
		language !== 'css'
	) {
		throw new Error(\`tape: no line rule for the language \${String(language)}\`);
	}
	if (!LITTLE_ENDIAN) throw new Error('tape: this decoder reads a little-endian host only');
	if (bytes.byteOffset % 4 !== 0) bytes = bytes.slice();
	if (bytes.byteLength < HEADER_BYTES) throw new Error('tape: shorter than its header');
	const header = new Uint32Array(bytes.buffer, bytes.byteOffset, HEADER_WORDS);
	if (header[0] !== MAGIC) throw new Error('tape: not a tape (bad magic)');
	if (header[1] !== TAPE_FORMAT_VERSION) {
		throw new Error(\`tape: format version \${header[1]}, this decoder reads \${TAPE_FORMAT_VERSION}\`);
	}
	if (header[2] !== SHAPES_HASH) {
		throw new Error('tape: written against another schema or shape list (SHAPES_HASH differs)');
	}
	const words = header[3];
	const pool_bytes = header[4];
	const pool_start = HEADER_BYTES + words * 4;
	if (bytes.byteLength !== pool_start + pool_bytes) {
		throw new Error('tape: its length disagrees with its header');
	}
	u = new Uint32Array(bytes.buffer, bytes.byteOffset + HEADER_BYTES, words);
	i = new Int32Array(bytes.buffer, bytes.byteOffset + HEADER_BYTES, words);
	pool = pool_bytes === 0 ? '' : utf8.decode(bytes.subarray(pool_start, pool_start + pool_bytes));
	if (pool.length !== header[6]) throw new Error('tape: its pool disagrees with its header');
	if (source.length !== header[7]) {
		throw new Error(
			'tape: written against another source (its UTF-16 length is not the one in its header)'
		);
	}
	src = source;
	p = 0;
	lh = 0;
	lm = language !== undefined;
	ls = lm ? build_line_starts(source, language === 'typescript') : NO_LINES;
	hosts = language === 'svelte' ? [] : null;
	return words;
}

/** Drops the module's hold on the last tape and source. */
function close() {
	u = NO_WORDS;
	i = NO_INTS;
	src = '';
	pool = '';
	lm = false;
	ls = NO_LINES;
	hosts = null;
}

/** A string ref. */
function S() {
	const a = i[p];
	const b = i[p + 1];
	p += 2;
	return a >= 0 ? src.slice(a, b) : pool.slice(-1 - a, b);
}

/** A string ref or the null ref. */
function SN() {
	const a = i[p];
	const b = i[p + 1];
	p += 2;
	if (a >= 0) return src.slice(a, b);
	return a === NULL_REF ? null : pool.slice(-1 - a, b);
}

/** An inline \`f64\`. */
function N() {
	F64_WORDS[0] = u[p];
	F64_WORDS[1] = u[p + 1];
	p += 2;
	return F64[0];
}

/** A count, then that many string refs. */
function L() {
	const n = u[p++];
	const a = new Array(n);
	for (let k = 0; k < n; k++) a[k] = S();
	return a;
}

/** A count, then that many records. */
function A() {
	const n = u[p++];
	const a = new Array(n);
	for (let k = 0; k < n; k++) a[k] = R();
	return a;
}

/** A record: a fast shape by its function, \`null\`, or any other through \`slow\`. */
function R() {
	const h = i[p];
	if (h > 0) {
		const shape = SHAPES[h];
		if (shape === undefined) throw new Error(\`tape: no fast shape \${h}\`);
		p++;
		return shape();
	}
	if (h === 0) {
		p++;
		return null;
	}
	return slow();
}

/** A tagged value. */
function V() {
	switch (u[p++]) {
		case 0:
			return null;
		case 1:
			return false;
		case 2:
			return true;
		case 3:
			return N();
		case 4:
			return S();
		case 5: {
			const n = u[p++];
			const a = new Array(n);
			for (let k = 0; k < n; k++) a[k] = V();
			return a;
		}
		case 6:
			return R();
	}
	throw new Error(\`tape: bad value tag at word \${p - 1}\`);
}

/** Sets an own key, \`__proto__\` too (a self-describing record may hold one). */
function put(o, key, v) {
	if (key === '__proto__') {
		Object.defineProperty(o, key, {
			value: v,
			writable: true,
			enumerable: true,
			configurable: true
		});
	} else o[key] = v;
}

/**
 * Decodes the record at \`p\` and everything inside it with an explicit stack — no recursion, so
 * no depth limit but memory. A frame is a typed record (\`t: 0\`, filled field by field from its
 * entry's descriptors), a self-describing one (\`t: 1\`), or an array (\`t: 2\`: of records when
 * \`r\`, else of tagged values). A container is put into its parent when it is made, so every key
 * lands in wire order.
 * @returns {unknown}
 */
function slow() {
	const stack = [];
	let root = null;
	let want_record = true;
	for (;;) {
		let v;
		let frame = null;
		if (want_record) {
			const h = i[p++];
			if (h === 0) v = null;
			else if (h === -1) {
				v = {};
				frame = { t: 1, o: v, n: u[p++], k: 0, key: '', e: 0, m: 0, f: 0, a: null, r: false };
			} else {
				let e;
				let m;
				if (h > 0) {
					e = SHAPE_ENTRY[h];
					m = SHAPE_MASK[h];
					if (e === undefined) throw new Error(\`tape: no fast shape \${h}\`);
				} else {
					const x = -h - 2;
					e = x % MAX_ENTRIES;
					m = (x - e) / MAX_ENTRIES;
					if (e >= ENTRY_KEYS.length) throw new Error(\`tape: no entry \${e}\`);
				}
				v = {};
				frame = { t: 0, o: v, n: 0, k: 0, key: '', e, m, f: 0, a: null, r: false };
			}
		} else {
			switch (u[p++]) {
				case 0:
					v = null;
					break;
				case 1:
					v = false;
					break;
				case 2:
					v = true;
					break;
				case 3:
					v = N();
					break;
				case 4:
					v = S();
					break;
				case 5: {
					const n = u[p++];
					v = new Array(n);
					frame = { t: 2, o: null, n, k: 0, key: '', e: 0, m: 0, f: 0, a: v, r: false };
					break;
				}
				case 6:
					want_record = true;
					continue;
				default:
					throw new Error(\`tape: bad value tag at word \${p - 1}\`);
			}
		}
		if (stack.length === 0) root = v;
		else {
			const top = stack[stack.length - 1];
			if (top.t === 2) top.a[top.k++] = v;
			else if (top.t === 1) {
				put(top.o, top.key, v);
				top.k++;
			} else top.o[ENTRY_KEYS[top.e][top.f++]] = v;
		}
		if (frame !== null) stack.push(frame);
		// find the next nested read, closing every container that is full
		for (;;) {
			if (stack.length === 0) return root;
			const top = stack[stack.length - 1];
			if (top.t === 2) {
				if (top.k < top.n) {
					want_record = top.r;
					break;
				}
				stack.pop();
				continue;
			}
			if (top.t === 1) {
				if (top.k < top.n) {
					top.key = S();
					want_record = false;
					break;
				}
				stack.pop();
				continue;
			}
			const next = fill(top, stack);
			if (next === 0) {
				want_record = true;
				break;
			}
			if (next === 1) {
				want_record = false;
				break;
			}
			if (next === 3) stack.pop();
		}
	}
}

/**
 * Reads a typed record's fields from \`frame.f\` on, until one needs a nested read.
 * @returns {number} \`0\` a record next, \`1\` a tagged value next, \`2\` a list frame pushed,
 * \`3\` the record is complete
 */
function fill(frame, stack) {
	const e = frame.e;
	const m = frame.m;
	const o = frame.o;
	const keys = ENTRY_KEYS[e];
	const encodings = ENTRY_ENCODINGS[e];
	const bits = ENTRY_BITS[e];
	for (let f = frame.f; f < keys.length; f++) {
		const bit = bits[f];
		if (bit >= 0 && (m & (1 << bit)) === 0) continue;
		const key = keys[f];
		switch (encodings[f]) {
			case 0:
				o[key] = ENTRY_TYPE[e];
				break;
			case 1:
				o[key] = u[p++];
				break;
			case 2:
				o[key] = u[p++] !== 0;
				break;
			case 3:
				o[key] = N();
				break;
			case 4:
				o[key] = null;
				break;
			case 5:
				o[key] = src.slice(o.start, o.end);
				break;
			case 6:
				o[key] = KW[ENTRY_KEYWORDS[e][f]][u[p++]];
				break;
			case 7:
				o[key] = S();
				break;
			case 8:
				o[key] = SN();
				break;
			case 9:
				o[key] = L();
				break;
			case 10:
				frame.f = f;
				return 0;
			case 11: {
				const n = u[p++];
				const a = new Array(n);
				o[key] = a;
				frame.f = f + 1;
				stack.push({ t: 2, o: null, n, k: 0, key: '', e: 0, m: 0, f: 0, a, r: true });
				return 2;
			}
			default:
				frame.f = f;
				return 1;
		}
		if (lm && (key === 'end' || key === 'name')) loc_field(o, key, e);
	}
	if (hosts !== null && STAMP_HOSTS.has(ENTRY_TYPE[e])) hosts.push(o);
	return 3;
}`;

/** The decoder's loc-mode readers (the module doc), after the fast shapes. */
const STATIC_LOC = `
/**
 * A record's \`loc\` over its own span: the start's line searched from the last one looked up,
 * the end's from the start's.
 * @param {number} s
 * @param {number} e
 */
function LOC(s, e) {
	const a = (lh = line_index_near(s, ls, lh));
	const b = line_index_near(e, ls, a);
	return {
		start: { line: a + 1, column: s - ls[a] },
		end: { line: b + 1, column: e - ls[b] }
	};
}

/**
 * The loc-wire field a typed record takes after its \`key\`, read by the entry-generic paths:
 * \`loc\` after an offset \`end\` (every entry with offset \`start\` and \`end\` carries one), a
 * Svelte \`name_loc\` after the \`name\` of an entry whose \`type\` \`NAME_LOC_KINDS\` names.
 * @param {any} o - the record, filled through \`key\`
 * @param {string} key
 * @param {number} e - its entry
 */
function loc_field(o, key, e) {
	if (typeof o.start !== 'number' || typeof o.end !== 'number') return;
	if (key === 'end') o.loc = LOC(o.start, o.end);
	else {
		const kind = NAME_LOC_KINDS.get(ENTRY_TYPE[e]);
		if (kind !== undefined) o.name_loc = name_loc_of(kind, o.start, o.end, o.name, src, ls, lh);
	}
}

/**
 * A loc-mode fast shape with no twin: its fields read from its entry's descriptors, nested reads
 * through the loc-mode readers (so only this record, not its subtree, is generic).
 */
function X() {
	const h = i[p - 1];
	const e = SHAPE_ENTRY[h];
	const m = SHAPE_MASK[h];
	const keys = ENTRY_KEYS[e];
	const encodings = ENTRY_ENCODINGS[e];
	const bits = ENTRY_BITS[e];
	const o = {};
	for (let f = 0; f < keys.length; f++) {
		const bit = bits[f];
		if (bit >= 0 && (m & (1 << bit)) === 0) continue;
		const key = keys[f];
		switch (encodings[f]) {
			case 0:
				o[key] = ENTRY_TYPE[e];
				break;
			case 1:
				o[key] = u[p++];
				break;
			case 2:
				o[key] = u[p++] !== 0;
				break;
			case 3:
				o[key] = N();
				break;
			case 4:
				o[key] = null;
				break;
			case 5:
				o[key] = src.slice(o.start, o.end);
				break;
			case 6:
				o[key] = KW[ENTRY_KEYWORDS[e][f]][u[p++]];
				break;
			case 7:
				o[key] = S();
				break;
			case 8:
				o[key] = SN();
				break;
			case 9:
				o[key] = L();
				break;
			case 10:
				o[key] = RL();
				break;
			case 11:
				o[key] = AL();
				break;
			default:
				o[key] = VL();
		}
		if (key === 'end' || key === 'name') loc_field(o, key, e);
	}
	if (hosts !== null && STAMP_HOSTS.has(ENTRY_TYPE[e])) hosts.push(o);
	return o;
}

/** \`R\` in loc mode. */
function RL() {
	const h = i[p];
	if (h > 0) {
		const shape = SHAPES_LOC[h];
		if (shape === undefined) throw new Error(\`tape: no fast shape \${h}\`);
		p++;
		return shape();
	}
	if (h === 0) {
		p++;
		return null;
	}
	return slow();
}

/** \`A\` in loc mode. */
function AL() {
	const n = u[p++];
	const a = new Array(n);
	for (let k = 0; k < n; k++) a[k] = RL();
	return a;
}

/** \`V\` in loc mode. */
function VL() {
	switch (u[p++]) {
		case 0:
			return null;
		case 1:
			return false;
		case 2:
			return true;
		case 3:
			return N();
		case 4:
			return S();
		case 5: {
			const n = u[p++];
			const a = new Array(n);
			for (let k = 0; k < n; k++) a[k] = VL();
			return a;
		}
		case 6:
			return RL();
	}
	throw new Error(\`tape: bad value tag at word \${p - 1}\`);
}

/**
 * Loc mode over a Svelte tape, once the tree is whole: the \`character\`-bearing positions
 * Svelte's own template reader creates, \`locations.js\`'s two stamps — each overwriting a
 * \`loc\` already in place, so no key moves.
 * @param {any} tree
 */
function restamp(tree) {
	for (const host of hosts) stamp_character_locs(host, ls, src);
	if (Array.isArray(tree?.comments)) stamp_in_tag_comment_locs(tree, ls, src);
}`;

/** The Rust name of an encoding. */
const rust_enc = (code: number): string =>
	ENC_NAMES[code]!.split('_')
		.map((part) => part[0]!.toUpperCase() + part.slice(1))
		.join('');

/** A Rust string literal. */
const rust_string = (text: string): string => JSON.stringify(text);

const by_text = (a: string, b: string): number => (a < b ? -1 : a > b ? 1 : 0);

/** The generated Rust tables' text. */
export function generate_tape_rust(schema: TapeSchema): string {
	const { lists, index } = keyword_tables(schema);
	const out: string[] = [
		'//! GENERATED by `npm run ast-schema:update` from',
		`//! ` +
			'`' +
			`${AST_SCHEMA_PATH}` +
			'` and `' +
			`${TAPE_SHAPES_PATH}` +
			'`' +
			` — edit those,`,
		'//! never this file; `npm run check:ast-schema` holds the two together.',
		'//!',
		"//! The tape schema every tape producer writes against and `tsv_debug`'s printer reads: the",
		'//! tape entries with their fields in wire order, the keyword vocabularies, the fast shapes,',
		'//! the byte-keyed lookup of the keys an entry carries ahead of its `type` (the transcoder',
		'//! builds its `type` lookup from `ENTRIES` itself, at compile time), and the name lookups a',
		"//! direct writer's sites ask — an entry by name, a field by key, a keyword by its text —",
		'//! as `const fn` matches, answered at compile time in a `const` and as a jump and a compare',
		'//! at run time.',
		'',
		'use super::{Enc, Entry, Field, NO_AUX, NO_BIT};',
		'',
		"/// The tape format's version, the header's second word.",
		`pub const TAPE_FORMAT_VERSION: u32 = ${TAPE_FORMAT_VERSION};`,
		'',
		'/// The most keys a typed entry carries ahead of its `type`: what the transcoder holds',
		'/// while it reads them, before the `type` names a candidate.',
		`pub const PRE_TYPE_MAX: usize = ${pre_type_max(schema.entries)};`,
		'',
		'/// The hash of the schema and shape list these tables were generated from.',
		`pub const SHAPES_HASH: u32 = ${hex(schema.hash)};`,
		'',
		'/// The keyword vocabularies a `keyword` field names by its `aux`.',
		`pub static KEYWORDS: [&[&str]; ${lists.length}] = [`
	];
	for (const list of lists) out.push(`    &[${list.map(rust_string).join(', ')}],`);
	out.push(
		'];',
		'',
		'/// The tape entries, by id.',
		`pub static ENTRIES: [Entry; ${schema.entries.length}] = [`
	);
	for (const e of schema.entries) {
		out.push(
			'    Entry {',
			`        name: ${rust_string(e.name)},`,
			`        ty: ${e.type === null ? 'None' : `Some(${rust_string(e.type)})`},`,
			'        fields: &['
		);
		for (const f of e.fields) {
			const bit = f.bit === -1 ? 'NO_BIT' : String(f.bit);
			const aux =
				f.values !== undefined
					? String(index.get(JSON.stringify(f.values)))
					: f.entry !== undefined
						? String(f.entry)
						: 'NO_AUX';
			out.push(
				`            Field { key: ${rust_string(f.key)}, enc: Enc::${rust_enc(f.enc)}, bit: ${bit}, aux: ${aux} },`
			);
		}
		out.push('        ],', '    },');
	}
	out.push(
		'];',
		'',
		"/// The fast shapes as `(entry, mask)`, sorted; a shape's id is its index plus one.",
		`pub static SHAPES: [(u16, u32); ${schema.shapes.length}] = [`
	);
	for (const s of schema.shapes) out.push(`    (${s.entry}, ${s.mask}),`);
	out.push(
		'];',
		'',
		'/// The encoding of a key some typed entry carries ahead of its `type`.',
		'#[must_use]',
		'pub fn pre_type_encoding(key: &[u8]) -> Option<Enc> {',
		'    match key {'
	);
	for (const [key, enc] of [...schema.pre_type].sort((a, b) => by_text(a[0], b[0]))) {
		out.push(`        b${rust_string(key)} => Some(Enc::${rust_enc(enc)}),`);
	}
	out.push('        _ => None,', '    }', '}');
	out.push(
		'',
		'/// The id of the tape entry named `name` (`ENTRIES[id].name`: `Identifier`, `css:Rule`,',
		'/// `~CSSRuleMetadata`).',
		'#[must_use]',
		'pub const fn entry_named(name: &[u8]) -> Option<u16> {',
		'    match name {'
	);
	schema.entries.forEach((e, id) => out.push(`        b${rust_string(e.name)} => Some(${id}),`));
	out.push(
		'        _ => None,',
		'    }',
		'}',
		'',
		"/// The index of the field `key` in entry `entry`'s `fields`.",
		'#[must_use]',
		'pub const fn field_named(entry: u16, key: &[u8]) -> Option<u8> {',
		'    match entry {'
	);
	schema.entries.forEach((e, id) => {
		if (e.fields.length === 0) return;
		const arms = e.fields.map((f, i) => `b${rust_string(f.key)} => Some(${i})`);
		out.push(`        ${id} => match key {`);
		for (const arm of arms) out.push(`            ${arm},`);
		out.push('            _ => None,', '        },');
	});
	out.push(
		'        _ => None,',
		'    }',
		'}',
		'',
		'/// The index of the keyword `word` in vocabulary `vocab` (`KEYWORDS[vocab]`).',
		'#[must_use]',
		'pub const fn keyword_named(vocab: u16, word: &[u8]) -> Option<u32> {',
		'    match vocab {'
	);
	lists.forEach((list, vocab) => {
		out.push(`        ${vocab} => match word {`);
		list.forEach((w, i) => out.push(`            b${rust_string(w)} => Some(${i}),`));
		out.push('            _ => None,', '        },');
	});
	out.push('        _ => None,', '    }', '}');
	return `${out.join('\n')}\n`;
}
