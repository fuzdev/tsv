/**
 * `strip:corpus:run`'s root (`benches/js/strip_corpus.ts`). Its reach: the corpus and the
 * harness's `node_modules`, read by the library, and the checkout's `target/corpus/tsv_debug`,
 * run in the checkout with gro's base environment and the panic hook's backtrace switches
 * (`TSV_DEBUG_PASS_ENV`). Its packages resolve from `benches/js/node_modules`, gro from the
 * checkout's own install above it.
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';

import { checkout_program, open_checkout } from '../../../scripts/roots/checkout.ts';
import { TSV_DEBUG_CORPUS, TSV_DEBUG_PASS_ENV } from '../lib/tsv_debug_server.ts';
import { run_strip_corpus } from '../strip_corpus.ts';

const checkout = await open_checkout();
const code = await run_strip_corpus({
	open_tsv_debug: () =>
		checkout_program(checkout, TSV_DEBUG_CORPUS, { pass_env: TSV_DEBUG_PASS_ENV })
});
process.set_exit_code(code);
