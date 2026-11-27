/**
 * The tape's schema: what the binary parse wire (the tape, `docs/tape_format.md`) derives from
 * the AST schema table (`tsv_ast_schema.json`) and the committed fast-shape list
 * (`tsv_tape_shapes.json`).
 *
 * The table says, per wire node type, its fields in wire order and each field's kind; the tape
 * turns each of those into one ENCODING (how many words a field takes and what they mean), and
 * the entries into TAPE ENTRIES — the table's entries with their `loc`-only fields dropped, an
 * entry whose encoding is identical to an earlier one's folded into it (a variant whose fields
 * and encodings match its base entry's never needs an id of its own). A record's body is a pure
 * function of its tape entry and its presence MASK — one bit per field the entry can leave out —
 * so a record names its layout by a FAST SHAPE id (an entry × mask pair the committed list holds,
 * decoded by a generated literal) or by the entry and mask themselves (the entry-generic record,
 * decoded key by key).
 *
 * Every reader and writer of the tape outside the Rust transcoder works from this module: the
 * offline encoder (`tape_encode.ts`), the generator of the JS decoder and of the transcoder's
 * tables (`tape_gen.ts`), and the gate (`check_tape.ts`). `SHAPES_HASH` is a hash of everything
 * here that shapes the bytes, so a decoder refuses a tape made from another table or shape list.
 *
 * @module
 */

import type { AstSchema, AstSchemaEntry, AstSchemaField } from './ast_schema.ts';

/** The fast-shape list's path, from the checkout root. */
export const TAPE_SHAPES_PATH = 'crates/tsv_wasm/types/tsv_tape_shapes.json';

/** The hot-shape list's path, from the checkout root (`TapeHotFile`). */
export const TAPE_HOT_PATH = 'crates/tsv_wasm/types/tsv_tape_hot.json';

/**
 * The tape format's version, the header's second word. Bumped by hand when the format's rules
 * change (an encoding, the header, the string-ref rule); a regenerated table or shape list moves
 * `SHAPES_HASH` instead.
 */
export const TAPE_FORMAT_VERSION = 2;

/** The header's first word: the bytes `TSVT` read as a little-endian `u32`. */
export const TAPE_MAGIC = 0x54565354;

/** Words in the header. */
export const TAPE_HEADER_WORDS = 8;

/** A string ref's first word for `null` (a nullable string field): `i32::MIN`. */
export const TAPE_NULL_REF = -0x80000000;

/** The widest mask an entry-generic header carries. */
export const TAPE_MASK_BITS = 20;

/** The most tape entries an entry-generic header can name (its low bits). */
export const TAPE_MAX_ENTRIES = 1 << (30 - TAPE_MASK_BITS);

/** How a field is laid down on the tape (`docs/tape_format.md` §Field encodings). */
export const Enc = {
	/** the record's `type`: no words, the entry's constant */
	type: 0,
	/** one word, an unsigned integer */
	offset: 1,
	/** one word, `0` or `1` */
	boolean: 2,
	/** two words, an `f64`'s little-endian bytes */
	number: 3,
	/** no words, always `null` */
	null: 4,
	/** no words: the source text between the record's own `start` and `end` */
	own: 5,
	/** one word, an index into the field's closed vocabulary */
	keyword: 6,
	/** two words, a string ref */
	ref: 7,
	/** two words, a string ref or the null ref */
	ref_nullable: 8,
	/** a count, then that many string refs */
	strings: 9,
	/** a nested record (header `0` is `null`) */
	record: 10,
	/** a count, then that many records */
	list: 11,
	/** a tagged value */
	value: 12
} as const;

export type EncCode = (typeof Enc)[keyof typeof Enc];

/** The name of each encoding, by code. */
export const ENC_NAMES: ReadonlyArray<string> = Object.keys(Enc);

/** A tagged value's tag word (`docs/tape_format.md` §Tagged values). */
export const Tag = {
	null: 0,
	false: 1,
	true: 2,
	number: 3,
	string: 4,
	array: 5,
	object: 6
} as const;

/** One field of a tape entry, in wire order. */
export interface TapeField {
	key: string;
	enc: EncCode;
	/** The field's mask bit, or `-1` for a field every record of the entry carries. */
	bit: number;
	/** A `keyword` field's vocabulary, in the table's order. */
	values?: ReadonlyArray<string>;
	/** A `record` field holding one `type`-less entry: that entry's id. */
	entry?: number;
}

/** One tape entry: a table entry minus its `loc` fields, identical twins folded. */
export interface TapeEntry {
	id: number;
	/** The table identity of the entry it was first derived from (`Type`, `Type/variant`, `css:Type`, `~Interface`). */
	name: string;
	/** The `type` value, or `null` for a `type`-less entry. */
	type: string | null;
	fields: ReadonlyArray<TapeField>;
	/** How many fields carry a mask bit. */
	bits: number;
	/**
	 * Whether the `loc` wire gives its records a `loc` — right after `end`, where the table's
	 * `presence: locations` field sits (every entry with required offset `start` and `end`).
	 * Not part of the tape (nor of `SHAPES_HASH`): the tape decoder's loc mode reads it.
	 */
	loc: boolean;
	/** Whether the `loc` wire gives its records a Svelte `name_loc`, right after `name`. */
	name_loc: boolean;
}

/** One fast shape: an id (from `1`), its entry and mask. */
export interface TapeShape {
	id: number;
	entry: number;
	mask: number;
}

/** The committed fast-shape list: entry names and masks. */
export interface TapeShapesFile {
	note: string[];
	shapes: Array<[entry: string, mask: number]>;
}

/** Everything the tape's bytes are derived from. */
export interface TapeSchema {
	entries: ReadonlyArray<TapeEntry>;
	/** Tape entry ids per `type` value, in table order: the candidates a typed object tries. */
	by_type: ReadonlyMap<string, ReadonlyArray<number>>;
	/** The keys some typed entry carries before its `type`, each with its one encoding. */
	pre_type: ReadonlyMap<string, EncCode>;
	shapes: ReadonlyArray<TapeShape>;
	/** Fast shape id per `shape_key(entry, mask)`. */
	shape_ids: ReadonlyMap<number, number>;
	/** FNV-1a over the canonical form of the entries and shapes (`tape_schema_canonical`). */
	hash: number;
}

/** What `derive_tape_entries` yields. */
export interface TapeEntries {
	entries: TapeEntry[];
	by_type: Map<string, number[]>;
	pre_type: Map<string, EncCode>;
}

const field_kinds = (field: AstSchemaField): string[] =>
	Array.isArray(field.kind) ? field.kind : [field.kind];

/** A field's encoding, from its kind, class and nullability; `null` for a `loc`-only field. */
export function field_encoding(field: AstSchemaField): EncCode | null {
	if (field.presence === 'locations') return null;
	const kinds = field_kinds(field);
	if (kinds.length > 1) return Enc.value;
	const nullable = field.nullable === true;
	switch (kinds[0]) {
		case 'type':
			return Enc.type;
		case 'offset':
			return nullable ? Enc.value : Enc.offset;
		case 'boolean':
			return nullable ? Enc.value : Enc.boolean;
		case 'number':
			return nullable ? Enc.value : Enc.number;
		case 'null':
			return Enc.null;
		case 'string':
			if (field.string === 'own') return nullable ? Enc.value : Enc.own;
			if (field.string === 'keyword') return nullable ? Enc.value : Enc.keyword;
			return nullable ? Enc.ref_nullable : Enc.ref;
		case 'strings':
			return nullable ? Enc.value : Enc.strings;
		case 'child':
		case 'object':
			return Enc.record;
		case 'list':
		case 'comments':
			return nullable ? Enc.value : Enc.list;
		case 'data':
			return Enc.value;
		default:
			return null;
	}
}

/** Whether a record of the entry may leave the field out, in either dialect. */
export const field_maskable = (field: AstSchemaField): boolean =>
	(field.presence ?? 'always') !== 'always' ||
	(field.vanilla_acorn !== undefined && field.vanilla_acorn !== 'always');

/** The table identity `ast_schema.ts` keys an entry by. */
export const entry_identity = (entry: AstSchemaEntry): string =>
	entry.type !== undefined
		? `${entry.css ? 'css:' : ''}${entry.type}${entry.variant ? `/${entry.variant}` : ''}`
		: `~${entry.interface}`;

/** FNV-1a, 32 bits, over a string's UTF-8 bytes. */
export function fnv1a32(text: string): number {
	let h = 0x811c9dc5;
	for (const byte of new TextEncoder().encode(text)) {
		h ^= byte;
		h = Math.imul(h, 0x01000193) >>> 0;
	}
	return h >>> 0;
}

/** One table entry's tape fields before folding, `entry` references still by interface name. */
interface Draft {
	name: string;
	type: string | null;
	interface: string | undefined;
	fields: Array<TapeField & { entry_interface?: string }>;
	loc: boolean;
	name_loc: boolean;
}

/**
 * The tape entries and lookups the table yields, before any shape list: every non-mixin entry,
 * its `loc` fields dropped, an entry identical to an earlier one (the same `type`, fields,
 * encodings and mask bits) folded into it. An `own` string ahead of its entry's required
 * `start` and `end` is carried as a ref, since a reader meets it before the span it names.
 * Refuses a table the tape cannot carry: more than `TAPE_MAX_ENTRIES` entries, more than
 * `TAPE_MASK_BITS` maskable fields in one, a pre-`type` key with two encodings or an
 * entry-dependent one, a `__proto__` key.
 */
export function derive_tape_entries(schema: AstSchema): TapeEntries | { errors: string[] } {
	const errors: string[] = [];
	const drafts: Draft[] = [];
	for (const section of schema.sections) {
		for (const item of section.entries) {
			if ('alias' in item || item.mixin) continue;
			const name = entry_identity(item);
			const fields: Draft['fields'] = [];
			let bit = 0;
			let loc = false;
			let name_loc = false;
			item.fields.forEach((field, at) => {
				if (field.presence !== 'locations') return;
				// the loc wire's own fields: where the tape decoder's loc mode builds each
				const before = item.fields[at - 1]?.key;
				if (field.key === 'loc' && before === 'end') loc = true;
				else if (field.key === 'name_loc' && before === 'name') name_loc = true;
				else {
					errors.push(
						`${name}.${field.key}: a loc-wire field the tape decoder cannot build ` +
							'(only `loc` right after `end` and `name_loc` right after `name`)'
					);
				}
			});
			for (const field of item.fields) {
				const enc = field_encoding(field);
				if (enc === null) {
					if (field.presence !== 'locations') {
						errors.push(`${name}.${field.key}: no tape encoding for its kind`);
					}
					continue;
				}
				if (field.key === '__proto__') errors.push(`${name}: a __proto__ key`);
				const out: Draft['fields'][number] = {
					key: field.key,
					enc,
					bit: field_maskable(field) ? bit++ : -1
				};
				if (enc === Enc.keyword) out.values = field.values ?? [];
				if (enc === Enc.record && field.entry !== undefined) out.entry_interface = field.entry;
				fields.push(out);
			}
			if (bit > TAPE_MASK_BITS) {
				errors.push(`${name}: ${bit} maskable fields, past the ${TAPE_MASK_BITS} a header carries`);
			}
			// an `own` field reads its record's span, so one ahead of the record's required `start`
			// and `end` (the CSS tree's `{type, name, start, end}`) is carried as a ref instead
			const required = (key: string): number =>
				fields.findIndex((f) => f.key === key && f.enc === Enc.offset && f.bit === -1);
			const s = required('start');
			const e = required('end');
			fields.forEach((f, at) => {
				if (f.enc === Enc.own && (s === -1 || e === -1 || s > at || e > at)) f.enc = Enc.ref;
			});
			// the loc wire's rule: every object with numeric `start` and `end` carries a `loc` —
			// so an entry has one exactly when both are required offsets
			if (loc !== (s !== -1 && e !== -1)) {
				errors.push(
					`${name}: ${loc ? 'a `loc` without' : 'no `loc` despite'} required offset \`start\` and \`end\``
				);
			}
			const named = fields.find((f) => f.key === 'name');
			if (name_loc && (named === undefined || named.bit !== -1 || s === -1 || e === -1)) {
				errors.push(`${name}: a \`name_loc\` without a required \`name\`, \`start\` and \`end\``);
			}
			drafts.push({
				name,
				type: item.type ?? null,
				interface: item.interface,
				fields,
				loc,
				name_loc
			});
		}
	}
	const canonical = (d: Draft): string =>
		JSON.stringify([
			d.type,
			d.fields.map((f) => [f.key, f.enc, f.bit, f.values ?? null, f.entry_interface ?? null])
		]);
	const by_canonical = new Map<string, number>();
	const draft_id: number[] = [];
	const unique: Draft[] = [];
	for (const d of drafts) {
		const key = canonical(d);
		const hit = by_canonical.get(key);
		if (hit !== undefined) {
			// the loc flags ride no tape byte, so twins that fold must agree on them
			const kept = unique[hit]!;
			if (kept.loc !== d.loc || kept.name_loc !== d.name_loc) {
				errors.push(`${d.name}: folds into ${kept.name} but differs in its loc-wire fields`);
			}
			draft_id.push(hit);
			continue;
		}
		by_canonical.set(key, unique.length);
		draft_id.push(unique.length);
		unique.push(d);
	}
	const by_interface = new Map<string, number>();
	drafts.forEach((d, i) => {
		if (d.type === null && d.interface !== undefined) by_interface.set(d.interface, draft_id[i]!);
	});
	if (unique.length > TAPE_MAX_ENTRIES) {
		errors.push(`${unique.length} tape entries, past ${TAPE_MAX_ENTRIES}`);
	}
	const entries: TapeEntry[] = unique.map((d, id) => ({
		id,
		name: d.name,
		type: d.type,
		bits: d.fields.filter((f) => f.bit !== -1).length,
		loc: d.loc,
		name_loc: d.name_loc,
		fields: d.fields.map(({ entry_interface, ...f }) => {
			if (entry_interface === undefined) return f;
			const target = by_interface.get(entry_interface);
			if (target === undefined) {
				errors.push(`${d.name}.${f.key}: ${entry_interface} is not a type-less entry`);
				return f;
			}
			return { ...f, entry: target };
		})
	}));
	const by_type = new Map<string, number[]>();
	const pre_type = new Map<string, EncCode>();
	for (const entry of entries) {
		if (entry.type === null) continue;
		const list = by_type.get(entry.type) ?? [];
		list.push(entry.id);
		by_type.set(entry.type, list);
		const at = entry.fields.findIndex((f) => f.key === 'type');
		if (at === -1 || entry.fields[at]!.enc !== Enc.type || entry.fields[at]!.bit !== -1) {
			errors.push(`${entry.name}: no required type field`);
			continue;
		}
		for (const f of entry.fields.slice(0, at)) {
			const known = pre_type.get(f.key);
			if (known !== undefined && known !== f.enc) {
				errors.push(`${entry.name}.${f.key}: a pre-type key with two encodings`);
			}
			if (f.enc === Enc.own || f.enc === Enc.keyword || f.entry !== undefined) {
				errors.push(`${entry.name}.${f.key}: a pre-type key whose encoding needs its entry`);
			}
			pre_type.set(f.key, f.enc);
		}
	}
	return errors.length > 0 ? { errors } : { entries, by_type, pre_type };
}

/** The most keys any typed entry carries ahead of its `type`: the transcoder's buffer for them. */
export const pre_type_max = (entries: ReadonlyArray<TapeEntry>): number =>
	Math.max(
		0,
		...entries
			.filter((e) => e.type !== null)
			.map((e) => e.fields.findIndex((f) => f.key === 'type'))
	);

/** The key a fast shape is found by. */
export const shape_key = (entry: number, mask: number): number => mask * TAPE_MAX_ENTRIES + entry;

/** An entry-generic record's header word, negative: `-(2 + (mask << 10 | entry))`. */
export const generic_header = (entry: number, mask: number): number =>
	-(2 + shape_key(entry, mask));

/** The entry and mask of an entry-generic header (one `generic_header` made). */
export function read_generic_header(header: number): { entry: number; mask: number } {
	const v = -header - 2;
	return { entry: v % TAPE_MAX_ENTRIES, mask: Math.floor(v / TAPE_MAX_ENTRIES) };
}

/** The canonical text `SHAPES_HASH` hashes: the format version, the entries, the shapes. */
export function tape_schema_canonical(
	entries: ReadonlyArray<TapeEntry>,
	shapes: ReadonlyArray<TapeShape>
): string {
	return JSON.stringify({
		version: TAPE_FORMAT_VERSION,
		entries: entries.map((e) => [
			e.type,
			e.fields.map((f) => [f.key, f.enc, f.bit, f.values ?? null, f.entry ?? null])
		]),
		shapes: shapes.map((s) => [s.entry, s.mask])
	});
}

/**
 * The schema the tape is written against: the table's tape entries, and as fast shapes every
 * entry's all-required mask (`0`) plus every pair the committed list names, sorted by entry then
 * mask and numbered from `1`.
 */
export function build_tape_schema(
	schema: AstSchema,
	shapes_file: TapeShapesFile
): TapeSchema | { errors: string[] } {
	const derived = derive_tape_entries(schema);
	if ('errors' in derived) return derived;
	const { entries, by_type, pre_type } = derived;
	const errors: string[] = [];
	const by_name = new Map(entries.map((e) => [e.name, e]));
	const pairs = new Set<string>(entries.map((e) => `${e.id}:0`));
	for (const [name, mask] of shapes_file.shapes) {
		const entry = by_name.get(name);
		if (entry === undefined) {
			errors.push(`shape list: ${name} names no tape entry`);
			continue;
		}
		if (!Number.isInteger(mask) || mask < 0 || mask >= 2 ** entry.bits) {
			errors.push(`shape list: ${name} mask ${mask} is outside its ${entry.bits} bits`);
			continue;
		}
		pairs.add(`${entry.id}:${mask}`);
	}
	if (errors.length > 0) return { errors };
	const sorted = [...pairs]
		.map((pair) => pair.split(':').map(Number) as [number, number])
		.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
	const shapes: TapeShape[] = sorted.map(([entry, mask], i) => ({ id: i + 1, entry, mask }));
	const shape_ids = new Map(shapes.map((s) => [shape_key(s.entry, s.mask), s.id]));
	return {
		entries,
		by_type,
		pre_type,
		shapes,
		shape_ids,
		hash: fnv1a32(tape_schema_canonical(entries, shapes))
	};
}

/**
 * The shape list a census of (entry, mask) pairs yields: every non-zero mask observed, by entry
 * then mask (a zero mask is a fast shape unlisted).
 */
export function shapes_file_from_census(
	entries: ReadonlyArray<TapeEntry>,
	observed: Iterable<readonly [entry: number, mask: number]>
): TapeShapesFile {
	const pairs = new Map<string, [number, number]>();
	for (const [entry, mask] of observed)
		if (mask !== 0) pairs.set(`${entry}:${mask}`, [entry, mask]);
	return {
		note: [
			'GENERATED by npm run tape-shapes:update: the presence masks the fixture wire carries, one',
			"fast shape each beside every entry's all-required mask. Never edited by hand."
		],
		shapes: [...pairs.values()]
			.sort((a, b) => a[0] - b[0] || a[1] - b[1])
			.map(([entry, mask]) => [entries[entry]!.name, mask])
	};
}

/** The shape list's canonical text: one shape per line. */
export function layout_tape_shapes(file: TapeShapesFile): string {
	const lines = ['{', '\t"note": ['];
	file.note.forEach((line, i) =>
		lines.push(`\t\t${JSON.stringify(line)}${i < file.note.length - 1 ? ',' : ''}`)
	);
	lines.push('\t],', '\t"shapes": [');
	file.shapes.forEach(([name, mask], i) =>
		lines.push(`\t\t[${JSON.stringify(name)}, ${mask}]${i < file.shapes.length - 1 ? ',' : ''}`)
	);
	lines.push('\t]', '}');
	return `${lines.join('\n')}\n`;
}

/** Reads the shape list's text, or what is wrong with it. */
export function parse_tape_shapes(text: string): TapeShapesFile | { errors: string[] } {
	let raw: unknown;
	try {
		raw = JSON.parse(text);
	} catch (error) {
		return { errors: [`not JSON: ${(error as Error).message}`] };
	}
	const r = raw as Partial<TapeShapesFile> | null;
	const ok =
		typeof r === 'object' &&
		r !== null &&
		Array.isArray(r.note) &&
		r.note.every((line) => typeof line === 'string') &&
		Array.isArray(r.shapes) &&
		r.shapes.every(
			(s) =>
				Array.isArray(s) && s.length === 2 && typeof s[0] === 'string' && typeof s[1] === 'number'
		);
	return ok ? (r as TapeShapesFile) : { errors: ['not {note: string[], shapes: [name, mask][]}'] };
}

/** The languages a hot-shape list ranks, each over its own documents. */
export const TAPE_HOT_LANGUAGES = ['typescript', 'svelte', 'css'] as const;

/** A language a hot-shape list ranks. */
export type TapeHotLanguage = (typeof TAPE_HOT_LANGUAGES)[number];

/**
 * How many of each language's hottest fast shapes get a `loc` twin in the decoder — the cut
 * `tape-hot:update` makes of each ranking. Chosen by measurement: past it a twin buys the
 * `{locations: true}` parse too little to pay for its bytes, which every `./parse` import loads.
 */
export const TAPE_HOT_TWINS: Readonly<Record<TapeHotLanguage, number>> = {
	typescript: 100,
	svelte: 100,
	css: 25
};

/**
 * The committed hot-shape list: per language, its hottest fast shapes (entry name and mask),
 * hottest first. The decoder's loc mode builds `loc` into these through a generated twin of each
 * one's fast function and reads every other record through its entry-generic loc reader, so the
 * list sizes the decoder, never what it decodes.
 */
export interface TapeHotFile {
	note: string[];
	typescript: Array<[entry: string, mask: number]>;
	svelte: Array<[entry: string, mask: number]>;
	css: Array<[entry: string, mask: number]>;
}

/**
 * The hot-shape list a census yields: per language, its fast shapes by how many records of its
 * documents took each (a tie by shape id), the first `TAPE_HOT_TWINS[language]` kept.
 */
export function hot_file_from_counts(
	schema: TapeSchema,
	counts: Readonly<Record<TapeHotLanguage, ReadonlyMap<number, number>>>
): TapeHotFile {
	const file: TapeHotFile = {
		note: [
			"GENERATED by npm run tape-hot:update: per language, the fast shapes the author's own",
			'corpus collections (held off the perf bench) decode most often, hottest first — the',
			"shapes the tape decoder's loc mode builds through a twin. Never edited by hand."
		],
		typescript: [],
		svelte: [],
		css: []
	};
	for (const language of TAPE_HOT_LANGUAGES) {
		const ranked = [...counts[language]]
			.filter(([, count]) => count > 0)
			.sort((a, b) => b[1] - a[1] || a[0] - b[0])
			.slice(0, TAPE_HOT_TWINS[language]);
		for (const [id] of ranked) {
			const shape = schema.shapes[id - 1]!;
			file[language].push([schema.entries[shape.entry]!.name, shape.mask]);
		}
	}
	return file;
}

/** The hot-shape list's canonical text: one shape per line. */
export function layout_tape_hot(file: TapeHotFile): string {
	const lines = ['{', '\t"note": ['];
	file.note.forEach((line, i) =>
		lines.push(`\t\t${JSON.stringify(line)}${i < file.note.length - 1 ? ',' : ''}`)
	);
	lines.push('\t],');
	TAPE_HOT_LANGUAGES.forEach((language, l) => {
		lines.push(`\t${JSON.stringify(language)}: [`);
		const list = file[language];
		list.forEach(([name, mask], i) =>
			lines.push(`\t\t[${JSON.stringify(name)}, ${mask}]${i < list.length - 1 ? ',' : ''}`)
		);
		lines.push(`\t]${l < TAPE_HOT_LANGUAGES.length - 1 ? ',' : ''}`);
	});
	lines.push('}');
	return `${lines.join('\n')}\n`;
}

/**
 * Where two hot-shape lists cut differently: per language, the shapes that entered `census`'s
 * cut and the shapes that left it, one line each — empty when every language keeps the same SET.
 * Rank order inside a cut is not compared: the decoder reads the list as a set
 * (`generate_tape_decoder` takes the union of the three), so a swap within a cut changes no byte
 * of it; a shape crossing one language's cut fails here even where the union keeps it.
 */
export function tape_hot_cut_changes(committed: TapeHotFile, census: TapeHotFile): string[] {
	const changes: string[] = [];
	const keys = (list: ReadonlyArray<readonly [string, number]>): Set<string> =>
		new Set(list.map(([name, mask]) => `${name} mask ${mask}`));
	for (const language of TAPE_HOT_LANGUAGES) {
		const before = keys(committed[language]);
		const after = keys(census[language]);
		const entered = [...after].filter((key) => !before.has(key)).sort();
		const left = [...before].filter((key) => !after.has(key)).sort();
		if (entered.length > 0) changes.push(`${language}: entered the cut: ${entered.join(', ')}`);
		if (left.length > 0) changes.push(`${language}: left the cut: ${left.join(', ')}`);
	}
	return changes;
}

/** Reads the hot-shape list's text, or what is wrong with it. */
export function parse_tape_hot(text: string): TapeHotFile | { errors: string[] } {
	let raw: unknown;
	try {
		raw = JSON.parse(text);
	} catch (error) {
		return { errors: [`not JSON: ${(error as Error).message}`] };
	}
	const r = raw as Partial<TapeHotFile> | null;
	const pairs = (list: unknown): boolean =>
		Array.isArray(list) &&
		list.every(
			(s) =>
				Array.isArray(s) && s.length === 2 && typeof s[0] === 'string' && typeof s[1] === 'number'
		);
	const ok =
		typeof r === 'object' &&
		r !== null &&
		Array.isArray(r.note) &&
		r.note.every((line) => typeof line === 'string') &&
		TAPE_HOT_LANGUAGES.every((language) => pairs(r[language]));
	return ok
		? (r as TapeHotFile)
		: { errors: ['not {note: string[], typescript | svelte | css: [name, mask][]}'] };
}

/**
 * The fast shape ids a hot-shape list names, over every language — or what it names that is no
 * fast shape of `schema` (a list older than the table or the shape list: `tape-hot:update`).
 */
export function hot_shape_ids(
	schema: TapeSchema,
	file: TapeHotFile
): Set<number> | { errors: string[] } {
	const by_name = new Map(schema.entries.map((e) => [e.name, e.id]));
	const ids = new Set<number>();
	const errors: string[] = [];
	for (const language of TAPE_HOT_LANGUAGES) {
		for (const [name, mask] of file[language]) {
			const entry = by_name.get(name);
			const id = entry === undefined ? undefined : schema.shape_ids.get(shape_key(entry, mask));
			if (id === undefined)
				errors.push(`hot list: ${language} names ${name} mask ${mask}, no fast shape`);
			else ids.add(id);
		}
	}
	return errors.length > 0 ? { errors } : ids;
}
