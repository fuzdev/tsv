/**
 * `tape:corpus:run`'s and `tape-hot:update`'s root (`benches/js/tape_corpus.ts`). Its reach: the
 * corpus, read by the library, the checkout's schema files, the hot-shape list written under
 * `--update-hot`, and the checkout's `target/corpus/tsv_debug`, run in the checkout with gro's
 * base environment and the panic hook's backtrace switches (`TSV_DEBUG_PASS_ENV`).
 *
 * It runs from the checkout's root, where the corpus and schema paths resolve, and refuses any
 * other working directory first (`exit_unless_checkout_cwd`).
 *
 * @module
 */

import { args } from '@fuzdev/gro/args.ts';
import { set_exit_code } from '@fuzdev/gro/exit.ts';

import { TAPE_HOT_PATH } from '../../../scripts/tape_schema.ts';

import { exit_unless_checkout_cwd, open_checkout } from '../../../scripts/roots/checkout.ts';
import { open_checkout_program } from '../../../scripts/roots/checkout_program.ts';
import { TSV_DEBUG_CORPUS, TSV_DEBUG_PASS_ENV } from '../lib/tsv_debug_server.ts';
import { run_tape_corpus } from '../tape_corpus.ts';

const checkout = await open_checkout();
exit_unless_checkout_cwd(checkout);
const checkout_program = await open_checkout_program(checkout);
set_exit_code(
	await run_tape_corpus(
		{
			open_tsv_debug: () => checkout_program(TSV_DEBUG_CORPUS, { pass_env: TSV_DEBUG_PASS_ENV }),
			write_hot_list: (text) => checkout.write(TAPE_HOT_PATH, text)
		},
		args
	)
);
