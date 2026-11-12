/**
 * `benches/js/diagnostics/wasm_memory_probe.ts`'s root, with `wasm_format_probe.ts`'s reach:
 * the glue builds it's pointed at, each load a fresh instance whose memory it reads
 * (`wasm_glue.ts`), and a corpus directory, through the corpus loader. It writes nothing but
 * stdout and runs no program.
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';

import { run_wasm_memory_probe } from '../diagnostics/wasm_memory_probe.ts';
import { DirectoryLoader } from '../lib/corpus.ts';
import { load_fresh_glue } from './wasm_glue.ts';

const code = await run_wasm_memory_probe(
	{
		load_fresh_glue,
		load_files: (path, logger) => new DirectoryLoader(path).load(logger),
		stdout: process.stdout
	},
	process.args
);
process.exit(code);
