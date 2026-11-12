/**
 * `benches/js/diagnostics/test262_compare.ts`'s root. Its reach: the manifest named by
 * `--manifest` and the test262 checkout it names, read through read-only handles opened by
 * those paths, relative to the checkout or absolute; and oxc-parser, loaded in process. It
 * writes nothing and runs no program.
 *
 * @module
 */

import { basename, dirname } from 'node:path';

import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import { process } from '@fuzdev/gro/process.ts';
import { parseSync } from 'oxc-parser';

import { run_test262_compare } from '../diagnostics/test262_compare.ts';
import { exit_with, open_readonly_dir } from './compare_deps.ts';

await run_test262_compare(
	{
		read_manifest: async (path) =>
			(await open_readonly_dir(dirname(path))).read_text(basename(path)),
		open_root: async (path) => {
			// opened on the first read, so a missing checkout counts each test unreadable
			let root: Promise<FsReadDir> | undefined;
			return {
				read_text: async (file) => (await (root ??= open_readonly_dir(path))).read_text(file)
			};
		},
		parse_sync: parseSync,
		stdout: process.stdout
	},
	process.args
);
exit_with(0);
