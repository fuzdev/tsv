/**
 * `roundtrip:audit:prettier`'s root (`scripts/roundtrip_audit_prettier.ts`). Its reach: the
 * checkout read-only; the `../prettier` checkout read-only, opened by its path
 * (`sibling_reads`), so one that is a symlink to a directory elsewhere is followed; and `cargo`
 * with cargo's variables (`CARGO_PASS_ENV`), which builds and runs `tsv_debug roundtrip_audit`
 * (it reads no other).
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

import { CARGO_PASS_ENV } from '../cargo_env.ts';
import { PRETTIER_CHECKOUT, run_roundtrip_audit_prettier } from '../roundtrip_audit_prettier.ts';
import { open_checkout, sibling_reads } from './checkout.ts';

const checkout = await open_checkout();
const code = await run_roundtrip_audit_prettier({
	checkout: checkout.readonly(),
	prettier: sibling_reads(PRETTIER_CHECKOUT),
	cargo: spawn.program('cargo', { pass_env: CARGO_PASS_ENV })
});
process.set_exit_code(code);
