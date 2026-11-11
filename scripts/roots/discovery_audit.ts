/**
 * `discovery:audit`'s root (`scripts/discovery_audit.ts`). Its reach: the checkout and the
 * directory it sits in, read-only; `git`, when one is on `PATH`, with git's local variables
 * (`GIT_LOCAL_PASS_ENV`); and the checkout's `target/corpus/tsv` with gro's base environment
 * alone (tsv reads none but `NO_COLOR` and `FORCE_COLOR`, which it holds).
 *
 * @module
 */

import { dirname } from 'node:path';

import { fs } from '@fuzdev/gro/fs.ts';
import { GIT_LOCAL_PASS_ENV } from '@fuzdev/gro/git.ts';
import { process } from '@fuzdev/gro/process.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

import { DISCOVERY_TSV_BIN, run_discovery_audit } from '../discovery_audit.ts';
import { checkout_program, open_checkout } from './checkout.ts';

const checkout = await open_checkout();
const code = await run_discovery_audit({
	checkout: checkout.readonly(),
	siblings: (await fs.open_dir(dirname(checkout.path))).readonly(),
	git: spawn.find('git', { pass_env: GIT_LOCAL_PASS_ENV }),
	tsv: checkout_program(checkout, DISCOVERY_TSV_BIN)
});
process.set_exit_code(code);
