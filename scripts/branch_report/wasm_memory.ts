/**
 * A child of the branch report: the linear-memory high-water of one built `wasm-pack --target
 * web` bundle over a corpus. `deno run --allow-read wasm_memory.ts <glue.js> <file-list> <surface>`
 * prints one JSON line. `<surface>` is `format` or `parse`; the file list is one path per line.
 * Linear memory never shrinks, so `memory.buffer.byteLength` after the sweep IS the peak the sweep
 * drove (a deterministic number, unlike a timing).
 *
 * Two regimes: `warm_high_water_bytes`, one instance over the whole corpus (the reset-reuse
 * high-water, set by the largest file), and `cold_largest_bytes`, a fresh instance per language
 * for that language's largest file (the peak of a cold first call).
 *
 * @module
 */

import { readFile } from 'node:fs/promises';

const [glue_path, list_path, surface] = Deno.args;
if (!glue_path || !list_path || (surface !== 'format' && surface !== 'parse')) {
	throw new Error('usage: wasm_memory.ts <glue.js> <file-list> <format|parse>');
}

type Language = 'typescript' | 'svelte' | 'css';

const language_of = (path: string): Language =>
	path.endsWith('.svelte') ? 'svelte' : path.endsWith('.css') ? 'css' : 'typescript';

interface LoadedModule {
	memory: WebAssembly.Memory;
	call: (language: Language, source: string) => void;
}

let import_counter = 0;

/** A fresh instance: a cache-busted import of the glue is a new module, so a new linear memory. */
async function load(glue: string): Promise<LoadedModule> {
	const mod = (await import(`file://${glue}?instance=${import_counter++}`)) as Record<
		string,
		(...args: Array<unknown>) => unknown
	>;
	const wasm_path = glue.replace(/tsv_wasm\.js$/, 'tsv_wasm_bg.wasm');
	const exports = mod.initSync!({ module: await readFile(wasm_path) }) as {
		memory: WebAssembly.Memory;
	};
	const fns: Record<Language, (source: string) => unknown> =
		surface === 'format'
			? {
					typescript: (s) => mod.format_typescript!(s),
					svelte: (s) => mod.format_svelte!(s),
					css: (s) => mod.format_css!(s)
				}
			: {
					typescript: (s) => mod.parse_typescript_json!(s),
					svelte: (s) => mod.parse_svelte_json!(s),
					css: (s) => mod.parse_css_json!(s)
				};
	return { memory: exports.memory, call: (language, source) => void fns[language](source) };
}

const paths = (await readFile(list_path, 'utf8')).split('\n').filter((line) => line !== '');
const sources = await Promise.all(
	paths.map(async (path) => ({
		path,
		language: language_of(path),
		source: await readFile(path, 'utf8')
	}))
);
const decoded_bytes = (s: { source: string }): number => s.source.length;

const warm = await load(glue_path);
let errors = 0;
let first_error: string | null = null;
const record_error = (path: string, err: unknown): void => {
	errors++;
	first_error ??= `${path}: ${err instanceof Error ? err.message : String(err)}`;
};
for (const { path, language, source } of sources) {
	try {
		warm.call(language, source);
	} catch (err) {
		record_error(path, err);
	}
}

const largest = new Map<Language, { path: string; source: string }>();
for (const entry of sources) {
	const current = largest.get(entry.language);
	if (!current || decoded_bytes(entry) > decoded_bytes(current)) largest.set(entry.language, entry);
}
const cold_largest_bytes: Partial<Record<Language, number>> = {};
for (const [language, { path, source }] of largest) {
	const cold = await load(glue_path);
	try {
		cold.call(language, source);
	} catch (err) {
		record_error(path, err);
	}
	cold_largest_bytes[language] = cold.memory.buffer.byteLength;
}

console.log(
	JSON.stringify({
		surface,
		files: sources.length,
		errors,
		first_error,
		warm_high_water_bytes: warm.memory.buffer.byteLength,
		cold_largest_bytes
	})
);
