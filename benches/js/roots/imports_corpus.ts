/**
 * `imports:corpus:run`'s root (`benches/js/imports_corpus.ts`). Its reach: the corpus and the
 * harness's `node_modules`, read by the library, and the checkout's `target/corpus/tsv_debug`,
 * run in the checkout with gro's base environment and the panic hook's backtrace switches
 * (`TSV_DEBUG_PASS_ENV`). Its packages resolve from `benches/js/node_modules`, gro from the
 * checkout's own install above it.
 *
 * It runs from the checkout's root, where the corpus paths resolve, and refuses any other working
 * directory first, before the freshness guard or the corpus loader runs
 * (`exit_unless_checkout_cwd`).
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';

import {
	exit_unless_checkout_cwd,
	open_checkout,
	open_checkout_program
} from '../../../scripts/roots/checkout.ts';
import { run_imports_corpus } from '../imports_corpus.ts';
import { TSV_DEBUG_CORPUS, TSV_DEBUG_PASS_ENV } from '../lib/tsv_debug_server.ts';

const checkout = await open_checkout();
exit_unless_checkout_cwd(checkout);
const checkout_program = await open_checkout_program(checkout);
const code = await run_imports_corpus({
	open_tsv_debug: () => checkout_program(TSV_DEBUG_CORPUS, { pass_env: TSV_DEBUG_PASS_ENV })
});
process.set_exit_code(code);
