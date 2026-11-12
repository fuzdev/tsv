/**
 * `benches/js/diagnostics/skip_triage.ts`'s root. Its reach: a corpus, the positional directory
 * or the `gates` view, read through the corpus loader, which Bun's bench runs too and so reads
 * with `node:fs`, a named exception whose reason is Bun; and the comparison tools' engines
 * (`compare_deps.ts`), the canonical parsers and tsv's N-API addon at the `corpus` profile, in
 * process, behind the bench install's preflight and the addon's freshness guard. It writes
 * nothing and runs no program. Run it from the checkout's root, where the corpus paths resolve.
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';

import { run_skip_triage } from '../diagnostics/skip_triage.ts';
import { CorpusLoader, DirectoryLoader } from '../lib/corpus.ts';
import { exit_with, init_implementations } from './compare_deps.ts';

const code = await run_skip_triage(
	{
		load_files: (path, logger) =>
			(path === undefined ? new CorpusLoader('gates') : new DirectoryLoader(path)).load(logger),
		init_implementations,
		stdout: process.stdout
	},
	process.args
);
exit_with(code);
