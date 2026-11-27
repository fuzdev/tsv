/**
 * The offline tape encoder: a wire tree (what `JSON.parse` makes of a parse export's JSON) and
 * its source to the tape `docs/tape_format.md` specifies, byte for byte the tape the Rust
 * transcoder (`tsv_debug::transcoder`, the oracle) makes from the same wire's JSON text.
 *
 * It is the format's executable reference on the JS side: `check:tape` encodes every fixture
 * document's wire with it, decodes the tape with the generated decoder, and compares the result
 * with the tree, then compares its bytes with the transcoder's. Nothing ships it.
 *
 * Every rule a choice rests on is the format doc's, in the order the doc gives them: a record
 * tries its candidate entries in table order and takes the first whose fields admit its keys
 * and whose encodings accept its values, else it is written self-describing; a string ref is a
 * source slice when the text occurs inside the record's scope, else a pool entry, resolved when
 * the record closes.
 *
 * @module
 */

import {
	Enc,
	generic_header,
	shape_key,
	Tag,
	TAPE_FORMAT_VERSION,
	TAPE_HEADER_WORDS,
	TAPE_MAGIC,
	TAPE_NULL_REF,
	type TapeEntry,
	type TapeSchema
} from './tape_schema.ts';

/** How many records of each kind an encode wrote. */
export interface TapeStats {
	fast: number;
	generic: number;
	escape: number;
}

/** What `encode_tape` takes beyond the tree and source. */
export interface TapeEncodeOptions {
	/** Called with each typed record's entry and mask (the shape census). */
	census?: (entry: number, mask: number) => void;
	/** Write every typed record entry-generic, no fast shape (the decoder's slow-path gate). */
	no_fast_shapes?: boolean;
	/** Called with each object written self-describing. */
	on_escape?: (object: Record<string, unknown>) => void;
}

/** A tape and its record counts. */
export interface EncodedTape {
	bytes: Uint8Array;
	stats: TapeStats;
}

type Scope = readonly [start: number, end: number] | null;

/** A string ref waiting for its record to close: its slot and its text. */
interface Pending {
	slot: number;
	text: string;
}

const LONE_SURROGATE = /[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/;

const is_record_value = (v: unknown): v is Record<string, unknown> =>
	typeof v === 'object' && v !== null && !Array.isArray(v);

const is_u32 = (v: unknown): v is number =>
	typeof v === 'number' && Number.isInteger(v) && v >= 0 && v <= 0xffffffff && !Object.is(v, -0);

/**
 * Encodes `tree`, a span-only wire tree, against `schema`.
 * @throws Error when a string holds a lone surrogate (the pool is UTF-8) or the tape outgrows
 * its words
 */
export function encode_tape(
	tree: unknown,
	source: string,
	schema: TapeSchema,
	options: TapeEncodeOptions = {}
): EncodedTape {
	const words: number[] = new Array(TAPE_HEADER_WORDS).fill(0);
	let pool = '';
	const pending: Pending[] = [];
	const stats: TapeStats = { fast: 0, generic: 0, escape: 0 };
	const length = source.length;

	const is_high = (c: number): boolean => c >= 0xd800 && c <= 0xdbff;
	const is_low = (c: number): boolean => c >= 0xdc00 && c <= 0xdfff;
	const splits_pair = (i: number): boolean =>
		i > 0 && i < length && is_high(source.charCodeAt(i - 1)) && is_low(source.charCodeAt(i));

	/** The record's own span: integer `start` ≤ `end` ≤ the source's length, splitting no pair. */
	const own_span = (o: Record<string, unknown>): Scope => {
		const s = o.start;
		const e = o.end;
		if (!is_u32(s) || !is_u32(e) || s > e || e > length || splits_pair(s) || splits_pair(e)) {
			return null;
		}
		return [s, e];
	};

	const check_text = (text: string): void => {
		if (LONE_SURROGATE.test(text)) {
			throw new Error(
				`a string holding a lone surrogate cannot be written: ${JSON.stringify(text)}`
			);
		}
	};

	const ref = (text: string): void => {
		check_text(text);
		pending.push({ slot: words.length, text });
		words.push(0, 0);
	};

	const f64 = new Float64Array(1);
	const f64_words = new Uint32Array(f64.buffer);
	const number = (n: number): void => {
		f64[0] = n;
		words.push(f64_words[0]!, f64_words[1]!);
	};

	/** Resolves the refs a record left pending since `mark`, within `scope`. */
	const resolve = (mark: number, scope: Scope): void => {
		for (let k = mark; k < pending.length; k++) {
			const { slot, text } = pending[k]!;
			let at = -1;
			if (scope !== null) {
				if (text === '') at = scope[0];
				else {
					const found = source.substring(scope[0], scope[1]).indexOf(text);
					if (found !== -1) at = scope[0] + found;
				}
			}
			if (at !== -1) {
				words[slot] = at;
				words[slot + 1] = at + text.length;
			} else {
				words[slot] = -1 - pool.length;
				pool += text;
				words[slot + 1] = pool.length;
			}
		}
		pending.length = mark;
	};

	/** Whether `v` suits encoding `enc` in record `o` (its own span `span`). */
	const accepts = (
		enc: number,
		v: unknown,
		values: ReadonlyArray<string> | undefined,
		o: Record<string, unknown>,
		span: Scope
	): boolean => {
		switch (enc) {
			case Enc.type:
				return false; // matched by the candidate's type, never here
			case Enc.offset:
				return is_u32(v);
			case Enc.boolean:
				return typeof v === 'boolean';
			case Enc.number:
				return typeof v === 'number';
			case Enc.null:
				return v === null;
			case Enc.own:
				return typeof v === 'string' && span !== null && source.slice(span[0], span[1]) === v;
			case Enc.keyword:
				return typeof v === 'string' && values!.includes(v);
			case Enc.ref:
				return typeof v === 'string';
			case Enc.ref_nullable:
				return v === null || typeof v === 'string';
			case Enc.strings:
				return Array.isArray(v) && v.every((x) => typeof x === 'string');
			case Enc.record:
				return v === null || is_record_value(v);
			case Enc.list:
				return Array.isArray(v) && v.every((x) => x === null || is_record_value(x));
			default:
				return true; // a tagged value takes anything
		}
	};

	/** The mask `o` has under `entry`, or `-1` when the entry cannot hold it. */
	const fit = (o: Record<string, unknown>, keys: string[], entry: TapeEntry): number => {
		const span = own_span(o);
		let cursor = 0;
		let mask = 0;
		const fields = entry.fields;
		for (const key of keys) {
			while (cursor < fields.length && fields[cursor]!.key !== key) {
				if (fields[cursor]!.bit === -1) return -1;
				cursor++;
			}
			if (cursor === fields.length) return -1;
			const field = fields[cursor]!;
			if (field.enc === Enc.type) {
				if (o[key] !== entry.type) return -1;
			} else if (!accepts(field.enc, o[key], field.values, o, span)) return -1;
			if (field.bit !== -1) mask |= 1 << field.bit;
			cursor++;
		}
		for (; cursor < fields.length; cursor++) if (fields[cursor]!.bit === -1) return -1;
		return mask;
	};

	/** The scope a value at key index `i` inherits: the own span once both its keys came first. */
	const child_scope = (keys: string[], i: number, span: Scope, scope: Scope): Scope =>
		span !== null && keys.indexOf('start') < i && keys.indexOf('end') < i ? span : scope;

	// The walk is a stack of tasks, never a recursion, so a tree as deep as the parsers accept
	// encodes on any stack. A record's fields are one task that suspends at each nested value and
	// resumes after it, so its refs still resolve after every nested record's, as a recursion
	// would order them.
	type Task =
		| { t: 'record'; v: unknown; context: number | null; scope: Scope }
		| { t: 'value'; v: unknown; scope: Scope }
		| {
				t: 'fields';
				o: Record<string, unknown>;
				keys: string[];
				i: number;
				entry: TapeEntry | null;
				cursor: number;
				span: Scope;
				scope: Scope;
				mark: number;
		  }
		| { t: 'items'; items: unknown[]; i: number; scope: Scope; records: boolean };
	const stack: Task[] = [{ t: 'record', v: tree, context: null, scope: [0, length] }];

	/** Starts a record: its header, then its fields as a task. */
	const open_record = (v: unknown, context: number | null, scope: Scope): void => {
		if (v === null) {
			words.push(0);
			return;
		}
		const o = v as Record<string, unknown>;
		const keys = Object.keys(o);
		const span = own_span(o);
		const candidates =
			context !== null
				? [context]
				: typeof o.type === 'string'
					? (schema.by_type.get(o.type) ?? [])
					: [];
		for (const id of candidates) {
			const entry = schema.entries[id]!;
			const mask = fit(o, keys, entry);
			if (mask === -1) continue;
			options.census?.(entry.id, mask);
			const shape = options.no_fast_shapes
				? undefined
				: schema.shape_ids.get(shape_key(entry.id, mask));
			if (shape === undefined) {
				stats.generic++;
				words.push(generic_header(entry.id, mask));
			} else {
				stats.fast++;
				words.push(shape);
			}
			stack.push({
				t: 'fields',
				o,
				keys,
				i: 0,
				entry,
				cursor: 0,
				span,
				scope,
				mark: pending.length
			});
			return;
		}
		stats.escape++;
		options.on_escape?.(o);
		words.push(-1, keys.length);
		stack.push({
			t: 'fields',
			o,
			keys,
			i: 0,
			entry: null,
			cursor: 0,
			span,
			scope,
			mark: pending.length
		});
	};

	/** Starts a tagged value: a scalar written whole, an array or object as a task. */
	const open_value = (v: unknown, scope: Scope): void => {
		if (v === null) words.push(Tag.null);
		else if (v === false) words.push(Tag.false);
		else if (v === true) words.push(Tag.true);
		else if (typeof v === 'number') {
			words.push(Tag.number);
			number(v);
		} else if (typeof v === 'string') {
			words.push(Tag.string);
			ref(v);
		} else if (Array.isArray(v)) {
			words.push(Tag.array, v.length);
			stack.push({ t: 'items', items: v, i: 0, scope, records: false });
		} else if (is_record_value(v)) {
			words.push(Tag.object);
			stack.push({ t: 'record', v, context: null, scope });
		} else {
			throw new Error(`not a JSON value: ${String(v)}`);
		}
	};

	/** Writes a record's fields from `task.i` until one holds a nested value, or the record ends. */
	const resume_fields = (task: Extract<Task, { t: 'fields' }>): void => {
		const { o, keys, entry, span, scope } = task;
		while (task.i < keys.length) {
			const i = task.i++;
			const key = keys[i]!;
			const v = o[key];
			const inner = child_scope(keys, i, span, scope);
			if (entry === null) {
				// self-describing: the key's text, then the value tagged
				ref(key);
				stack.push(task, { t: 'value', v, scope: inner });
				return;
			}
			while (entry.fields[task.cursor]!.key !== key) task.cursor++;
			const field = entry.fields[task.cursor++]!;
			switch (field.enc) {
				case Enc.type:
				case Enc.null:
				case Enc.own:
					break;
				case Enc.offset:
					words.push(v as number);
					break;
				case Enc.boolean:
					words.push(v ? 1 : 0);
					break;
				case Enc.number:
					number(v as number);
					break;
				case Enc.keyword:
					words.push(field.values!.indexOf(v as string));
					break;
				case Enc.ref:
					ref(v as string);
					break;
				case Enc.ref_nullable:
					if (v === null) words.push(TAPE_NULL_REF, 0);
					else ref(v as string);
					break;
				case Enc.strings:
					words.push((v as string[]).length);
					for (const text of v as string[]) ref(text);
					break;
				case Enc.record:
					stack.push(task, { t: 'record', v, context: field.entry ?? null, scope: inner });
					return;
				case Enc.list:
					words.push((v as unknown[]).length);
					stack.push(task, {
						t: 'items',
						items: v as unknown[],
						i: 0,
						scope: inner,
						records: true
					});
					return;
				default:
					stack.push(task, { t: 'value', v, scope: inner });
					return;
			}
		}
		resolve(task.mark, span ?? scope);
	};

	while (stack.length > 0) {
		const task = stack.pop()!;
		switch (task.t) {
			case 'record':
				open_record(task.v, task.context, task.scope);
				break;
			case 'value':
				open_value(task.v, task.scope);
				break;
			case 'fields':
				resume_fields(task);
				break;
			case 'items':
				if (task.i < task.items.length) {
					const item = task.items[task.i++];
					stack.push(
						task,
						task.records
							? { t: 'record', v: item, context: null, scope: task.scope }
							: { t: 'value', v: item, scope: task.scope }
					);
				}
				break;
		}
	}
	if (words.length > 0xffffffff) throw new Error('the tape outgrew its 32-bit word count');
	const pool_bytes = new TextEncoder().encode(pool);
	const body = words.length - TAPE_HEADER_WORDS;
	words[0] = TAPE_MAGIC;
	words[1] = TAPE_FORMAT_VERSION;
	words[2] = schema.hash;
	words[3] = body;
	words[4] = pool_bytes.length;
	words[5] = 0;
	words[6] = pool.length;
	words[7] = length;
	const bytes = new Uint8Array(words.length * 4 + pool_bytes.length);
	const view = new DataView(bytes.buffer);
	words.forEach((word, i) => view.setUint32(i * 4, word >>> 0, true));
	bytes.set(pool_bytes, words.length * 4);
	return { bytes, stats };
}
