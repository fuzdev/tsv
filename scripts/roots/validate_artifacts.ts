/**
 * `validate:artifacts`' root (`scripts/validate_artifacts.ts`): the built WASM artifacts'
 * sizes and the npm packages' third-party notice. Its reach:
 *
 * - the checkout read-only, found from this module's location
 * - `BENCH_STALE_OK`, and no other variable
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';

import { run_validate_artifacts } from '../validate_artifacts.ts';
import { open_checkout } from './checkout.ts';

const checkout = await open_checkout();
const code = await run_validate_artifacts({
	checkout: checkout.readonly(),
	stale_ok: process.env.narrow(['BENCH_STALE_OK']).get('BENCH_STALE_OK') === '1',
	print: (line) => console.log(line),
	print_error: (line) => console.error(line)
});
process.set_exit_code(code);
