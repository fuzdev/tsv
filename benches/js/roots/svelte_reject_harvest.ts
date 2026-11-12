/**
 * `bench:harvest:svelte-rejects`'s root (`benches/js/diagnostics/svelte_reject_harvest.ts`).
 * Its reach: the conformance corpus, read through the corpus loader, which Bun's bench runs too
 * and so reads with `node:fs`, a named exception whose reason is Bun; the checkout's files,
 * read-only, and the reject list and the stamp it writes beneath `benches/js/.cache`; `git`
 * with git's local environment, for the sibling checkouts' commits; and `svelte/compiler`, the
 * oracle, loaded in process from the bench install. It runs no other program. Run it from the
 * checkout's root, where the corpus paths and the sibling checkouts resolve.
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';

import { open_checkout } from '../../../scripts/roots/checkout.ts';
import { run_svelte_reject_harvest } from '../diagnostics/svelte_reject_harvest.ts';
import { CanonicalImplementation } from '../lib/canonical.ts';
import { corpus_view_paths, load_pinned_language_corpus } from '../lib/corpus.ts';
import { cache_files, stamp_git } from './harvest_deps.ts';

const checkout = await open_checkout();

const code = await run_svelte_reject_harvest(
	{
		git: stamp_git(checkout),
		files: cache_files(checkout),
		corpus: { view_paths: corpus_view_paths, load: load_pinned_language_corpus },
		init_oracle: async (versions) => {
			const canonical = new CanonicalImplementation(versions);
			await canonical.init();
			return canonical;
		}
	},
	process.args
);
process.exit(code);
