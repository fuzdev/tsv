/**
 * `run_if_stale`'s root (`scripts/run_if_stale.ts`), run by the `build:wasm:*` tasks. Its reach:
 *
 * - the checkout read-only, found from this module's location, the command's working directory
 * - `wasm-pack` alone, found on `PATH` when the root starts, with gro's base environment and
 *   what a wasm build reads (`WASM_PACK_PASS_ENV`); a missing one fails only a run that builds
 * - `TSV_BUILD_FORCE`, and no other variable
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

import { run_if_stale } from '../run_if_stale.ts';
import { WASM_PACK_PASS_ENV } from '../wasm_pack_env.ts';
import { open_checkout } from './checkout.ts';

const checkout = await open_checkout();
const code = await run_if_stale(
	{
		checkout: checkout.readonly(),
		force: process.env.narrow(['TSV_BUILD_FORCE']).get('TSV_BUILD_FORCE') === '1',
		programs: new Map([['wasm-pack', spawn.find('wasm-pack', { pass_env: WASM_PACK_PASS_ENV })]]),
		print: (line) => console.log(line),
		print_error: (line) => console.error(line)
	},
	process.args
);
process.set_exit_code(code);
