/**
 * Install the bench harness's npm deps. `package.json` is the single source of
 * truth for versions; both runtimes consume the resulting `node_modules` (Deno
 * via `nodeModulesDir: "manual"` in deno.json, Node directly). Runs the one
 * installer — `npm install` — then force-fetches the `force_installed` pins: the
 * packages npm will not install as ordinary deps, today only the pure-wasm
 * `@oxc-parser/binding-wasm32-wasi` binding (the oxc-parser WASM bench row), which
 * npm skips on a non-wasm32 host because its metadata declares `cpu: wasm32`.
 * Portable (node: builtins only), so `deno run` or `node` both drive it.
 *
 * Usage: deno task bench:install   (or: node benches/js/install_deps.ts)
 */
import { spawnSync } from 'node:child_process';
import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

import { read_force_installed_pins } from './lib/versions.ts';

const here = dirname(fileURLToPath(import.meta.url));

function npm(args: Array<string>): void {
	const r = spawnSync('npm', args, { cwd: here, stdio: 'inherit' });
	if (r.status !== 0) process.exit(r.status ?? 1);
}

// 1. Main install — everything in `dependencies`.
npm(['install']);

// 2. The `force_installed` pins. `--force` bypasses npm's cpu gate; `--no-save` keeps
//    them out of package.json/lock (where an optionalDependency entry would make a
//    later forced reinstall no-op as "up to date") — which also means each entry's
//    transitive closure resolves live here, unlocked (`package.json`
//    `//force_installed`). ONE command for every entry: an `npm install` prunes
//    extraneous packages, so a second `--no-save` install would remove what the
//    first one fetched. The pins are read through `versions.ts` — it refuses a
//    non-exact entry, `check_node_modules.ts` grades what this installs, and the
//    reports label the rows with the same pins, so all three read one map. No
//    per-package presence check: `load_all_versions` throws on a missing entry it
//    reads, which is the one posture (a missing pin is a bug, not a degraded run).
const forced = await read_force_installed_pins();
const specs = Object.entries(forced).map(([name, version]) => `${name}@${version}`);
if (specs.length > 0) npm(['install', ...specs, '--force', '--no-save']);
