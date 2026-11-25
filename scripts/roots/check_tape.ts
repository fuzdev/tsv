/**
 * `check:tape`'s and `tape-shapes:update`'s root (`scripts/check_tape.ts`). Its reach: the checkout,
 * read for the table and the tape's shape list and written by `--update-shapes`, and the
 * checkout's `target/corpus/tsv_debug` with gro's base environment alone (`tape` reads none).
 *
 * @module
 */

import { args } from '@fuzdev/gro/args.ts';
import { set_exit_code } from '@fuzdev/gro/exit.ts';

import { CHECK_TAPE_TSV_DEBUG, run_check_tape } from '../check_tape.ts';
import { open_checkout } from './checkout.ts';
import { open_checkout_program } from './checkout_program.ts';

const checkout = await open_checkout();
const checkout_program = await open_checkout_program(checkout);
set_exit_code(
	await run_check_tape({ checkout, tsv_debug: checkout_program(CHECK_TAPE_TSV_DEBUG) }, args)
);
