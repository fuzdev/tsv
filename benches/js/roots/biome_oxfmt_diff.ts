/**
 * `benches/js/diagnostics/biome_oxfmt_diff.ts`'s root. Its reach: tsv's N-API addon at the
 * `corpus` profile, prettier, biome, and oxfmt, loaded in process (biome and oxfmt at the
 * versions the bench install pins, each once the tool asks for it); the files `--file` and
 * `--files` name, each read through a read-only handle on its directory, relative to the
 * checkout or absolute; and stdin, for `--stdin`. It writes nothing but stdout and runs no
 * program.
 *
 * @module
 */

import { basename, dirname } from 'node:path';

import { process } from '@fuzdev/gro/process.ts';

import {
	type DiffFormatter,
	type DiffFormatterName,
	run_biome_oxfmt_diff
} from '../diagnostics/biome_oxfmt_diff.ts';
import { BiomeImplementation } from '../lib/biome.ts';
import { OxcImplementation } from '../lib/oxc.ts';
import { load_all_versions } from '../lib/versions.ts';
import { exit_with, init_implementations, open_readonly_dir } from './compare_deps.ts';

/** Biome's wasm (synchronous) or oxfmt (async only), initialized. */
const load_formatter = async (name: DiffFormatterName): Promise<DiffFormatter> => {
	const versions = await load_all_versions();
	if (name === 'biome') {
		const biome = new BiomeImplementation(versions.biome);
		await biome.init();
		return {
			supports_format_language: (language) => biome.supports_format_language(language),
			format: (source, language) => biome.format(source, language),
			dispose: () => biome.dispose()
		};
	}
	const oxc = new OxcImplementation(versions.oxc);
	await oxc.init();
	return {
		supports_format_language: (language) => oxc.supports_format_language(language),
		format: (source, language) => oxc.format_async(source, language),
		dispose: () => oxc.dispose()
	};
};

const code = await run_biome_oxfmt_diff(
	{
		init_implementations,
		load_formatter,
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
