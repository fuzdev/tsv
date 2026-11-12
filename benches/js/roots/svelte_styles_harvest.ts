/**
 * `bench:harvest:svelte-styles`' root (`benches/js/diagnostics/svelte_styles_harvest.ts`). Its
 * reach: the perf view, read through the corpus loader, which Bun's bench runs too and so
 * reads with `node:fs`, a named exception whose reason is Bun; the checkout's files, read-only,
 * and the stamp it writes beneath `benches/js/.cache`; the concats' directory,
 * `benches/js/.cache/svelte_styles`, which it rewrites and clears of strays; and `git` with
 * git's local environment, for the snapshot's `collections/` tree id. It runs no other program.
 * Run it from the checkout's root, where the corpus paths and `../corpora` resolve.
 *
 * @module
 */

import { resolve } from 'node:path';

import { process } from '@fuzdev/gro/process.ts';

import { CHECKOUT_PATH, open_checkout } from '../../../scripts/roots/checkout.ts';
import { run_svelte_styles_harvest } from '../diagnostics/svelte_styles_harvest.ts';
import { CORPORA_COLLECTIONS } from '../lib/corpora.ts';
import { CorpusLoader, corpus_view_paths, SVELTE_STYLES_CACHE } from '../lib/corpus.ts';
import { cache_files, stamp_git } from './harvest_deps.ts';

const checkout = await open_checkout();
// opened on each call: the harvest makes the directory before its first use
const styles = () => checkout.dir(SVELTE_STYLES_CACHE);

const code = await run_svelte_styles_harvest(
	{
		git: stamp_git(checkout),
		files: cache_files(checkout),
		corpus: {
			view_paths: corpus_view_paths,
			stream: (logger) => new CorpusLoader('perf').stream(logger)
		},
		styles: {
			read_text_if_exists: (name) => styles().read_text_if_exists(name),
			write: (name, data) => styles().write(name, data),
			list: () => styles().list(),
			remove: (name) => styles().remove(name)
		},
		collections_root: resolve(CHECKOUT_PATH, CORPORA_COLLECTIONS)
	},
	process.args
);
process.exit(code);
