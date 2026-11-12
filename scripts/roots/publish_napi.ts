/**
 * `publish:napi`'s entry: the staged N-API npm packages checked and published, platforms then
 * loader (`scripts/publish_napi.ts`).
 *
 * A named exception to the host-module rule: this entry and everything it imports use Node's
 * own `node:*` modules and no package, because the release workflow's `publish` job runs it
 * holding the npm token and `id-token: write`, and so installs nothing (docs/tasks.md
 * §`publish:napi`). `scripts/no_package_closure_test.ts` (in `test:node`) holds this module and
 * everything beneath it to no package import.
 *
 * The checkout is this module's own location, never the working directory. It reads the staged
 * packages under `crates/tsv_napi/pkg/`, at directories named by the loader's
 * `optionalDependencies`, sets the CLI binaries' mode there (`chmod`, which the library skips on
 * Windows; the one write, refused outside that directory), and runs `npm` by name through `PATH`
 * with no shell, in the checkout, with the whole environment (npm's auth and provenance read the
 * runner's variables). Its deps are `create_publish_napi_deps`' (`scripts/release/node_host.ts`).
 *
 * @module
 */

import process from 'node:process';

import { run_publish_napi } from '../publish_napi.ts';
import { create_publish_napi_deps, entry_checkout } from '../release/node_host.ts';

process.exitCode = await run_publish_napi(
	create_publish_napi_deps(entry_checkout(import.meta.url)),
	process.argv.slice(2)
);
