/**
 * `check:loc`'s root (`scripts/check_loc.ts`). Its reach: the checkout read-only and the
 * checkout's `target/corpus/tsv_debug` with gro's base environment alone (`loc_wires` reads
 * none).
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';

import { CHECK_LOC_TSV_DEBUG, run_check_loc } from '../check_loc.ts';
import { open_checkout, open_checkout_program } from './checkout.ts';

const checkout = await open_checkout();
const checkout_program = await open_checkout_program(checkout);
const code = await run_check_loc(
	{
		checkout: checkout.readonly(),
		tsv_debug: checkout_program(CHECK_LOC_TSV_DEBUG)
	},
	process.args
);
process.set_exit_code(code);
