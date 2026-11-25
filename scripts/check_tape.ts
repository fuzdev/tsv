/**
 * The tape gate (`npm run check:tape`): every fixture document's span-only wire through the tape
 * (`docs/tape_format.md`) and back, graded three ways.
 *
 * The Rust half is one `tsv_debug tape` process streaming each document's source, span-only wire
 * and the transcoder's tape (`tsv_arena::tape::Transcoder`) as NDJSON — no per-file spawn, no
 * sidecar. This half, per document:
 *
 * - **the producers agree**: the offline JS encoder (`tape_encode.ts`) writes the wire's tree to a
 *   tape byte for byte the transcoder's — two implementations of one format, each the other's
 *   drift check;
 * - **the fast path is exact**: the shipped decoder (`crates/tsv_wasm/npm/tape_decode.js`)
 *   decodes it to a tree deep-equal to `JSON.parse`'s of the wire, key order included;
 * - **the slow path is exact**: the same tape through the iterative interpreter alone
 *   (`decode_tape_iterative`, what a stack overflow falls back to), and the wire re-encoded with
 *   every typed record entry-generic, so every entry's field descriptors are read by the
 *   interpreter that serves the records no fast shape covers.
 *
 * The NDJSON is split on `\n` alone (`lines_of`): a source holding U+2028 is embedded raw, and
 * `node:readline` would end a line there.
 *
 * `--update-shapes` instead rewrites the committed fast-shape list (`tsv_tape_shapes.json`) from
 * the masks the fixture wire carries — every (entry, mask) a record of the wire takes — for
 * `npm run ast-schema:update` to regenerate the decoder and the transcoder's tables from. A run
 * reports how many records fell outside the list, which an update folds in; that count is no
 * failure, since the entry-generic record carries any mask.
 *
 * Usage: `npm run check:tape` (builds `target/corpus/tsv_debug` first), or directly,
 * `node scripts/roots/check_tape.ts [root] [--update-shapes]`.
 *
 * Its reach: the checkout, read for the table and the shape list and written under
 * `--update-shapes`, and the checkout's `target/corpus/tsv_debug`, run in it (`CheckTapeDeps`,
 * built by `scripts/roots/check_tape.ts`).
 */

import type { FsDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';

import {
	decode_tape,
	decode_tape_iterative,
	SHAPES_HASH
} from '../crates/tsv_wasm/npm/tape_decode.js';
import { lines_of } from '../benches/js/lib/text_lines.ts';
import { AST_SCHEMA_PATH, parse_ast_schema } from './ast_schema.ts';
import { to_ended_code } from './check_loc.ts';
import { encode_tape } from './tape_encode.ts';
import {
	build_tape_schema,
	layout_tape_shapes,
	parse_tape_shapes,
	shape_key,
	shapes_file_from_census,
	TAPE_SHAPES_PATH,
	type TapeSchema
} from './tape_schema.ts';

/** The `tsv_debug` binary of the shared `--profile corpus` build world. */
export const CHECK_TAPE_TSV_DEBUG = 'target/corpus/tsv_debug';

const DEFAULT_ROOT = 'tests/fixtures';

/** Failing documents listed in full before the rest are only counted. */
const MAX_REPORTED = 20;

/** What `run_check_tape` reaches. */
export interface CheckTapeDeps {
	/** The checkout: the table and shape list read, the shape list written by an update. */
	checkout: FsDir;
	/** The checkout's `target/corpus/tsv_debug` (`CHECK_TAPE_TSV_DEBUG`). */
	tsv_debug: SpawnProgram;
}

/** One `tsv_debug tape` record. */
interface TapeRecord {
	path: string;
	language: 'svelte' | 'typescript' | 'css';
	source: string;
	span: unknown;
	tape: string;
}

/**
 * The text a wire's offsets index: the source, less a leading byte-order mark for a Svelte or CSS
 * document, whose wires elide it (`tsv_debug::tape::wire_source`).
 */
export const wire_source = (language: string, source: string): string =>
	language !== 'typescript' && source.startsWith('﻿') ? source.slice(1) : source;

const describe = (v: unknown): string => {
	const text = JSON.stringify(v) ?? String(v);
	return text.length > 60 ? `${text.slice(0, 57)}...` : text;
};

/**
 * Where two JSON trees first differ — a value, an array's length, an object's keys or their
 * order — as a path and the two sides, or `null` when they are equal with keys in one order.
 * Numbers compare by `Object.is`, so `-0` is not `0`. Walks with a stack, so a tree as deep as
 * the parsers accept compares on any stack; the first difference in key order is the one named.
 */
export function wire_difference(a: unknown, b: unknown, path = '$'): string | null {
	const stack: Array<[unknown, unknown, string]> = [[a, b, path]];
	while (stack.length > 0) {
		const [x, y, at] = stack.pop()!;
		if (x === null || y === null || typeof x !== 'object' || typeof y !== 'object') {
			if (!Object.is(x, y)) return `${at}: ${describe(x)} vs ${describe(y)}`;
			continue;
		}
		if (Array.isArray(x) !== Array.isArray(y)) return `${at}: an array vs an object`;
		const kx = Object.keys(x);
		const ky = Object.keys(y);
		if (kx.length !== ky.length || kx.some((k, i) => k !== ky[i])) {
			return `${at}: keys [${kx.join(', ')}] vs [${ky.join(', ')}]`;
		}
		for (let i = kx.length - 1; i >= 0; i--) {
			const key = kx[i]!;
			stack.push([
				(x as Record<string, unknown>)[key],
				(y as Record<string, unknown>)[key],
				Array.isArray(x) ? `${at}[${key}]` : `${at}.${key}`
			]);
		}
	}
	return null;
}

/** Where two tapes first differ, as a word index and both words, or `null` when equal. */
export function tape_difference(a: Uint8Array, b: Uint8Array): string | null {
	const n = Math.min(a.length, b.length);
	let i = 0;
	while (i < n && a[i] === b[i]) i++;
	if (i === n && a.length === b.length) return null;
	const word = (bytes: Uint8Array, at: number): string =>
		at + 4 <= bytes.length
			? String(new DataView(bytes.buffer, bytes.byteOffset).getUint32(at, true))
			: 'end';
	const at = i - (i % 4);
	return `byte ${i} (word ${at / 4}): ${word(a, at)} vs ${word(b, at)}, lengths ${a.length} vs ${b.length}`;
}

/** Grades one document; the failures it finds, none when it passes. */
export function grade_document(
	tree: unknown,
	source: string,
	transcoded: Uint8Array,
	schema: TapeSchema
): string[] {
	const failures: string[] = [];
	let encoded: Uint8Array;
	try {
		encoded = encode_tape(tree, source, schema).bytes;
	} catch (error) {
		return [`the offline encoder threw: ${(error as Error).message}`];
	}
	const producers = tape_difference(encoded, transcoded);
	if (producers !== null) failures.push(`encoder vs transcoder at ${producers}`);
	const decoders: Array<[string, () => unknown]> = [
		['decode_tape', () => decode_tape(encoded, source)],
		['decode_tape_iterative', () => decode_tape_iterative(encoded, source)],
		[
			'decode_tape over entry-generic records',
			() => decode_tape(encode_tape(tree, source, schema, { no_fast_shapes: true }).bytes, source)
		]
	];
	for (const [name, decode] of decoders) {
		let decoded: unknown;
		try {
			decoded = decode();
		} catch (error) {
			failures.push(`${name} threw: ${(error as Error).message}`);
			continue;
		}
		const d = wire_difference(decoded, tree);
		if (d !== null) failures.push(`${name} vs JSON.parse at ${d}`);
	}
	return failures;
}

/** The tape schema the committed table and shape list make. */
async function read_tape_schema(checkout: FsDir): Promise<TapeSchema | { errors: string[] }> {
	const table = parse_ast_schema(await checkout.read_text(AST_SCHEMA_PATH));
	if ('errors' in table) return table;
	const shapes = parse_tape_shapes(await checkout.read_text(TAPE_SHAPES_PATH));
	if ('errors' in shapes) return shapes;
	return build_tape_schema(table, shapes);
}

/**
 * The gate over `args`' root (the fixture tree by default), or with `--update-shapes` the shape
 * list's rewrite.
 * @returns the exit code: `1` on any failure, `2` on an unknown flag, else `0`
 */
export async function run_check_tape(
	deps: CheckTapeDeps,
	args: ReadonlyArray<string>
): Promise<number> {
	const update = args.includes('--update-shapes');
	const positional = args.filter((arg) => arg !== '--update-shapes');
	if (positional.some((arg) => arg.startsWith('-')) || positional.length > 1) {
		console.error(
			`unknown argument(s) ${positional.join(' ')} — check:tape takes [root] [--update-shapes]`
		);
		return 2;
	}
	const root = positional[0] ?? DEFAULT_ROOT;
	if (update && root !== DEFAULT_ROOT) {
		console.error('--update-shapes reads the whole fixture tree; it takes no root');
		return 2;
	}
	const schema = await read_tape_schema(deps.checkout);
	if ('errors' in schema) {
		console.error(`${AST_SCHEMA_PATH} and ${TAPE_SHAPES_PATH} do not make a tape schema:`);
		for (const error of schema.errors) console.error(`  ${error}`);
		return 1;
	}
	const failures: string[] = [];
	if (!update && schema.hash !== SHAPES_HASH) {
		failures.push(
			'the committed decoder was generated from another table or shape list — run `npm run ast-schema:update`'
		);
	}
	const started = performance.now();
	const child = await deps.tsv_debug.start(['tape', root], {
		cwd: deps.checkout,
		stdio: ['inherit', 'pipe', 'inherit']
	});
	const graded = { svelte: 0, typescript: 0, css: 0 };
	const census = new Map<string, readonly [number, number]>();
	const outside = new Set<string>();
	let outside_records = 0;
	const stats = { fast: 0, generic: 0, escape: 0 };
	const failed: string[] = [];
	for await (const line of lines_of(child.stdout!)) {
		if (line === '') continue;
		const record = JSON.parse(line) as TapeRecord;
		const source = wire_source(record.language, record.source);
		graded[record.language]++;
		const encoded = encode_tape(record.span, source, schema, {
			census: (entry, mask) => {
				const key = `${entry}:${mask}`;
				census.set(key, [entry, mask]);
				if (mask !== 0 && !schema.shape_ids.has(shape_key(entry, mask))) {
					outside.add(key);
					outside_records++;
				}
			}
		});
		stats.fast += encoded.stats.fast;
		stats.generic += encoded.stats.generic;
		stats.escape += encoded.stats.escape;
		if (update) continue;
		const problems = grade_document(
			record.span,
			source,
			Buffer.from(record.tape, 'base64'),
			schema
		);
		if (problems.length > 0)
			failed.push(`${record.path} (${record.language}): ${problems.join('; ')}`);
	}
	const code = to_ended_code(await child.closed);
	if (code !== 0)
		failures.push(`` + '`' + `${CHECK_TAPE_TSV_DEBUG} tape` + '`' + ` exited ${code}`);
	const total = graded.svelte + graded.typescript + graded.css;
	const seconds = ((performance.now() - started) / 1000).toFixed(1);
	if (update) {
		if (failures.length === 0 && code === 0) {
			const file = shapes_file_from_census(schema.entries, census.values());
			await deps.checkout.write(TAPE_SHAPES_PATH, layout_tape_shapes(file));
			console.log(
				`${TAPE_SHAPES_PATH}: rewritten from ${total} documents, ${file.shapes.length} masks beyond the all-required ones — now run ` +
					'`npm run ast-schema:update`'
			);
			return 0;
		}
		for (const f of failures) console.log(`  ${f}`);
		return 1;
	}
	console.log(
		`tape: ${total} documents (${graded.svelte} svelte, ${graded.typescript} typescript, ${graded.css} css) in ${seconds}s — ${stats.fast} fast-shape, ${stats.generic} entry-generic, ${stats.escape} self-describing records`
	);
	if (outside.size > 0) {
		console.log(
			`  ${outside_records} records in ${outside.size} masks outside the shape list — ` +
				'`npm run tape-shapes:update`' +
				` folds them in`
		);
	}
	if (root === DEFAULT_ROOT) {
		for (const [language, count] of Object.entries(graded)) {
			if (count === 0) failures.push(`no ${language} document was graded`);
		}
	} else if (total === 0) failures.push(`no document under ${root} was graded`);
	if (failed.length > 0) {
		for (const f of failed.slice(0, MAX_REPORTED)) console.log(`  ${f}`);
		if (failed.length > MAX_REPORTED) console.log(`  … and ${failed.length - MAX_REPORTED} more`);
		failures.push(`${failed.length} document(s) failed`);
	}
	if (failures.length > 0) {
		for (const f of failures) console.error(`check:tape: ${f}`);
		return 1;
	}
	return 0;
}
