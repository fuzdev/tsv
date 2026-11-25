/**
 * The AST schema table, `crates/tsv_wasm/types/tsv_ast_schema.json`: its vocabulary, its reader,
 * and its canonical layout.
 *
 * The table is the one description of the wire JSON every parse export returns. It holds one
 * ENTRY per wire node type, per named variant of one (a node the same `type` names but whose key
 * set or field types differ by where it sits — the identifier Svelte builds for a shorthand
 * directive is not acorn's), and per `type`-less object (`loc`, a template element's `value`),
 * each listing its FIELDS in wire order. The type-level declarations the `.d.ts` adds around
 * them (unions such as `Expression`, the comment-attachment mixin) are ALIAS entries and a
 * `mixin` entry beside them. The vocabulary below is what an entry and a field may say; the
 * table's own `note` and `crates/tsv_wasm/CLAUDE.md` §TS Type Maintenance say what each means.
 *
 * The `.d.ts` beside it, `tsv_ast.d.ts`, is generated from it (`ast_types_gen.ts`): the table is
 * what a writer change edits, and the published declarations follow.
 *
 * The layout is this module's: `layout_ast_schema` writes the table in one canonical form
 * (key order fixed per object kind, a field on one line when it fits, doc text of more than one
 * line one string per line), and `npm run check:ast-schema` fails when the committed bytes are
 * not that form, or the committed `.d.ts` is not what the table generates — so a hand edit in
 * any layout reads back in this one after `npm run ast-schema:update`, and no general JSON
 * formatter (the repo has none, and Prettier must never run on it) is asked to agree with it.
 *
 * `JSON.parse` collapses a duplicated key to its last occurrence, so the reader scans the text
 * for one first (`duplicate_keys`) and refuses it, in the check and the rewrite alike; the
 * wire's own duplicate check is the Rust validator's (`tsv_debug schema_audit`), which reads
 * the wire key by key.
 *
 * Its reach: the checkout's `crates/tsv_wasm/types/`, read always and written under
 * `--update` (`AstSchemaDeps`, built by `scripts/roots/ast_schema.ts`).
 */

import type { FsDir } from '@fuzdev/gro/fs.ts';

import { AST_TYPES_PATH, generate_ast_types } from './ast_types_gen.ts';

/** The table's path, from the checkout root. */
export const AST_SCHEMA_PATH = 'crates/tsv_wasm/types/tsv_ast_schema.json';

/** What a field's value is on the wire; a field that takes several lists each. */
export type FieldKind =
	| 'type'
	| 'offset'
	| 'loc'
	| 'boolean'
	| 'number'
	| 'string'
	| 'strings'
	| 'child'
	| 'list'
	| 'object'
	| 'comments'
	| 'data'
	| 'null';

const FIELD_KINDS: ReadonlySet<string> = new Set<FieldKind>([
	'type',
	'offset',
	'loc',
	'boolean',
	'number',
	'string',
	'strings',
	'child',
	'list',
	'object',
	'comments',
	'data',
	'null'
]);

/** When a field is on the wire, in the dialect it describes. */
export type Presence = 'always' | 'optional' | 'locations' | 'absent';

const PRESENCES: ReadonlySet<string> = new Set<Presence>([
	'always',
	'optional',
	'locations',
	'absent'
]);

/** Where a string field's text comes from. */
export type StringClass = 'own' | 'slice' | 'cooked' | 'keyword' | 'pooled';

const STRING_CLASSES: ReadonlySet<string> = new Set<StringClass>([
	'own',
	'slice',
	'cooked',
	'keyword',
	'pooled'
]);

/** One key of an entry, in wire order. */
export interface AstSchemaField {
	key: string;
	kind: FieldKind | FieldKind[];
	/** The `.d.ts` member type; absent on a field the `.d.ts` declares elsewhere (the attached
	 * comment keys, through `AcornCommentAttachment`) or not at all (an opaque entry's). */
	ts?: string;
	/** The `type`-less entry an `object` or `loc` field holds, by its `interface`. */
	entry?: string;
	/** Default `always`. */
	presence?: Presence;
	/** The presence under vanilla acorn (a Svelte component without `lang="ts"`) where it
	 * differs from `presence`, which describes acorn-typescript. */
	vanilla_acorn?: Presence;
	nullable?: boolean;
	/** A `list` whose items may be `null` (an array hole). */
	items_nullable?: boolean;
	string?: StringClass;
	/** A `keyword` string's closed vocabulary, every spelling the writer emits. */
	values?: string[];
	/** A key Svelte's `remove_typescript_nodes` deletes before any visitor walks the tree. */
	ts_only?: boolean;
	/** A `loc` whose positions may carry `character`. */
	character?: boolean;
	/** For a `data` field: the `type` spellings its value holds as data, not as nodes. */
	discriminators?: string[];
	doc?: string[];
	note?: string[];
}

/** A wire node type, a named variant of one, a `type`-less object, or the comment mixin. */
export interface AstSchemaEntry {
	/** The wire discriminator; absent on a `type`-less object and on the mixin. */
	type?: string;
	/** Names an entry the same `type` already has a base entry for. */
	variant?: string;
	/** The `.d.ts` interface it generates; entries sharing one merge into it. */
	interface?: string;
	/** A node of the CSS tree (below a `StyleSheet` or `StyleSheetFile`, its `attributes` aside). */
	css?: boolean;
	/** An interface no wire object is graded against: its fields are another entry's by `extends`. */
	mixin?: boolean;
	/** The `.d.ts` leaves it undeclared (the region above it types it `unknown`). */
	opaque?: boolean;
	/** The parent slots (`Type.key`) it may stand in, where its interface alone admits more. */
	positions?: string[];
	extends?: string[];
	doc?: string[];
	note?: string[];
	fields: AstSchemaField[];
}

/** A `.d.ts` union: `export type <alias> = <union> [& <intersect>]`. */
export interface AstSchemaAlias {
	alias: string;
	doc?: string[];
	note?: string[];
	union: string[];
	intersect?: string;
}

export interface AstSchemaSection {
	title: string;
	note?: string[];
	entries: Array<AstSchemaEntry | AstSchemaAlias>;
}

export interface AstSchema {
	/** The generated `.d.ts`'s file header. */
	doc: string[];
	note?: string[];
	sections: AstSchemaSection[];
}

/** Whether `item` is an alias entry. */
export const is_alias = (item: AstSchemaEntry | AstSchemaAlias): item is AstSchemaAlias =>
	'alias' in item;

/** The keys each object kind may carry, in the canonical layout's order. */
const TOP_KEYS = ['doc', 'note', 'sections'];
const SECTION_KEYS = ['title', 'note', 'entries'];
const ENTRY_KEYS = [
	'type',
	'variant',
	'interface',
	'css',
	'mixin',
	'opaque',
	'positions',
	'extends',
	'doc',
	'note',
	'fields'
];
const ALIAS_KEYS = ['alias', 'doc', 'note', 'union', 'intersect'];
const FIELD_KEYS = [
	'key',
	'kind',
	'ts',
	'entry',
	'presence',
	'vanilla_acorn',
	'nullable',
	'items_nullable',
	'string',
	'values',
	'ts_only',
	'character',
	'discriminators',
	'doc',
	'note'
];

const is_record = (v: unknown): v is Record<string, unknown> =>
	typeof v === 'object' && v !== null && !Array.isArray(v);
const is_lines = (v: unknown): v is string[] =>
	Array.isArray(v) && v.every((line) => typeof line === 'string');

/**
 * Reads the table's text into an `AstSchema`, or the list of what is wrong with it: an unknown
 * key, a value outside its vocabulary, a missing required key, a duplicated identity.
 * Field-level rules the wire decides (does the writer emit it, in this order) are the Rust
 * validator's, not this reader's.
 */
export function parse_ast_schema(text: string): AstSchema | { errors: string[] } {
	let raw: unknown;
	try {
		raw = JSON.parse(text);
	} catch (error) {
		return { errors: [`not JSON: ${(error as Error).message}`] };
	}
	const errors: string[] = duplicate_keys(text).map(
		({ key, line }) => `line ${line}: key ${JSON.stringify(key)} twice in one object`
	);
	const keys = (where: string, obj: Record<string, unknown>, allowed: string[]): void => {
		for (const key of Object.keys(obj)) {
			if (!allowed.includes(key)) errors.push(`${where}: unknown key ${JSON.stringify(key)}`);
		}
	};
	const lines = (where: string, obj: Record<string, unknown>, key: string): void => {
		if (key in obj && !is_lines(obj[key]))
			errors.push(`${where}: \`${key}\` is not an array of strings`);
	};
	const flag = (where: string, obj: Record<string, unknown>, key: string): void => {
		if (key in obj && obj[key] !== true) {
			errors.push(`${where}: \`${key}\` is written \`true\` or left out`);
		}
	};
	if (!is_record(raw)) return { errors: ['the table is not an object'] };
	keys('table', raw, TOP_KEYS);
	if (!is_lines(raw.doc)) errors.push('table: `doc` is not an array of strings');
	lines('table', raw, 'note');
	if (!Array.isArray(raw.sections)) return { errors: [...errors, 'table: no `sections` array'] };
	const identities = new Set<string>();
	const interfaces = new Set<string>();
	const aliases = new Set<string>();
	for (const [si, section] of raw.sections.entries()) {
		const sw = `sections[${si}]`;
		if (!is_record(section)) {
			errors.push(`${sw}: not an object`);
			continue;
		}
		keys(sw, section, SECTION_KEYS);
		if (typeof section.title !== 'string') errors.push(`${sw}: no \`title\``);
		lines(sw, section, 'note');
		if (!Array.isArray(section.entries)) {
			errors.push(`${sw}: no \`entries\` array`);
			continue;
		}
		for (const [ei, entry] of section.entries.entries()) {
			const ew = `${sw}.entries[${ei}]`;
			if (!is_record(entry)) {
				errors.push(`${ew}: not an object`);
				continue;
			}
			if ('alias' in entry) {
				keys(ew, entry, ALIAS_KEYS);
				if (typeof entry.alias !== 'string') errors.push(`${ew}: \`alias\` is not a string`);
				else if (aliases.has(entry.alias)) errors.push(`${ew}: alias ${entry.alias} twice`);
				else aliases.add(entry.alias);
				lines(ew, entry, 'doc');
				lines(ew, entry, 'note');
				if (!is_lines(entry.union) || entry.union.length === 0) {
					errors.push(`${ew}: \`union\` is not a non-empty array of strings`);
				}
				if ('intersect' in entry && typeof entry.intersect !== 'string') {
					errors.push(`${ew}: \`intersect\` is not a string`);
				}
				continue;
			}
			keys(ew, entry, ENTRY_KEYS);
			for (const key of ['type', 'variant', 'interface']) {
				if (key in entry && typeof entry[key] !== 'string')
					errors.push(`${ew}: \`${key}\` is not a string`);
			}
			for (const key of ['css', 'mixin', 'opaque']) flag(ew, entry, key);
			lines(ew, entry, 'extends');
			lines(ew, entry, 'positions');
			lines(ew, entry, 'doc');
			lines(ew, entry, 'note');
			if (entry.type === undefined && entry.interface === undefined) {
				errors.push(`${ew}: neither \`type\` nor \`interface\``);
			}
			if (entry.variant !== undefined && entry.type === undefined) {
				errors.push(`${ew}: a \`variant\` with no \`type\``);
			}
			const identity =
				entry.type !== undefined
					? `${entry.css ? 'css:' : ''}${String(entry.type)}${entry.variant ? `/${String(entry.variant)}` : ''}`
					: `~${String(entry.interface)}`;
			if (identities.has(identity)) errors.push(`${ew}: ${identity} has two entries`);
			identities.add(identity);
			if (typeof entry.interface === 'string') interfaces.add(entry.interface);
			if (!Array.isArray(entry.fields)) {
				errors.push(`${ew}: no \`fields\` array`);
				continue;
			}
			const field_keys = new Set<string>();
			for (const [fi, field] of entry.fields.entries()) {
				const fw = `${ew} (${identity}).fields[${fi}]`;
				if (!is_record(field)) {
					errors.push(`${fw}: not an object`);
					continue;
				}
				keys(fw, field, FIELD_KEYS);
				if (typeof field.key !== 'string') errors.push(`${fw}: no \`key\``);
				else if (field_keys.has(field.key)) errors.push(`${fw}: key ${field.key} twice`);
				else field_keys.add(field.key);
				const kinds = Array.isArray(field.kind) ? field.kind : [field.kind];
				if (kinds.length === 0 || kinds.some((k) => typeof k !== 'string' || !FIELD_KINDS.has(k))) {
					errors.push(`${fw}: \`kind\` ${JSON.stringify(field.kind)} is outside the vocabulary`);
				}
				for (const key of ['ts', 'entry']) {
					if (key in field && typeof field[key] !== 'string')
						errors.push(`${fw}: \`${key}\` is not a string`);
				}
				for (const key of ['presence', 'vanilla_acorn']) {
					if (key in field && !PRESENCES.has(field[key] as string)) {
						errors.push(
							`${fw}: \`${key}\` ${JSON.stringify(field[key])} is outside the vocabulary`
						);
					}
				}
				if (field.presence === 'always')
					errors.push(`${fw}: \`presence: always\` is the default — leave it out`);
				if ('string' in field && !STRING_CLASSES.has(field.string as string)) {
					errors.push(
						`${fw}: \`string\` ${JSON.stringify(field.string)} is outside the vocabulary`
					);
				}
				if (kinds.includes('string') !== 'string' in field) {
					errors.push(`${fw}: a \`string\` kind and a \`string\` class come together`);
				}
				for (const key of ['nullable', 'items_nullable', 'ts_only', 'character'])
					flag(fw, field, key);
				lines(fw, field, 'values');
				if ((field.string === 'keyword') !== 'values' in field) {
					errors.push(`${fw}: a \`keyword\` class and its \`values\` come together`);
				}
				lines(fw, field, 'discriminators');
				lines(fw, field, 'doc');
				lines(fw, field, 'note');
			}
		}
	}
	// every `entry` reference names a `type`-less entry's interface
	for (const section of raw.sections as Array<Record<string, unknown>>) {
		for (const entry of (section.entries ?? []) as Array<Record<string, unknown>>) {
			for (const field of (entry.fields ?? []) as Array<Record<string, unknown>>) {
				if (typeof field.entry === 'string' && !interfaces.has(field.entry)) {
					errors.push(
						`${String(entry.type ?? entry.interface)}.${String(field.key)}: \`entry\` ${field.entry} names no entry`
					);
				}
			}
		}
	}
	return errors.length > 0 ? { errors } : (raw as unknown as AstSchema);
}

/**
 * Every key a JSON text repeats within one object, with the line of its second spelling —
 * which `JSON.parse` would silently collapse to the last one, so neither the check nor the
 * rewrite could see it. Runs on text `JSON.parse` has already accepted.
 */
export function duplicate_keys(text: string): Array<{ key: string; line: number }> {
	const out: Array<{ key: string; line: number }> = [];
	// one frame per open container: the keys seen, or `null` for an array
	const frames: Array<Set<string> | null> = [];
	let line = 1;
	let expect_key = false;
	for (let i = 0; i < text.length; i++) {
		const c = text[i]!;
		if (c === '\n') line++;
		else if (c === '{') {
			frames.push(new Set());
			expect_key = true;
		} else if (c === '[') frames.push(null);
		else if (c === '}' || c === ']') frames.pop();
		else if (c === ',') expect_key = frames.at(-1) instanceof Set;
		else if (c === '"') {
			let j = i + 1;
			while (text[j] !== '"') j += text[j] === '\\' ? 2 : 1;
			const frame = frames.at(-1);
			if (expect_key && frame) {
				const key = JSON.parse(text.slice(i, j + 1)) as string;
				if (frame.has(key)) out.push({ key, line });
				frame.add(key);
				expect_key = false;
			}
			i = j;
		}
	}
	return out;
}

/** The widest line the layout puts a value on, a tab counting two columns. */
const LAYOUT_WIDTH = 100;

const width = (line: string): number => line.length + (line.match(/\t/g)?.length ?? 0);

/** Keys whose string arrays always print one string per line. */
const LINE_KEYS: ReadonlySet<string> = new Set(['doc', 'note']);

function ordered(obj: Record<string, unknown>, order: string[]): Array<[string, unknown]> {
	return order.filter((key) => key in obj).map((key) => [key, obj[key]]);
}

function inline(value: unknown, order_of: (obj: Record<string, unknown>) => string[]): string {
	if (Array.isArray(value)) return `[${value.map((v) => inline(v, order_of)).join(', ')}]`;
	if (is_record(value)) {
		const parts = ordered(value, order_of(value)).map(
			([k, v]) => `${JSON.stringify(k)}: ${inline(v, order_of)}`
		);
		return `{${parts.join(', ')}}`;
	}
	return JSON.stringify(value);
}

/**
 * The table in canonical layout: tab-indented; keys in the fixed order above; a field, and any
 * array of short strings, on one line when it fits `LAYOUT_WIDTH` and holds no doc or note;
 * doc and note text one string per line.
 */
export function layout_ast_schema(schema: AstSchema): string {
	const order_of = (obj: Record<string, unknown>): string[] => {
		if ('sections' in obj) return TOP_KEYS;
		if ('entries' in obj) return SECTION_KEYS;
		if ('alias' in obj) return ALIAS_KEYS;
		if ('fields' in obj) return ENTRY_KEYS;
		return FIELD_KEYS;
	};
	const out: string[] = [];
	const write = (
		value: unknown,
		indent: string,
		prefix: string,
		suffix: string,
		key: string | null
	): void => {
		if (Array.isArray(value)) {
			const one = `${indent}${prefix}${inline(value, order_of)}${suffix}`;
			const strings = value.every((v) => typeof v === 'string');
			const short = !(key !== null && LINE_KEYS.has(key)) || value.length === 1;
			if (value.length === 0 || (strings && short && width(one) <= LAYOUT_WIDTH)) {
				out.push(one);
				return;
			}
			out.push(`${indent}${prefix}[`);
			value.forEach((v, i) => write(v, `${indent}\t`, '', i < value.length - 1 ? ',' : '', null));
			out.push(`${indent}]${suffix}`);
			return;
		}
		if (is_record(value)) {
			const order = order_of(value);
			const is_field = order === FIELD_KEYS;
			if (is_field && !('doc' in value) && !('note' in value)) {
				const one = `${indent}${prefix}${inline(value, order_of)}${suffix}`;
				if (width(one) <= LAYOUT_WIDTH) {
					out.push(one);
					return;
				}
			}
			const entries = ordered(value, order);
			out.push(`${indent}${prefix}{`);
			entries.forEach(([k, v], i) =>
				write(v, `${indent}\t`, `${JSON.stringify(k)}: `, i < entries.length - 1 ? ',' : '', k)
			);
			out.push(`${indent}}${suffix}`);
			return;
		}
		out.push(`${indent}${prefix}${JSON.stringify(value)}${suffix}`);
	};
	write(schema, '', '', '', null);
	return `${out.join('\n')}\n`;
}

/** What `run_ast_schema` reaches. */
export interface AstSchemaDeps {
	/** The checkout's `crates/tsv_wasm/types/`, holding the table and the `.d.ts`. */
	types: FsDir;
}

const TABLE_FILE = 'tsv_ast_schema.json';
const TYPES_FILE = 'tsv_ast.d.ts';

/**
 * Checks the table reads, sits in canonical layout, and generates the committed `.d.ts` — or
 * with `update`, rewrites the two files that drifted.
 * @returns the exit code: `1` when the table does not read or generate, or either file drifted
 * without `update`, else `0`
 */
export async function run_ast_schema(deps: AstSchemaDeps, update: boolean): Promise<number> {
	const text = await deps.types.read_text(TABLE_FILE);
	const schema = parse_ast_schema(text);
	if ('errors' in schema) {
		console.error(`${AST_SCHEMA_PATH} does not read:`);
		for (const error of schema.errors) console.error(`  ${error}`);
		return 1;
	}
	const types = generate_ast_types(schema);
	if (typeof types !== 'string') {
		console.error(`${AST_SCHEMA_PATH} does not generate ${AST_TYPES_PATH}:`);
		for (const error of types.errors) console.error(`  ${error}`);
		return 1;
	}
	const outputs: Array<[file: string, path: string, wanted: string]> = [
		[TABLE_FILE, AST_SCHEMA_PATH, layout_ast_schema(schema)],
		[TYPES_FILE, AST_TYPES_PATH, types]
	];
	let drifted = 0;
	for (const [file, path, wanted] of outputs) {
		const committed = await deps.types.read_text_if_exists(file);
		if (committed === wanted) continue;
		if (update) {
			await deps.types.write(file, wanted);
			console.log(`${path}: rewritten`);
		} else {
			console.error(`${path} is not what the table generates — run \`npm run ast-schema:update\`.`);
			drifted++;
		}
	}
	if (drifted > 0) return 1;
	console.log(`${AST_SCHEMA_PATH}: in canonical layout; ${AST_TYPES_PATH}: generated`);
	return 0;
}
