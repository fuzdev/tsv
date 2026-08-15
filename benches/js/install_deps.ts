/**
 * Install the bench harness's npm deps. `package.json` is the single source of
 * truth for versions; both runtimes consume the resulting `node_modules` (Deno
 * via `nodeModulesDir: "manual"` in deno.json, Node directly). Runs the one
 * installer — `npm install` — then force-fetches the pure-wasm
 * `@oxc-parser/binding-wasm32-wasi` binding (the oxc-parser WASM bench row),
 * which npm skips on a non-wasm32 host because its metadata declares
 * `cpu: wasm32`. Portable (node: builtins only), so `deno run` or `node` both
 * drive it.
 *
 * Usage: deno task bench:install   (or: node benches/js/install_deps.ts)
 */
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { OXC_WASI_BINDING } from './lib/check_node_modules.ts';

const here = dirname(fileURLToPath(import.meta.url));

function npm(args: Array<string>): void {
	const r = spawnSync('npm', args, { cwd: here, stdio: 'inherit' });
	if (r.status !== 0) process.exit(r.status ?? 1);
}

// 1. Main install — everything in `dependencies` (the wasi binding in
//    `optionalDependencies` is skipped here on a non-wasm host, no error).
npm(['install']);

// 2. The oxc-parser WASM binding: pure-wasm, runtime-portable, but npm's cpu
//    gate skips it. `--force` bypasses the gate; `--no-save` keeps it out of
//    package.json/lock (where an optionalDependency entry would make a later
//    forced reinstall no-op as "up to date"). oxc ships every binding at the
//    oxc-parser version, so that pin is the single source of truth. The package
//    NAME is imported rather than spelled again — `check_node_modules.ts` grades
//    what this line installs, so the two must name the same package by
//    construction.
const pkg = JSON.parse(readFileSync(join(here, 'package.json'), 'utf8')) as {
	dependencies?: Record<string, string>;
};
const oxc_version = pkg.dependencies?.['oxc-parser'];
if (oxc_version) {
	npm(['install', `${OXC_WASI_BINDING}@${oxc_version}`, '--force', '--no-save']);
} else {
	console.error(
		`warning: oxc-parser not pinned in dependencies; ${OXC_WASI_BINDING} (oxc-parser-wasm row) will be unavailable`
	);
}
