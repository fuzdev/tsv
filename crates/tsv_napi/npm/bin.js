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
import { existsSync } from 'node:fs';
import { createRequire } from 'node:module';
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
		// rather than failing a run the fallback can serve
		process.stderr.write(
			`warning: @fuzdev/tsv could not run its native CLI at ${bin} ` +
				`(${result.error.message}); falling back to the JS CLI\n`
		);
		await import('./cli.js');
	} else if (result.signal) {
		// the child died by signal — re-raise it so the parent's exit status
		// reports the same signal death (128+n) instead of a plain exit code
		process.kill(process.pid, result.signal);
	} else {
		process.exit(result.status ?? 1);
	}
}
