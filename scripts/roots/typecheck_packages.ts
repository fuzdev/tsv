/**
 * `typecheck:packages`' root (`scripts/typecheck_packages.ts`): the staged npm packages'
 * declarations, type-checked as a consumer's compiler sees them. Its reach:
 *
 * - the checkout read-only, found from this module's location
 * - a temp directory, the consumer project, removed when the run ends
 * - `BENCH_STALE_OK`, and no other variable
 * - TypeScript from the `benches/js` install, loaded with `node:module`'s `createRequire`, a
 *   named exception to the host-module rule: no host module loads a CommonJS module, and
 *   TypeScript reads the consumer project and the install through Node's own `fs`
 *
 * @module
 */

import { createRequire } from 'node:module';
import { join } from 'node:path';

import { fs } from '@fuzdev/gro/fs.ts';
import { process } from '@fuzdev/gro/process.ts';

import { run_typecheck_packages, type TypeScriptApi } from '../typecheck_packages.ts';
import { open_checkout } from './checkout.ts';

const checkout = await open_checkout();
const code = await run_typecheck_packages(
	{
		checkout: checkout.readonly(),
		temp_dir: () => fs.temp_dir('tsv-typecheck-packages-'),
		load_typescript: (path) =>
			createRequire(import.meta.url)(join(checkout.path, path)) as TypeScriptApi,
		stale_ok: process.env.narrow(['BENCH_STALE_OK']).get('BENCH_STALE_OK') === '1',
		print: (line) => console.log(line),
		print_error: (line) => console.error(line)
	},
	process.args
);
process.set_exit_code(code);
