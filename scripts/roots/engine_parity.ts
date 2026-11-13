/**
 * `engines:audit`' root (`scripts/engine_parity.ts`): the WASM and native engines, graded for
 * agreeing on every byte. Its reach:
 *
 * - the checkout read-only, found from this module's location, the native CLI's and the strip
 *   leg's working directory
 * - each corpus root read-only, opened by its path resolved against the checkout
 * - temp directories, for the two trees each root is formatted in and the in-process half's
 *   consumer project
 * - `node`, found on `PATH`, and the native CLI by its path beneath the checkout, each with
 *   gro's base environment, since neither CLI reads a variable of its own
 * - `BENCH_STALE_OK`, and no other variable
 *
 * The host's platform and CPU, which pick the staged platform package to grade first, come
 * from `node:process`, a named exception to the host-module rule: no host module reports them.
 *
 * @module
 */

import { resolve } from 'node:path';
import node_process from 'node:process';

import { fs } from '@fuzdev/gro/fs.ts';
import { is_fs_error } from '@fuzdev/gro/fs_error.ts';
import { process } from '@fuzdev/gro/process.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

import { run_engine_parity } from '../engine_parity.ts';
import { node_host_triple } from '../napi_host.ts';
import { open_checkout, open_checkout_program } from './checkout.ts';

const checkout = await open_checkout();
const checkout_program = await open_checkout_program(checkout);
const report = node_process.report?.getReport() as { header?: { glibcVersionRuntime?: string } };
const code = await run_engine_parity(
	{
		checkout: checkout.readonly(),
		open_root: async (root) => {
			try {
				return (await fs.open_dir(resolve(checkout.path, root))).readonly();
			} catch (error) {
				if (is_fs_error(error, 'not_found')) return null;
				throw error;
			}
		},
		temp_dir: () => fs.temp_dir('tsv_engines_'),
		host_triple: node_host_triple({
			platform: node_process.platform,
			arch: node_process.arch,
			glibc: Boolean(report?.header?.glibcVersionRuntime)
		}),
		native_program: (path) => checkout_program(path),
		node: spawn.find('node'),
		stale_ok: process.env.narrow(['BENCH_STALE_OK']).get('BENCH_STALE_OK') === '1',
		print: (line) => console.log(line),
		print_error: (line) => console.error(line)
	},
	process.args
);
process.set_exit_code(code);
