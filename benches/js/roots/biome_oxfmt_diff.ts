/**
 * `benches/js/diagnostics/biome_oxfmt_diff.ts`'s root. Its reach: tsv's N-API addon at the
 * `corpus` profile, prettier, biome, and oxfmt, loaded in process; the files `--file` and
 * `--files` name, each read through a read-only handle on its directory, relative to the
 * checkout or absolute; and stdin, for `--stdin`. It writes nothing but stdout and runs no
 * program.
 *
 * @module
 */

import { basename, dirname } from 'node:path';

import { process } from '@fuzdev/gro/process.ts';

import { run_biome_oxfmt_diff } from '../diagnostics/biome_oxfmt_diff.ts';
import { exit_with, init_implementations, open_readonly_dir } from './compare_deps.ts';

const code = await run_biome_oxfmt_diff(
	{
		init_implementations,
		read_file: async (path) => (await open_readonly_dir(dirname(path))).read_text(basename(path)),
		read_stdin: async () => {
			const decoder = new TextDecoder();
			let text = '';
			for await (const chunk of process.stdin) text += decoder.decode(chunk, { stream: true });
			return text + decoder.decode();
		},
		stdout: process.stdout
	},
	process.args
);
exit_with(code);
