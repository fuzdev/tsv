/**
 * `release:assets`' entry: a published version's native CLI binaries pulled back from the npm
 * registry, with their `SHA512SUMS` and the third-party NOTICE, the GitHub Release assets
 * (`scripts/release_assets.ts`).
 *
 * A named exception to the host-module rule: this entry and everything it imports use Node's
 * own `node:*` modules and no package, because the release workflow's `release` job runs it
 * holding the repository's `contents: write` and `id-token: write`, and so installs nothing
 * (docs/tasks.md §`release:assets`). The root config's Deno walk (`typecheck:scripts`) checks
 * this module with no `node_modules` in view, so a package import here or beneath it fails
 * `check`.
 *
 * The checkout is this module's own location, never the working directory; `--out` is read
 * against the working directory, as given. It writes beneath `--out` and one temp directory,
 * reads the checkout's NOTICE and the tarballs, and runs `npm` and `tar` by name through
 * `PATH` with no shell, in the working directory, with the whole environment. Its deps are
 * `create_release_assets_deps`' (`scripts/release/node_host.ts`).
 *
 * @module
 */

import process from 'node:process';

import { create_release_assets_deps, entry_checkout } from '../release/node_host.ts';
import { run_release_assets } from '../release_assets.ts';

process.exitCode = await run_release_assets(
	create_release_assets_deps(entry_checkout(import.meta.url)),
	process.argv.slice(2)
);
