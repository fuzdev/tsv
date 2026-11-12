/**
 * `publish`'s entry: the wasm packages released, or the release dry-run
 * (`scripts/publish.ts`). It runs on gro's host modules; its deps, the checkout found from
 * this module's location and each program with the environment it needs, are
 * `create_publish_deps`' (`scripts/roots/publish_deps.ts`).
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';

import { run_publish } from '../publish.ts';
import { open_checkout } from './checkout.ts';
import { create_publish_deps } from './publish_deps.ts';

process.set_exit_code(await run_publish(create_publish_deps(await open_checkout()), process.args));
