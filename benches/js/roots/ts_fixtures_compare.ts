/**
 * `conformance:ts-fixtures:run`'s root (`benches/js/diagnostics/ts_fixtures_compare.ts`,
 * over `lib/fixtures_gate.ts`). Its reach: tsv's N-API addon at the `corpus` profile and the
 * canonical oracles, loaded in process, and the suite checkout, read through read-only
 * handles opened by the paths the gate spells. It writes nothing and runs no program. Its
 * packages resolve from `benches/js/node_modules`, gro from the checkout's own install above
 * it.
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';

import { TS_FIXTURES_GATE } from '../diagnostics/ts_fixtures_compare.ts';
import { run_fixtures_gate } from '../lib/fixtures_gate.ts';
import { exit_with, init_implementations, open_readonly_dir } from './compare_deps.ts';

const code = await run_fixtures_gate(
	{ init_implementations, open_dir: open_readonly_dir, stdout: process.stdout },
	TS_FIXTURES_GATE,
	process.args
);
exit_with(code);
