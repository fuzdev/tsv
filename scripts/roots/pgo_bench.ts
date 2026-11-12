/**
 * `build:bench:pgo`'s root (`scripts/pgo/bench.ts`). Its reach:
 *
 * - the checkout, found from this module's location, the pipeline's working directory, with
 *   `.pgo/` beneath it read and its `bench/` written
 * - `node`, found on `PATH` when the root starts, with the whole environment: the pipeline's
 *   stamp key hashes the caller's cargo and rustc variables, so `build:bench:pgo` and
 *   `build:pgo` share `.pgo/` only while they see the same ones
 * - `TSV_BUILD_FORCE`, and no other variable
 *
 * The host's platform and CPU, which name the pipeline's build of each library, come from
 * `node:process`, a named exception to the host-module rule: no host module reports them.
 *
 * @module
 */

import node_process from 'node:process';

import { process } from '@fuzdev/gro/process.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

import { node_host_triple } from '../napi_host.ts';
import { run_pgo_bench } from '../pgo/bench.ts';
import { open_checkout } from './checkout.ts';

const report = node_process.report?.getReport() as { header?: { glibcVersionRuntime?: string } };
const code = await run_pgo_bench({
	checkout: await open_checkout(),
	node: spawn.program('node', { pass_env: 'all' }),
	host_triple: node_host_triple({
		platform: node_process.platform,
		arch: node_process.arch,
		glibc: Boolean(report?.header?.glibcVersionRuntime)
	}),
	force: process.env.narrow(['TSV_BUILD_FORCE']).get('TSV_BUILD_FORCE') === '1',
	now: () => new Date(),
	print: (line) => console.log(line),
	print_error: (line) => console.error(line)
});
process.set_exit_code(code);
