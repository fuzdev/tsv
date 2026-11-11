/**
 * `validate:napi`'s entry: the size bands of a staged N-API platform package's two binaries
 * (`scripts/validate_napi_artifact.ts`).
 *
 * A named exception to the host-module rule: this entry and everything it imports use Node's
 * own `node:*` modules and no package, because CI runs it where no `node_modules` is
 * installed: in the `artifacts` job and on every release build row, `win32-x64` included
 * (docs/tasks.md §`validate:napi`). The root config's Deno walk (`typecheck:scripts`) checks
 * this module with no `node_modules` in view, so a package import here or beneath it fails
 * `check`.
 *
 * The checkout is this module's own location, never the working directory. It reads
 * directory entries and file sizes, and writes nothing.
 *
 * @module
 */

import { readdir, stat } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

import { run_validate_napi_artifact } from '../validate_napi_artifact.ts';

const checkout = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');

process.exitCode = await run_validate_napi_artifact(
	{
		checkout,
		list_dirs: async (path) =>
			(await readdir(path, { withFileTypes: true }))
				.filter((entry) => entry.isDirectory())
				.map((entry) => entry.name),
		size: async (path) => (await stat(path)).size,
		print: (line) => console.log(line),
		print_error: (line) => console.error(line)
	},
	process.argv.slice(2)
);
