/**
 * `pins:audit` and `pins:audit:checkouts`' root (`scripts/check_canonical_pins.ts`). Its reach:
 * the checkout read-only; with `--checkouts` (or no flag), also the directory the checkout sits
 * in read-only and `git`, found on `PATH`, with git's local variables (`GIT_LOCAL_PASS_ENV`).
 * `--pins` alone gets neither.
 *
 * @module
 */

import { dirname } from 'node:path';

import { fs } from '@fuzdev/gro/fs.ts';
import { GIT_LOCAL_PASS_ENV } from '@fuzdev/gro/git.ts';
import { process } from '@fuzdev/gro/process.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

import { parse_pins_args, run_check_canonical_pins } from '../check_canonical_pins.ts';
import { open_checkout } from './checkout.ts';

const modes = parse_pins_args(process.args);
if ('error' in modes) {
	console.error(modes.error);
	process.exit(1);
}
const checkout = await open_checkout();
const code = await run_check_canonical_pins(
	{
		checkout: checkout.readonly(),
		siblings: modes.run_checkouts ? (await fs.open_dir(dirname(checkout.path))).readonly() : null,
		git: modes.run_checkouts ? spawn.find('git', { pass_env: GIT_LOCAL_PASS_ENV }) : null
	},
	modes
);
process.set_exit_code(code);
