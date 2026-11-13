/**
 * The branch report's WASM-memory probe: the linear-memory high-water of one built `wasm-pack
 * --target web` bundle over a corpus. Its entry, `node scripts/roots/wasm_memory.ts <glue.js>
 * <file-list> <surface>`, prints one JSON line (`WasmMemoryResult`). `<surface>` is `format` or
 * `parse`; the file list is one path per line. Linear memory never shrinks, so
 * `memory.buffer.byteLength` after the sweep IS the peak the sweep drove (a deterministic number,
 * unlike a timing): the module's own allocator sets it, so the same `.wasm` reads the same under
 * any JS engine that runs the glue.
 *
 * Two regimes: `warm_high_water_bytes`, one instance over the whole corpus (the reset-reuse
 * high-water, set by the largest file), and `cold_largest_bytes`, a fresh instance per language
 * for that language's largest file (the peak of a cold first call).
 *
 * @module
 */

export type Surface = 'format' | 'parse';

type Language = 'typescript' | 'svelte' | 'css';

/** What a sweep prints. */
export interface WasmMemoryResult {
	surface: Surface;
	files: number;
	/** How many calls threw; the report fails a sweep with any (`assert_wasm_sweep`). */
	errors: number;
	first_error: string | null;
	warm_high_water_bytes: number;
	cold_largest_bytes: Record<string, number>;
}

/** A fresh instance of the bundle: its glue's exports and its own linear memory. */
export interface WasmInstance {
	memory: { readonly buffer: { readonly byteLength: number } };
	glue: Readonly<Record<string, unknown>>;
}

/** What a sweep reads: the file list and each listed file, and fresh instances of the bundle. */
export interface WasmMemoryDeps {
	read_text: (path: string) => Promise<string>;
	/** A new instance, with a new linear memory, on each call. */
	instantiate: () => Promise<WasmInstance>;
}

export const WASM_MEMORY_USAGE = 'usage: wasm_memory.ts <glue.js> <file-list> <format|parse>';

/** The probe's arguments. */
export function parse_wasm_memory_args(args: ReadonlyArray<string>): {
	glue: string;
	list: string;
	surface: Surface;
} {
	const [glue, list, surface] = args;
	if (!glue || !list || (surface !== 'format' && surface !== 'parse') || args.length !== 3) {
		throw new Error(WASM_MEMORY_USAGE);
	}
	return { glue, list, surface };
}

const language_of = (path: string): Language =>
	path.endsWith('.svelte') ? 'svelte' : path.endsWith('.css') ? 'css' : 'typescript';

const EXPORTS: Readonly<Record<Surface, Readonly<Record<Language, string>>>> = {
	format: { typescript: 'format_typescript', svelte: 'format_svelte', css: 'format_css' },
	parse: {
		typescript: 'parse_typescript_json',
		svelte: 'parse_svelte_json',
		css: 'parse_css_json'
	}
};

/** The surface's call for `language` on `instance`, throwing like the call when it is missing. */
function call_of(instance: WasmInstance, surface: Surface, language: Language) {
	return (source: string): void => {
		const name = EXPORTS[surface][language];
		(instance.glue[name] as (source: string) => unknown)(source);
	};
}

/** Sweep the files `list_path` names through `surface`, warm then cold (the module doc). */
export async function measure_wasm_memory(
	deps: WasmMemoryDeps,
	list_path: string,
	surface: Surface
): Promise<WasmMemoryResult> {
	const paths = (await deps.read_text(list_path)).split('\n').filter((line) => line !== '');
	const sources = [];
	for (const path of paths) {
		sources.push({ path, language: language_of(path), source: await deps.read_text(path) });
	}
	let errors = 0;
	let first_error: string | null = null;
	const record_error = (path: string, err: unknown): void => {
		errors++;
		first_error ??= `${path}: ${err instanceof Error ? err.message : String(err)}`;
	};

	const warm = await deps.instantiate();
	for (const { path, language, source } of sources) {
		try {
			call_of(warm, surface, language)(source);
		} catch (err) {
			record_error(path, err);
		}
	}

	// the largest file per language, by decoded length, the first of equals
	const largest = new Map<Language, { path: string; source: string }>();
	for (const entry of sources) {
		const current = largest.get(entry.language);
		if (!current || entry.source.length > current.source.length) largest.set(entry.language, entry);
	}
	const cold_largest_bytes: Partial<Record<Language, number>> = {};
	for (const [language, { path, source }] of largest) {
		const cold = await deps.instantiate();
		try {
			call_of(cold, surface, language)(source);
		} catch (err) {
			record_error(path, err);
		}
		cold_largest_bytes[language] = cold.memory.buffer.byteLength;
	}

	return {
		surface,
		files: sources.length,
		errors,
		first_error,
		warm_high_water_bytes: warm.memory.buffer.byteLength,
		cold_largest_bytes
	};
}
