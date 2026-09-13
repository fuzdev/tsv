#!/usr/bin/env node
/**
 * The `tsv` bin of `@fuzdev/tsv` — a dispatcher, not a second CLI: it execs
 * the real `tsv_cli` binary shipped beside the addon in the platform package
 * (`@fuzdev/tsv-<triple>`), forwarding argv, stdio, exit codes, and signals
 * verbatim — so `npx tsv` here IS the native CLI (real `--jobs` parallelism,
 * native discovery and error paths), the esbuild/biome shape. When no binary
 * is reachable it defers to `./cli.js`, the shared JS mirror of the same
 * contract that `@fuzdev/tsv_wasm` ships as its bin — on an unsupported
 * platform that path ends at the loader's error pointing at the WASM package.
 *
 * The binary is resolved from this package's own optionalDependency, never
 * from PATH, and the probe never loads the addon — dispatch costs ~1 ms on
 * top of Node's own startup.
 */

import { spawnSync } from 'node:child_process';
import { existsSync, writeSync } from 'node:fs';
import { createRequire } from 'node:module';
import { constants } from 'node:os';
import { dirname, join } from 'node:path';
import { platform_triple } from './platform.js';

const require = createRequire(import.meta.url);

/** The platform package's native CLI binary, or undefined (unsupported
 * platform, or a platform package without the binary). */
const native_cli_path = () => {
	let addon_path;
	try {
		addon_path = require.resolve(`@fuzdev/tsv-${platform_triple()}`);
	} catch {
		return undefined;
	}
	const bin = join(dirname(addon_path), process.platform === 'win32' ? 'tsv.exe' : 'tsv');
	return existsSync(bin) ? bin : undefined;
};

const bin = native_cli_path();
if (bin === undefined) {
	// no binary to dispatch to — run the JS mirror (which reports the
	// unsupported platform via the loader's own error if the whole platform
	// package is absent)
	await import('./cli.js');
} else {
	const result = spawnSync(bin, process.argv.slice(2), { stdio: 'inherit' });
	if (result.error) {
		// present-but-unrunnable binary (lost executable bit, torn install) —
		// the JS mirror implements the same contract, so degrade to it loudly
		// rather than failing a run the fallback can serve. A synchronous write, as
		// every line `cli.js` prints: the stream API is asynchronous on a pipe, and the
		// mirror this hands off to exits synchronously. Best-effort, as `cli.js`'s own
		// stderr notice of a failed write is: the fd is still as inherited (nothing here
		// touches `process.stderr` or spawns a pool, the two things that flip it
		// non-blocking), but a closed stderr throws, and the run the fallback can serve
		// matters more than the notice. `bin` is this package's own install path, not a
		// name a user typed, and Node's error spells it raw beside it — so it prints as
		// it is, where a path from the command line would be quoted (`cli.js`'s
		// `quote_path`, which the dispatcher cannot reach without loading the engine)
		try {
			writeSync(
				2,
				`warning: @fuzdev/tsv could not run its native CLI at ${bin} ` +
					`(${result.error.message}); falling back to the JS CLI — reinstall ` +
					`@fuzdev/tsv-${platform_triple()} (or restore the executable bit) to get the native one back\n`
			);
		} catch {
			// a closed or unwritable stderr: the fallback below is the run's answer
		}
		await import('./cli.js');
	} else if (result.signal) {
		// The child died by signal: re-raise it, so this process dies the same way. Not
		// SIGUSR1, which starts Node's inspector instead; and a re-raise Node survives
		// (SIGPIPE is ignored) or cannot make falls through to the shell's spelling of a
		// signal death, 128 + its number — never exit 0.
		const number = constants.signals[result.signal];
		if (result.signal !== 'SIGUSR1') {
			try {
				process.kill(process.pid, result.signal);
			} catch {
				// a signal this host cannot raise — the exit below reports it
			}
		}
		process.exit(number === undefined ? 1 : 128 + number);
	} else {
		process.exit(result.status ?? 1);
	}
}
