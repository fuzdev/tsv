/**
 * `ts-repo:over-acceptance:run`'s root (`benches/js/diagnostics/ts_repo_over_acceptance.ts`).
 * Its reach: the rejects cache in the checkout and the `../typescript` files it lists, read
 * through read-only handles, and every implementation the harness knows, tsv's N-API addon
 * at the `corpus` profile and its `nodejs` WASM bundle among them. It writes nothing. The one
 * program it runs is the set's: `rsvelte-fmt`'s binary from the bench install, at init
 * (`graded_implementations.ts`), a row this tool doesn't read.
 *
 * @module
 */

import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import { process } from '@fuzdev/gro/process.ts';

import {
	run_ts_repo_over_acceptance,
	TS_REPO_CHECKOUT
} from '../diagnostics/ts_repo_over_acceptance.ts';
import { exit_with, open_readonly_dir } from './compare_deps.ts';
import { init_graded_implementations } from './graded_implementations.ts';

// opened on the first read, so a missing checkout fails the read the tool reports
let typescript: Promise<FsReadDir> | undefined;

const code = await run_ts_repo_over_acceptance(
	{
		checkout: await open_readonly_dir('.'),
		typescript: {
			read_text: async (path) =>
				(await (typescript ??= open_readonly_dir(TS_REPO_CHECKOUT))).read_text(path)
		},
		init_implementations: init_graded_implementations,
		stdout: process.stdout
	},
	process.args
);
exit_with(code);
