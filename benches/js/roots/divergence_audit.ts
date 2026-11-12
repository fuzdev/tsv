/**
 * `divergence:audit`'s root (`benches/js/divergence_audit.ts`). Its reach: read-only handles
 * on the checkout's `tests/fixtures` and `docs/`, found from this module's location, never the
 * working directory. It writes nothing and runs nothing.
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';

import { open_checkout } from '../../../scripts/roots/checkout.ts';
import { run_divergence_audit } from '../divergence_audit.ts';
import { fixture_io } from '../lib/divergence/fixture_io.ts';

const checkout = (await open_checkout()).readonly();
const code = await run_divergence_audit(
	{ fixtures: fixture_io(checkout.dir('tests/fixtures')), docs: checkout.dir('docs') },
	process.args
);
process.set_exit_code(code);
