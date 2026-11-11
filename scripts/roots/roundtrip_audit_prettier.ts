/**
 * `roundtrip:audit:prettier`'s root (`scripts/roundtrip_audit_prettier.ts`). Its reach: the
 * checkout and the directory it sits in, read-only, and `cargo` with cargo's variables
 * (`CARGO_PASS_ENV`), which builds and runs `tsv_debug roundtrip_audit` (it reads no other).
 *
 * @module
 */

import { dirname } from 'node:path';

import { fs } from '@fuzdev/gro/fs.ts';
import { process } from '@fuzdev/gro/process.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

import { CARGO_PASS_ENV } from '../cargo_env.ts';
import { run_roundtrip_audit_prettier } from '../roundtrip_audit_prettier.ts';
import { open_checkout } from './checkout.ts';

const checkout = await open_checkout();
const code = await run_roundtrip_audit_prettier({
	checkout: checkout.readonly(),
	siblings: (await fs.open_dir(dirname(checkout.path))).readonly(),
	cargo: spawn.program('cargo', { pass_env: CARGO_PASS_ENV })
});
process.set_exit_code(code);
