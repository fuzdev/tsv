/**
 * `corpus:compare:parse:run`'s root (`benches/js/corpus_compare_parse.ts`). Its reach: tsv's
 * N-API addon at the `corpus` profile and the canonical oracles, loaded in process; the corpus,
 * read by the corpus loader through `node:*`; `HOME`; `benches/js/lib/corpus_parse_pins.txt`,
 * which `--update` rewrites; and the checkout's `target/corpus/tsv_debug`, run in the checkout
 * with gro's base environment and the panic hook's backtrace switches (`TSV_DEBUG_PASS_ENV`).
 * Its packages resolve from `benches/js/node_modules`, gro from the checkout's own install
 * above it.
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
import { corpus_compare_parse_main } from '../corpus_compare_parse.ts';
import {
	exit_with,
	init_implementations,
	pins_file,
	read_home,
	tsv_debug_opener
} from './compare_deps.ts';

const checkout = await open_checkout();
exit_unless_checkout_cwd(checkout);
const code = await corpus_compare_parse_main(
	{
		init_implementations,
		open_tsv_debug: tsv_debug_opener(await open_checkout_program(checkout)),
		home: read_home(),
		stdout: process.stdout,
		pins: pins_file(checkout.dir('benches/js/lib'), 'corpus_parse_pins.txt')
	},
	process.args
);
exit_with(code);
