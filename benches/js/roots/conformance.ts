/**
 * The `conformance` aggregate's driver's root (`benches/js/conformance.ts`). Its reach is its
 * legs': tsv's N-API addon at the `corpus` profile and the canonical oracles, loaded in
 * process; the corpus, read, and `benches/js/.cache`, written, by the corpus loader and the
 * prettier-output cache through `node:*`; the suite checkouts, read through read-only handles;
 * `HOME`; both pins files, read only (the driver never re-pins); the checkout's
 * `target/corpus/tsv_debug`, run in the checkout with gro's base environment and the panic
 * hook's backtrace switches; and `cargo`, run in the checkout with stdio inherited and gro's
 * base environment plus cargo's variables (`CARGO_PASS_ENV`), for the render audit. Its
 * packages resolve from `benches/js/node_modules`, gro from the checkout's own install above
 * it.
 *
 * It runs from the checkout's root, where the corpus paths resolve, and refuses any other working
 * directory first, before the freshness guard or the corpus loader runs
 * (`exit_unless_checkout_cwd`).
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

import { CARGO_PASS_ENV } from '../../../scripts/cargo_env.ts';
import {
	exit_unless_checkout_cwd,
	open_checkout,
	open_checkout_program
} from '../../../scripts/roots/checkout.ts';
import { run_conformance } from '../conformance.ts';
import {
	exit_with,
	init_implementations,
	open_readonly_dir,
	read_home,
	readonly_pins_file,
	tsv_debug_opener
} from './compare_deps.ts';

const checkout = await open_checkout();
exit_unless_checkout_cwd(checkout);
const lib = checkout.readonly().dir('benches/js/lib');
const home = read_home();
const stdout = process.stdout;
const code = await run_conformance(
	{
		fixtures: { init_implementations, open_dir: open_readonly_dir, stdout },
		ts_repo: { init_implementations, open_dir: open_readonly_dir, stdout },
		parse: {
			init_implementations,
			open_tsv_debug: tsv_debug_opener(await open_checkout_program(checkout)),
			home,
			stdout,
			pins: readonly_pins_file(lib, 'corpus_parse_pins.txt')
		},
		format: {
			init_implementations,
			home,
			stdout,
			pins: readonly_pins_file(lib, 'corpus_format_pins.txt')
		},
		cargo: spawn.program('cargo', { pass_env: CARGO_PASS_ENV })
	},
	process.args
);
exit_with(code);
