/**
 * `bench:harvest:ts-repo`'s root (`benches/js/harvest_ts_repo.ts`). Its reach: the
 * `../typescript` checkout, read-only (its cases and baselines); the checkout's files,
 * read-only, and the two lists and the stamp it writes beneath `benches/js/.cache`; `git` with
 * git's local environment, for the checkout's commit; and the `typescript` package, the
 * validity oracle, loaded in process from the bench install. It runs no other program. Run it
 * from the checkout's root, where `../typescript` and the cache paths resolve.
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';

import { open_checkout, sibling_reads } from '../../../scripts/roots/checkout.ts';
import { run_harvest_ts_repo } from '../harvest_ts_repo.ts';
import { load_typescript } from '../lib/tsc.ts';
import { TS_REPO } from '../lib/ts_repo.ts';
import { cache_files, stamp_git } from './harvest_deps.ts';

const checkout = await open_checkout();

const code = await run_harvest_ts_repo(
	{
		git: stamp_git(checkout),
		files: cache_files(checkout),
		typescript: sibling_reads(TS_REPO),
		load_typescript
	},
	process.args
);
process.exit(code);
