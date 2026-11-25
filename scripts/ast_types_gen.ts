/**
 * `crates/tsv_wasm/types/tsv_ast.d.ts`, generated from the AST schema table (`ast_schema.ts`).
 *
 * The `.d.ts` is output: an edit to the wire's description is an edit to the table, and
 * `npm run ast-schema:update` regenerates this file from it, which `npm run check:ast-schema`
 * holds byte-for-byte to the committed one. Members are listed in WIRE order, the table's field
 * order. Entries sharing an `interface` merge into one declaration — the `type` member becomes
 * the union of their discriminators, a member some entry lacks or emits only conditionally
 * becomes optional (`T?`), and one emitted with a `null` fallback keeps the `T | null` its
 * `ts` spells. An `opaque` entry generates nothing (the region above it types it
 * `unknown`); a field with no `ts` is declared elsewhere (the attached-comment keys, through
 * `AcornCommentAttachment`).
 *
 * The text is laid out the way `tsv format` lays it out, so `format:audit` holds the generated
 * file to the formatter's fixed point as it does every other TypeScript file in the checkout.
 */

import {
	type AstSchema,
	type AstSchemaAlias,
	type AstSchemaEntry,
	type AstSchemaField,
	is_alias
} from './ast_schema.ts';

/** The `.d.ts`'s path, from the checkout root. */
export const AST_TYPES_PATH = 'crates/tsv_wasm/types/tsv_ast.d.ts';

/** `tsv format`'s print width, a tab counting two columns. */
const PRINT_WIDTH = 100;

const width = (line: string): number => line.length + (line.match(/\t/g)?.length ?? 0);

function doc_block(lines: string[] | undefined, indent: string): string[] {
	if (!lines || lines.length === 0) return [];
	if (lines.length === 1) return [`${indent}/** ${lines[0]} */`];
	return [
		`${indent}/**`,
		...lines.map((line) => (line === '' ? `${indent} *` : `${indent} * ${line}`)),
		`${indent} */`
	];
}

/** A field's member type: its `ts`, or the `type`-less entry an `object` or `loc` field holds. */
const member_ts = (field: AstSchemaField): string | undefined => field.ts ?? field.entry;

/** A member is optional unless every merged entry carries it, emitted on every wire. */
function is_required(field: AstSchemaField): boolean {
	return (
		field.presence === undefined &&
		(field.vanilla_acorn === undefined || field.vanilla_acorn === 'always')
	);
}

interface Member {
	key: string;
	ts: string;
	optional: boolean;
	doc?: string[];
}

function merge_interface(name: string, entries: AstSchemaEntry[], errors: string[]): string[] {
	const keys: string[] = [];
	for (const entry of entries) {
		let previous = -1;
		for (const field of entry.fields) {
			const at = keys.indexOf(field.key);
			if (at === -1) {
				keys.splice(previous + 1, 0, field.key);
				previous += 1;
			} else {
				if (at < previous) errors.push(`${name}: entries order ${field.key} differently`);
				previous = at;
			}
		}
	}
	const members: Member[] = [];
	for (const key of keys) {
		const fields = entries.flatMap((entry) => entry.fields.filter((f) => f.key === key));
		if (key === 'type') {
			const types = [...new Set(entries.map((entry) => `'${entry.type}'`))];
			members.push({
				key,
				ts: types.join(' | '),
				optional: false,
				doc: fields.find((f) => f.doc)?.doc
			});
			continue;
		}
		const typed = fields.filter((f) => member_ts(f) !== undefined);
		if (typed.length === 0) {
			if (fields.some((f) => f.kind !== 'comments')) errors.push(`${name}.${key}: no \`ts\``);
			continue;
		}
		if (typed.length !== fields.length) errors.push(`${name}.${key}: \`ts\` on some entries only`);
		const ts = member_ts(typed[0]!)!;
		if (typed.some((f) => member_ts(f) !== ts))
			errors.push(`${name}.${key}: entries disagree on \`ts\``);
		const docs = fields.filter((f) => f.doc);
		if (docs.some((f) => JSON.stringify(f.doc) !== JSON.stringify(docs[0]!.doc))) {
			errors.push(`${name}.${key}: entries disagree on \`doc\``);
		}
		const optional = fields.length < entries.length || fields.some((f) => !is_required(f));
		members.push({ key, ts, optional, doc: docs[0]?.doc });
	}
	const docs = entries.filter((entry) => entry.doc);
	if (docs.length > 1) errors.push(`${name}: \`doc\` on more than one entry`);
	const extended = entries.filter((entry) => entry.extends);
	if (extended.length > 1) errors.push(`${name}: \`extends\` on more than one entry`);
	const heritage = extended[0]?.extends ? ` extends ${extended[0].extends.join(', ')}` : '';
	const out = [...doc_block(docs[0]?.doc, ''), `export interface ${name}${heritage} {`];
	for (const member of members) {
		out.push(...doc_block(member.doc, '\t'));
		const head = `\t${member.key}${member.optional ? '?' : ''}:`;
		const line = `${head} ${member.ts};`;
		const alternatives = member.ts.split(' | ');
		if (width(line) <= PRINT_WIDTH || alternatives.length === 1) out.push(line);
		else {
			out.push(head);
			alternatives.forEach((alt, i) =>
				out.push(`\t\t| ${alt}${i === alternatives.length - 1 ? ';' : ''}`)
			);
		}
	}
	out.push('}');
	return out;
}

function print_alias(alias: AstSchemaAlias): string[] {
	const out = doc_block(alias.doc, '');
	const head = `export type ${alias.alias} =`;
	if (alias.intersect !== undefined) {
		const one = `${head} (${alias.union.join(' | ')}) & ${alias.intersect};`;
		if (width(one) <= PRINT_WIDTH) return [...out, one];
		return [
			...out,
			`${head} (`,
			...alias.union.map((member) => `\t| ${member}`),
			') &',
			`\t${alias.intersect};`
		];
	}
	const one = `${head} ${alias.union.join(' | ')};`;
	if (width(one) <= PRINT_WIDTH) return [...out, one];
	const hung = `\t${alias.union.join(' | ')};`;
	if (width(hung) <= PRINT_WIDTH) return [...out, head, hung];
	return [
		...out,
		head,
		...alias.union.map((member, i) => `\t| ${member}${i === alias.union.length - 1 ? ';' : ''}`)
	];
}

/**
 * The `.d.ts` text the table generates, or the list of what keeps it from generating one (an
 * interface whose entries disagree on a member's type, doc, or order).
 */
export function generate_ast_types(schema: AstSchema): string | { errors: string[] } {
	const errors: string[] = [];
	const by_interface = new Map<string, AstSchemaEntry[]>();
	for (const section of schema.sections) {
		for (const item of section.entries) {
			if (is_alias(item) || item.opaque) continue;
			if (item.interface === undefined) {
				errors.push(`${item.type ?? '?'}: a declared entry with no \`interface\``);
				continue;
			}
			const list = by_interface.get(item.interface);
			if (list) list.push(item);
			else by_interface.set(item.interface, [item]);
		}
	}
	const blocks: string[] = [doc_block(schema.doc, '').join('\n')];
	const printed = new Set<string>();
	for (const section of schema.sections) {
		const declarations: string[] = [];
		for (const item of section.entries) {
			if (is_alias(item)) {
				declarations.push(print_alias(item).join('\n'));
				continue;
			}
			if (item.opaque || item.interface === undefined || printed.has(item.interface)) continue;
			printed.add(item.interface);
			declarations.push(
				merge_interface(item.interface, by_interface.get(item.interface)!, errors).join('\n')
			);
		}
		if (declarations.length === 0) continue;
		blocks.push(`//\n// ${section.title}\n//`, ...declarations);
	}
	if (errors.length > 0) return { errors };
	return `${blocks.join('\n\n')}\n`;
}
