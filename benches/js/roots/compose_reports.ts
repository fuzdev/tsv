/**
 * `bench:compose`'s root (`benches/js/compose_reports.ts`). Its reach: the checkout's
 * `benches/js/results/`, found from this module's location, never the working directory, read
 * and written (`report.json`, `report.md`). It runs nothing.
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';

import { open_checkout } from '../../../scripts/roots/checkout.ts';
import { run_compose_reports } from '../compose_reports.ts';

const checkout = await open_checkout();
process.set_exit_code(await run_compose_reports({ results: checkout.dir('benches/js/results') }));
