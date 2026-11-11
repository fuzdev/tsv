/**
 * `docs:audit:docsrs`' root (`scripts/docsrs_audit.ts`). Its reach: the checkout read-only,
 * `target/docsrs` read-write (made when missing, opened through `target/` wherever it leads:
 * `open_target`), and `cargo` with cargo's variables
 * (`CARGO_PASS_ENV`) and the gate's `RUSTDOCFLAGS` and `CARGO_TARGET_DIR`, which replace the
 * host's.
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

import { CARGO_PASS_ENV } from '../cargo_env.ts';
import {
	DOCSRS_DIR,
	DOCSRS_RUSTDOCFLAGS,
	DOCSRS_TARGET_DIR,
	run_docsrs_audit
} from '../docsrs_audit.ts';
import { open_checkout, open_target } from './checkout.ts';

const checkout = await open_checkout();
const target = await open_target(checkout);
await target.mkdir(DOCSRS_DIR, { recursive: true });
const code = await run_docsrs_audit(
	{
		checkout: checkout.readonly(),
		docsrs: target.dir(DOCSRS_DIR),
		cargo: spawn.program('cargo', {
			pass_env: CARGO_PASS_ENV,
			env: { RUSTDOCFLAGS: DOCSRS_RUSTDOCFLAGS, CARGO_TARGET_DIR: DOCSRS_TARGET_DIR }
		})
	},
	process.args
);
process.set_exit_code(code);
