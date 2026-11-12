/**
 * `conformance:ts-repo:run`'s root (`benches/js/diagnostics/ts_repo_compare.ts`). Its reach:
 * tsv's N-API addon at the `corpus` profile and the canonical oracles, loaded in process, and
 * the `../typescript` checkout, read through read-only handles opened by the paths the gate
 * spells. It writes nothing and runs no program. Its packages resolve from
 * `benches/js/node_modules`, gro from the checkout's own install above it.
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';

import { run_ts_repo_compare } from '../diagnostics/ts_repo_compare.ts';
import { exit_with, init_implementations, open_readonly_dir } from './compare_deps.ts';

const code = await run_ts_repo_compare(
	{ init_implementations, open_dir: open_readonly_dir, stdout: process.stdout },
	process.args
);
exit_with(code);
