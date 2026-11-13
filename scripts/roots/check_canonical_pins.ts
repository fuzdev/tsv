/**
 * `pins:audit` and `pins:audit:checkouts`' root (`scripts/check_canonical_pins.ts`). Its reach:
 * the checkout read-only; with `--checkouts` (or no flag), also the sibling checkouts
 * read-only, each opened by its path (`open_sibling`), so one that is a symlink to a directory
 * elsewhere is followed, and `git`, found on `PATH`, with git's local variables
 * (`GIT_LOCAL_PASS_ENV`), never reading a legacy `info/grafts` file (`GIT_GRAFT_FILE`).
 * `--pins` alone gets neither.
 *
 * @module
 */

import { GIT_LOCAL_PASS_ENV } from '@fuzdev/gro/git.ts';
import { process } from '@fuzdev/gro/process.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

import { parse_pins_args, run_check_canonical_pins } from '../check_canonical_pins.ts';
import { open_checkout, open_sibling } from './checkout.ts';

const modes = parse_pins_args(process.args);
if ('error' in modes) {
	console.error(modes.error);
	process.exit(1);
}
const checkout = await open_checkout();
const code = await run_check_canonical_pins(
	{
		checkout: checkout.readonly(),
		siblings: modes.run_checkouts ? { open: open_sibling } : null,
		// no legacy `info/grafts` rewrite of the clone's history: an empty graft file path names no
		// file, so git reads none and prints no deprecation hint
		git: modes.run_checkouts
			? spawn.find('git', {
					pass_env: GIT_LOCAL_PASS_ENV,
					env: { GIT_GRAFT_FILE: '' }
				})
			: null
	},
	modes
);
process.set_exit_code(code);
