/**
 * The branch report's WASM-memory probe (`scripts/branch_report/wasm_memory.ts`), run by its
 * WASM-memory leg as `node scripts/roots/wasm_memory.ts <glue.js> <file-list> <format|parse>`.
 * Its reach: the bundle's glue, imported fresh per instance (a cache-busting query on its URL),
 * the `.wasm` beside it, the file list and each listed file, every one opened by its path,
 * symlinks followed; nothing else, no program and no variable.
 *
 * The bundle is the `--target web` one npm ships, the same bytes the report's size rows read:
 * its glue is an ES module whose `initSync` takes the module's bytes, which Node runs as any
 * engine does.
 *
 * @module
 */

import { pathToFileURL } from 'node:url';

import { fs } from '@fuzdev/gro/fs.ts';
import { create_fs_paths } from '@fuzdev/gro/fs_paths.ts';
import { process } from '@fuzdev/gro/process.ts';

import {
	measure_wasm_memory,
	parse_wasm_memory_args,
	type WasmInstance
} from '../branch_report/wasm_memory.ts';

const fs_paths = create_fs_paths(fs);

/** A file's text as `readFile(path, 'utf8')` gives it: a leading BOM kept, a bad byte U+FFFD. */
const decoder = new TextDecoder('utf-8', { ignoreBOM: true });

/** The file at `path`, absolute, its symlinks followed. */
const read = async (path: string): Promise<Uint8Array> => {
	const opened = await fs_paths.open_path(path);
	return await opened.dir.read(opened.path);
};

let code = 1;
try {
	const { glue, list, surface } = parse_wasm_memory_args(process.args);
	const bytes = await read(glue.replace(/tsv_wasm\.js$/, 'tsv_wasm_bg.wasm'));
	let instances = 0;
	const instantiate = async (): Promise<WasmInstance> => {
		// a cache-busted import of the glue is a new module, so a new linear memory
		const url = `${pathToFileURL(glue).href}?instance=${instances++}`;
		const module = (await import(url)) as Record<string, unknown>;
		const exports = (module.initSync as (options: { module: Uint8Array }) => WasmInstance['glue'])({
			module: bytes
		});
		return { memory: exports.memory as WasmInstance['memory'], glue: module };
	};
	const result = await measure_wasm_memory(
		{ read_text: async (path) => decoder.decode(await read(path)), instantiate },
		list,
		surface
	);
	await process.stdout.write(`${JSON.stringify(result)}\n`);
	code = 0;
} catch (err) {
	await process.stderr.write(`wasm_memory: ${err instanceof Error ? err.message : String(err)}\n`);
}
process.set_exit_code(code);
