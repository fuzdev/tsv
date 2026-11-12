/**
 * `benches/js/diagnostics/wasm_format_probe.ts`'s root. Its reach: the wasm-pack `nodejs` glue
 * builds it's pointed at, loaded in process (`wasm_glue.ts`); and a corpus directory, read
 * through the corpus loader, which Bun's bench runs too and so reads with `node:fs`, a named
 * exception whose reason is Bun. It writes nothing and runs no program.
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';

import { run_wasm_format_probe } from '../diagnostics/wasm_format_probe.ts';
import { DirectoryLoader } from '../lib/corpus.ts';
import { load_fresh_glue } from './wasm_glue.ts';

const code = await run_wasm_format_probe(
	{
		load_glue: async (path) => (await load_fresh_glue(path)).exports,
		load_files: (path, logger) => new DirectoryLoader(path).load(logger)
	},
	process.args
);
process.exit(code);
