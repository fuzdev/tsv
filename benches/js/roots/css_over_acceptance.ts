/**
 * `css:over-acceptance:run`'s and `css:over-acceptance:pin`'s root
 * (`benches/js/diagnostics/css_over_acceptance.ts`). Its reach: the conformance corpus and
 * the harness's `node_modules`, read through the corpus loader, which Bun's bench runs too
 * and so reads with `node:fs`, a named exception whose reason is Bun; the checkout's files the
 * stamp names, read through a read-only handle, and the stamp itself, the one file it writes,
 * beneath `benches/js/.cache`; `git` with git's local environment, for the sibling checkouts'
 * commits; and, for the profile, every implementation the harness knows, tsv's N-API addon at
 * the `corpus` profile and its `nodejs` WASM bundle among them, whose init runs one more program,
 * `rsvelte-fmt`'s binary from the bench install (`graded_implementations.ts`), a row this tool
 * doesn't read. Run it from the checkout's root, where the corpus paths and the sibling checkouts
 * resolve.
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';

import { open_checkout } from '../../../scripts/roots/checkout.ts';
import { run_css_over_acceptance } from '../diagnostics/css_over_acceptance.ts';
import { CanonicalImplementation } from '../lib/canonical.ts';
import {
	corpus_missing_entries,
	corpus_view_paths,
	load_pinned_language_corpus
} from '../lib/corpus.ts';
import { load_all_versions } from '../lib/versions.ts';
import { exit_with } from './compare_deps.ts';
import { cache_files, stamp_git } from './harvest_deps.ts';

const checkout = await open_checkout();

const code = await run_css_over_acceptance(
	{
		git: stamp_git(checkout),
		files: cache_files(checkout),
		corpus: {
			missing_entries: corpus_missing_entries,
			view_paths: corpus_view_paths,
			load: load_pinned_language_corpus
		},
		init_oracle: async () => {
			const canonical = new CanonicalImplementation((await load_all_versions()).canonical);
			await canonical.init();
			return canonical;
		},
		// imported on the call, so the pin leg never loads the engines' modules
		init_implementations: async (logger) =>
			(await import('./graded_implementations.ts')).init_graded_implementations(logger),
		stdout: process.stdout
	},
	process.args
);
exit_with(code);
