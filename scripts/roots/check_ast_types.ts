/**
 * `check:ast-types`' root (`scripts/check_ast_types.ts`). Its reach: the checkout read-only,
 * its `scripts/` read-write, and two programs beneath it with gro's base environment alone:
 * `target/corpus/tsv` (which reads none but `NO_COLOR` and `FORCE_COLOR`) and
 * `node_modules/.bin/tsc`, a `#!/usr/bin/env node` script found through the base `PATH`.
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';

import { AST_TYPES_TSV_BIN, run_check_ast_types } from '../check_ast_types.ts';
import { checkout_program, open_checkout } from './checkout.ts';

const checkout = await open_checkout();
const code = await run_check_ast_types({
	checkout: checkout.readonly(),
	scripts: checkout.dir('scripts'),
	tsv: checkout_program(checkout, AST_TYPES_TSV_BIN),
	tsc: checkout_program(checkout, 'node_modules/.bin/tsc')
});
process.set_exit_code(code);
