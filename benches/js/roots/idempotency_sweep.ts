/**
 * `idempotency:sweep`'s root (`benches/js/idempotency_sweep.ts`). Its reach: the `robustness`
 * view's seeds, found by the corpus loader, which Bun's bench runs too and so reads the corpus
 * and runs `git` for the live diff through `node:*`, a named exception whose reason is Bun; and
 * `cargo` with cargo's variables (`CARGO_PASS_ENV`), run in the checkout with its output
 * inherited, which builds `tsv_debug` and runs its `fuzz` pass over the seeds. It writes
 * nothing itself. Run it from the checkout's root, where the corpus paths resolve.
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

import { CARGO_PASS_ENV } from '../../../scripts/cargo_env.ts';
import { open_checkout } from '../../../scripts/roots/checkout.ts';
import { run_idempotency_sweep } from '../idempotency_sweep.ts';
import { corpus_robustness_seeds } from '../lib/corpus.ts';

const checkout = await open_checkout();
const cargo = spawn.program('cargo', { pass_env: CARGO_PASS_ENV });

const code = await run_idempotency_sweep({
	seeds: corpus_robustness_seeds,
	cargo: { run: (args) => cargo.run(args, { cwd: checkout }) }
});
process.exit(code);
