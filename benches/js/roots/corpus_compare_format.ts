/**
 * `corpus:compare:format:run`'s root (`benches/js/corpus_compare_format.ts`). Its reach: tsv's
 * N-API addon at the `corpus` profile and the canonical oracles, loaded in process; the corpus,
 * read, and `benches/js/.cache`, written, by the corpus loader and the prettier-output cache
 * through `node:*`; `HOME`; and `benches/js/lib/corpus_format_pins.txt`, which `--update`
 * rewrites. It runs no program. Its packages resolve from `benches/js/node_modules`, gro from
 * the checkout's own install above it.
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';

import { open_checkout } from '../../../scripts/roots/checkout.ts';
import { corpus_compare_format_main } from '../corpus_compare_format.ts';
import { exit_with, init_implementations, pins_file, read_home } from './compare_deps.ts';

const checkout = await open_checkout();
const code = await corpus_compare_format_main(
	{
		init_implementations,
		home: read_home(),
		stdout: process.stdout,
		pins: pins_file(checkout.dir('benches/js/lib'), 'corpus_format_pins.txt')
	},
	process.args
);
exit_with(code);
