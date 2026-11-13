/**
 * `discovery:audit`'s root (`scripts/discovery_audit.ts`). Its reach: the checkout read-only;
 * the `../corpora` checkout read-only, opened by its path (`sibling_reads`), so one that is a
 * symlink to a directory elsewhere is followed; `git`, when one is on `PATH`, with git's local
 * variables (`GIT_LOCAL_PASS_ENV`); and the checkout's `target/corpus/tsv` with gro's base
 * environment alone (tsv reads none but `NO_COLOR` and `FORCE_COLOR`, which it holds).
 *
 * @module
 */

import { GIT_LOCAL_PASS_ENV } from '@fuzdev/gro/git.ts';
import { process } from '@fuzdev/gro/process.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

import { CORPORA_ROOT } from '../../benches/js/lib/corpora.ts';
import { DISCOVERY_TSV_BIN, run_discovery_audit } from '../discovery_audit.ts';
import { open_checkout, open_checkout_program, sibling_reads } from './checkout.ts';

const checkout = await open_checkout();
const checkout_program = await open_checkout_program(checkout);
const code = await run_discovery_audit({
	checkout: checkout.readonly(),
	corpora: sibling_reads(CORPORA_ROOT),
	git: spawn.find('git', { pass_env: GIT_LOCAL_PASS_ENV }),
	tsv: checkout_program(DISCOVERY_TSV_BIN)
});
process.set_exit_code(code);
