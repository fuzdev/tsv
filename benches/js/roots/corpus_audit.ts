/**
 * `audit:corpus`'s root (`benches/js/corpus_audit.ts`). Its reach: the `robustness` view's
 * seeds, found by the corpus loader, which Bun's bench runs too and so reads the corpus and
 * runs `git` for the live diff through `node:*`, a named exception whose reason is Bun; the
 * `../prettier` checkout, read-only, for which of its suites are present, opened by its path
 * (`sibling_reads`), so a checkout that is a symlink to a directory elsewhere is followed; and
 * `cargo` with cargo's variables (`CARGO_PASS_ENV`), run in the checkout with its output
 * inherited, which builds `tsv_debug` and runs each audit leg. It writes nothing itself. Run it from the
 * checkout's root, where the corpus paths and the sibling checkouts resolve.
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

import { CARGO_PASS_ENV } from '../../../scripts/cargo_env.ts';
import { PRETTIER_CHECKOUT } from '../../../scripts/roundtrip_audit_prettier.ts';
import { open_checkout } from '../../../scripts/roots/checkout.ts';
import { run_corpus_audit } from '../corpus_audit.ts';
import { corpus_robustness_seeds } from '../lib/corpus.ts';
import { sibling_reads } from './harvest_deps.ts';

const checkout = await open_checkout();
const cargo = spawn.program('cargo', { pass_env: CARGO_PASS_ENV });

const code = await run_corpus_audit({
	seeds: corpus_robustness_seeds,
	prettier: sibling_reads(PRETTIER_CHECKOUT),
	cargo: { run: (args) => cargo.run(args, { cwd: checkout }) }
});
process.exit(code);
