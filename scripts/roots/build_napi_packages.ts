/**
 * `build:napi:packages`' staging entry: the `@fuzdev/tsv` loader and the host's platform
 * package staged into `crates/tsv_napi/pkg/` (`scripts/build_napi_packages.ts`).
 *
 * A named exception to the host-module rule: this entry and everything it imports use Node's
 * own `node:*` modules and no package, because CI runs it where no `node_modules` is
 * installed: on Windows and macOS (the `platforms` job's `test:napi:npm`), in the `artifacts`
 * job, on every release build row, `win32-x64` included (through `scripts/pgo/pgo.ts stage`),
 * and in the release `publish` job (`--loader-only`), which holds the npm credentials and so
 * installs nothing (docs/tasks.md §`build:napi:packages`). The root config's Deno walk
 * (`typecheck:scripts`) checks this module with no `node_modules` in view, so a package
 * import here or beneath it fails `check`.
 *
 * The checkout is this module's own location, never the working directory. Each staged copy
 * is `copyFile` (the source's mode kept) then `utimes`; the CLI binary's mode is `chmod`, which
 * the library skips on Windows; the host triple is Node's platform and CPU, `gnu` or `musl` by
 * whether the runtime reports a glibc.
 *
 * @module
 */

import { chmod, copyFile, mkdir, readFile, stat, utimes, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

import { run_build_napi_packages } from '../build_napi_packages.ts';
import { node_host_triple } from '../napi_host.ts';

const checkout = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');
const report = process.report?.getReport() as { header?: { glibcVersionRuntime?: string } };

process.exitCode = await run_build_napi_packages(
	{
		checkout,
		platform: process.platform,
		host_triple: node_host_triple({
			platform: process.platform,
			arch: process.arch,
			glibc: Boolean(report?.header?.glibcVersionRuntime)
		}),
		fs: {
			read_text: (path) => readFile(path, 'utf8'),
			write_text: (path, text) => writeFile(path, text),
			make_dir: async (path) => {
				await mkdir(path, { recursive: true });
			},
			copy: (from, to) => copyFile(from, to),
			set_times: (path, { mtime_ms, atime_ms = mtime_ms }) =>
				utimes(path, new Date(atime_ms), new Date(mtime_ms)),
			set_mode: (path, mode) => chmod(path, mode),
			size: async (path) => (await stat(path)).size
		},
		print: (line) => console.log(line),
		print_error: (line) => console.error(line)
	},
	process.argv.slice(2)
);
