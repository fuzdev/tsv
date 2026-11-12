/**
 * `bench:harvest:prettier-jsx`'s root (`benches/js/diagnostics/prettier_jsx_harvest.ts`). Its
 * reach: the conformance corpus's Prettier JS suite, read through the corpus loader, which
 * Bun's bench runs too and so reads with `node:fs`, a named exception whose reason is Bun; the
 * checkout's files, read-only, and the JSX list and the stamp it writes beneath
 * `benches/js/.cache`; `git` with git's local environment, for `../prettier`'s commit; and
 * `prettier`, the oracle, imported in process from the bench install. It runs no other
 * program. Run it from the checkout's root, where the corpus paths and the sibling checkouts
 * resolve.
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';

import { open_checkout } from '../../../scripts/roots/checkout.ts';
import {
	type PrettierDebugParse,
	run_prettier_jsx_harvest
} from '../diagnostics/prettier_jsx_harvest.ts';
import { corpus_view_paths, load_pinned_language_corpus } from '../lib/corpus.ts';
import { cache_files, stamp_git } from './harvest_deps.ts';

const checkout = await open_checkout();

const code = await run_prettier_jsx_harvest(
	{
		git: stamp_git(checkout),
		files: cache_files(checkout),
		corpus: { view_paths: corpus_view_paths, load: load_pinned_language_corpus },
		import_prettier: async () => (await import('prettier')) as unknown as PrettierDebugParse
	},
	process.args
);
process.exit(code);
