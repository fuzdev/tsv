/**
 * The addon's in-process half of the PGO pipeline, run by Node because it loads a staged
 * N-API addon through the `@fuzdev/tsv` loader: per listed file, each named operation's
 * answer — the span-only wire `parse_<lang>_json` returns, the type strip's code
 * (`strip_typescript`, TypeScript paths), the import records (`parse_imports_typescript` /
 * `parse_imports_svelte`) — as a sha256 of the output, or the failure. `pgo.ts train`
 * runs it to train the strip and the import records (`--require-ok`), and `pgo.ts byteid`
 * to compare their outputs.
 *
 * The hashes are spelled as the native CLI prints the same answer, so they compare to its
 * stdout without holding either: the parse wire plus the newline `tsv parse` prints after
 * it, the strip's code as `tsv strip` prints it. A `.mts` path strips at the module goal,
 * as the CLI reads that extension; every other path reads the goal as a format does.
 *
 * Usage: node scripts/pgo/addon_ops.ts --loader <loader index.js> --list <list file>
 *          --out <out.ndjson> --ops <parse,strip,imports> [--require-ok]
 *
 * @module
 */

import { createHash } from 'node:crypto';
import { readFileSync, realpathSync, writeFileSync } from 'node:fs';
import process from 'node:process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { parseArgs } from 'node:util';

import { parse_language } from './layout.ts';

/** An operation the addon is asked per file. */
export type AddonOp = 'parse' | 'strip' | 'imports';

/** One answer: a file's operation, its output's hash or its failure. */
export interface AddonRecord {
	path: string;
	op: AddonOp;
	/** The CLI refuses invalid UTF-8 at the read, so the addon is not asked. */
	unreadable?: true;
	ok?: boolean;
	hash?: string;
	/** A returned failure's `kind` and `message`. */
	kind?: string;
	message?: string;
	/** A thrown error's message — always a bug. */
	thrown?: string;
}

/** Whether `tsv strip` takes the path (it refuses JavaScript paths). */
export const is_strip_path = (path: string): boolean => /\.(?:c|m)?ts$/i.test(path);

/** The ops that apply to a path: the strip to TypeScript paths, the import records to
 * TypeScript and Svelte. */
export const ops_for = (path: string, ops: readonly AddonOp[]): AddonOp[] => {
	const language = parse_language(path);
	return ops.filter(
		(op) =>
			op === 'parse' ||
			(op === 'strip' && is_strip_path(path)) ||
			(op === 'imports' && language !== 'css')
	);
};

const main = async (): Promise<void> => {
	const { values } = parseArgs({
		options: {
			loader: { type: 'string' },
			list: { type: 'string' },
			out: { type: 'string' },
			ops: { type: 'string', default: 'parse' },
			'require-ok': { type: 'boolean', default: false }
		}
	});
	const ops = values.ops!.split(',').filter(Boolean) as AddonOp[];
	if (
		!values.loader ||
		!values.list ||
		!values.out ||
		ops.some((op) => !['parse', 'strip', 'imports'].includes(op))
	) {
		console.error(
			'usage: node scripts/pgo/addon_ops.ts --loader <index.js> --list <file> --out <ndjson> --ops <parse,strip,imports> [--require-ok]'
		);
		process.exit(2);
	}

	// deno-lint-ignore no-explicit-any
	const tsv: any = await import(pathToFileURL(values.loader).href);
	const decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
	const sha256 = (text: string): string => createHash('sha256').update(text).digest('hex');

	const call = (op: AddonOp, path: string, source: string): Omit<AddonRecord, 'path' | 'op'> => {
		const language = parse_language(path);
		// deno-lint-ignore no-explicit-any
		let result: any;
		try {
			result =
				op === 'parse'
					? tsv[`parse_${language}_json`](source)
					: op === 'strip'
						? tsv.strip_typescript(
								source,
								/\.mts$/i.test(path) ? { sourceType: 'module' } : undefined
							)
						: tsv[`parse_imports_${language}`](source);
		} catch (e) {
			return { thrown: String((e as Error).message ?? e) };
		}
		if (!result.ok) return { ok: false, kind: result.error.kind, message: result.error.message };
		const text =
			op === 'parse'
				? `${result.json}\n`
				: op === 'strip'
					? result.code
					: JSON.stringify({ hasModuleSyntax: result.hasModuleSyntax, imports: result.imports });
		return { ok: true, hash: sha256(text) };
	};

	const records: AddonRecord[] = [];
	for (const path of readFileSync(values.list, 'utf8').split('\n').filter(Boolean)) {
		const file_ops = ops_for(path, ops);
		if (file_ops.length === 0) continue;
		let source: string | null = null;
		try {
			// a leading BOM is kept: the CLI hands the engine the bytes as read
			source = decoder.decode(readFileSync(path));
		} catch {
			for (const op of file_ops) records.push({ path, op, unreadable: true });
		}
		if (source === null) continue;
		for (const op of file_ops) records.push({ path, op, ...call(op, path, source) });
	}
	writeFileSync(values.out, records.map((r) => JSON.stringify(r)).join('\n') + '\n');

	if (values['require-ok']) {
		// a strip's refusal is a verdict; every other failure means the input did not train
		const bad = records.filter(
			(r) =>
				r.unreadable ||
				r.thrown ||
				(r.ok === false && !(r.op === 'strip' && r.kind === 'unsupported'))
		);
		const counts = Object.fromEntries(
			ops.map((op) => [op, records.filter((r) => r.op === op).length])
		);
		console.log(`addon ops: ${JSON.stringify(counts)}, ${bad.length} failed`);
		if (bad.length > 0) {
			console.error(
				bad
					.slice(0, 10)
					.map((r) => `${r.op} ${r.path}: ${r.thrown ?? r.message ?? 'unreadable'}`)
					.join('\n')
			);
			process.exit(1);
		}
	}
};

// run as a script, not imported (pgo.ts imports its types and helpers) — read off argv
// rather than `import.meta.main`, which Node only has from 24.2 and the pipeline supports
// Node from 22.18
const entry = process.argv[1];
if (entry !== undefined && realpathSync(entry) === fileURLToPath(import.meta.url)) await main();
