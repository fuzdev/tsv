/**
 * `docs:audit:docsrs`' root (`scripts/docsrs_audit.ts`). Its reach: the checkout read-only,
 * `target/docsrs` read-write (made when missing, opened through `target/` wherever it leads:
 * `open_target`), and `cargo` with cargo's variables
 * (`CARGO_PASS_ENV`) and the gate's `RUSTDOCFLAGS` and `CARGO_TARGET_DIR`, which replace the
 * host's. The same flags go in `CARGO_ENCODED_RUSTDOCFLAGS` too: cargo reads that variable
 * ahead of `RUSTDOCFLAGS`, even set empty, and `CARGO_PASS_ENV`'s `CARGO_*` would hand it a
 * host's, which would drop the gate's lints.
 *
 * @module
 */

import { args } from '@fuzdev/gro/args.ts';
import { set_exit_code } from '@fuzdev/gro/exit.ts';
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
			env: {
				RUSTDOCFLAGS: DOCSRS_RUSTDOCFLAGS,
				// cargo's encoding: one flag per field, fields split by U+001F, the whitespace
				// split cargo makes of `RUSTDOCFLAGS`
				CARGO_ENCODED_RUSTDOCFLAGS: DOCSRS_RUSTDOCFLAGS.split(/\s+/).filter(Boolean).join('\x1f'),
				CARGO_TARGET_DIR: DOCSRS_TARGET_DIR
			}
		})
	},
	args
);
set_exit_code(code);
