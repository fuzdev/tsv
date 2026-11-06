/**
 * The C-FFI library's half of the PGO pipeline (`pgo.ts --ffi`), run by Deno because it
 * loads a `tsv_ffi` build through `Deno.dlopen` — through the bench's own FFI wrapper
 * (`benches/js/lib/ffi.ts` `NativeImplementation`, its facade included), so a library is
 * called exactly as the Deno bench row calls it: per listed file, in order, each named
 * operation — `parse` (the package's default parse, the span-only wire through the
 * facade's `JSON.parse`), `parse_internal` (the parse that hands JS nothing) and `format`
 * — recorded as a sha256 of its output, or its failure. `pgo.ts train` runs it over the
 * instrumented library (`--require-ok`: every call must succeed, or the input did not
 * train), and `pgo.ts byteid-ffi` over the plain and the profile-guided library, whose
 * records must be identical.
 *
 * The goal is the bench rows' own: none named, so a parse reads TypeScript at the module
 * goal and a format reads it module-then-script, as every surface's bare call does.
 *
 * Usage: deno run --no-lock --allow-ffi --allow-read --allow-write --allow-env
 *          scripts/pgo/ffi_ops.ts --lib <library> --list <list file> --out <out.ndjson>
 *          --ops <parse,parse_internal,format> [--require-ok]
 *
 * @module
 */

import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { parseArgs } from 'node:util';

import { NativeImplementation } from '../../benches/js/lib/ffi.ts';
import { parse_language } from './layout.ts';

/** An operation the library is asked per file. */
type FfiOp = 'parse' | 'parse_internal' | 'format';
const FFI_OPS: readonly FfiOp[] = ['parse', 'parse_internal', 'format'];

/** One answer: a file's operation, its output's hash or its failure. */
interface FfiRecord {
	path: string;
	op: FfiOp;
	/** The file is not UTF-8, so nothing was asked. */
	unreadable?: true;
	ok?: boolean;
	hash?: string;
	/** A failure's message, first line. */
	error?: string;
}

const { values } = parseArgs({
	options: {
		lib: { type: 'string' },
		list: { type: 'string' },
		out: { type: 'string' },
		ops: { type: 'string', default: FFI_OPS.join(',') },
		'require-ok': { type: 'boolean', default: false }
	}
});
const ops = values.ops!.split(',').filter(Boolean) as FfiOp[];
if (!values.lib || !values.list || !values.out || ops.some((op) => !FFI_OPS.includes(op))) {
	console.error(
		'usage: deno run … scripts/pgo/ffi_ops.ts --lib <library> --list <file> --out <ndjson> ' +
			'--ops <parse,parse_internal,format> [--require-ok]'
	);
	Deno.exit(2);
}

const binding = new NativeImplementation(values.lib);
await binding.init();
const decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
const sha256 = (text: string): string => createHash('sha256').update(text).digest('hex');

const call = (op: FfiOp, path: string, source: string): Omit<FfiRecord, 'path' | 'op'> => {
	const language = parse_language(path);
	try {
		const text =
			op === 'parse'
				? JSON.stringify(binding.parse(source, language))
				: op === 'format'
					? binding.format(source, language)
					: (binding.parse_internal(source, language), '');
		return { ok: true, hash: sha256(text) };
	} catch (e) {
		return { ok: false, error: String((e as Error).message ?? e).split('\n')[0] };
	}
};

const records: FfiRecord[] = [];
for (const path of readFileSync(values.list, 'utf8').split('\n').filter(Boolean)) {
	let source: string | null = null;
	try {
		// a leading BOM is kept: a file read hands the engine the bytes as read
		source = decoder.decode(readFileSync(path));
	} catch {
		for (const op of ops) records.push({ path, op, unreadable: true });
	}
	if (source === null) continue;
	for (const op of ops) records.push({ path, op, ...call(op, path, source) });
}
writeFileSync(values.out, records.map((r) => JSON.stringify(r)).join('\n') + '\n');

if (values['require-ok']) {
	const bad = records.filter((r) => r.ok !== true);
	const counts = Object.fromEntries(
		ops.map((op) => [op, records.filter((r) => r.op === op).length])
	);
	console.log(`ffi ops: ${JSON.stringify(counts)}, ${bad.length} failed`);
	if (bad.length > 0) {
		console.error(
			bad
				.slice(0, 10)
				.map((r) => `${r.op} ${r.path}: ${r.error ?? 'unreadable'}`)
				.join('\n')
		);
		Deno.exit(1);
	}
}
// exit, not unload: an instrumented library writes its profile from the exit handler its
// runtime registered when it loaded
Deno.exit(0);
