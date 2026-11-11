/**
 * `deps:audit`'s root (`scripts/deps_audit.ts`). Its reach: the checkout read-only and `cargo`
 * with cargo's variables (`CARGO_PASS_ENV`).
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

import { CARGO_PASS_ENV } from '../cargo_env.ts';
import { run_deps_audit } from '../deps_audit.ts';
import { open_checkout } from './checkout.ts';

const checkout = await open_checkout();
const code = await run_deps_audit({
	checkout: checkout.readonly(),
	cargo: spawn.program('cargo', { pass_env: CARGO_PASS_ENV })
});
process.set_exit_code(code);
