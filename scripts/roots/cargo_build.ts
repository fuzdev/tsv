/**
 * `build:stamped`'s entry: `cargo build <args>` in the checkout, then each leaf artifact dated
 * (`scripts/cargo_build.ts`).
 *
 * A named exception to the host-module rule: this entry and everything it imports use Node's
 * own `node:*` modules and no package, because CI runs `build:stamped` on Windows, where gro's
 * host modules don't run programs, and in jobs that install no `node_modules` (the `platforms`
 * and `artifacts` jobs, through `test:napi` and `test:napi:npm`). The root config's Deno walk
 * (`typecheck:scripts`) checks this module with no `node_modules` in view, so a package import
 * here or beneath it fails `check`.
 *
 * Cargo is the bare name `cargo`, found on `PATH` by Node's `spawn` with no shell, and inherits
 * the whole environment. The checkout is this module's own location, never the working
 * directory. The stamp is `utimes` on the path beneath `target/`.
 *
 * @module
 */

import { spawn } from 'node:child_process';
import { utimes } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

import { type CargoExit, run_cargo_build } from '../cargo_build.ts';

const checkout = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');
const target = join(checkout, 'target');

process.exitCode = await run_cargo_build(
	{
		start_cargo: (args, cwd) => {
			const child = spawn('cargo', args, { cwd, stdio: ['inherit', 'pipe', 'inherit'] });
			const closed = new Promise<CargoExit>((resolve_closed, reject) => {
				child.once('error', reject);
				child.once('close', (code, signal) =>
					resolve_closed(
						signal === null ? { kind: 'exited', code: code ?? 1 } : { kind: 'signaled', signal }
					)
				);
			});
			// awaited after stdout ends; marked handled so a spawn failure isn't reported early
			closed.catch(() => {});
			return { stdout: child.stdout, closed };
		},
		checkout,
		target: {
			path: target,
			set_times: (path, { mtime_ms, atime_ms = mtime_ms }) =>
				utimes(join(target, path), new Date(atime_ms), new Date(mtime_ms))
		},
		print: (line) => console.log(line)
	},
	process.argv.slice(2)
);
