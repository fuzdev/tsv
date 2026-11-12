/**
 * `bench:harvest:test262`'s root (`benches/js/harvest_test262.ts`). Its reach: the `../test262`
 * checkout, read-only, for its presence; the checkout's files, read-only, and the path list
 * and the stamp it writes beneath `benches/js/.cache`; `git` with git's local environment, for
 * the checkout's commit; and `cargo` with cargo's variables (`CARGO_PASS_ENV`), run in the
 * checkout with its output inherited, which builds `tsv_debug` and runs its `test262` grade,
 * which writes the manifest beneath `benches/js/.cache` itself. Run it from the checkout's
 * root, where `../test262` and the cache paths resolve.
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

import { CARGO_PASS_ENV } from '../../../scripts/cargo_env.ts';
import { open_checkout } from '../../../scripts/roots/checkout.ts';
import { run_harvest_test262, TEST262_ROOT } from '../harvest_test262.ts';
import { cache_files, sibling_reads, stamp_git } from './harvest_deps.ts';

const checkout = await open_checkout();

const code = await run_harvest_test262(
	{
		git: stamp_git(checkout),
		files: cache_files(checkout),
		test262: sibling_reads(TEST262_ROOT),
		// made on the call, so a run that stops before the grade never looks for `cargo`
		cargo: {
			run: (args) =>
				spawn.program('cargo', { pass_env: CARGO_PASS_ENV }).run(args, { cwd: checkout })
		}
	},
	process.args
);
process.exit(code);
