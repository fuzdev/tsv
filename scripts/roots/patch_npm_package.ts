/**
 * The wasm package patcher's root (`scripts/patch_npm_package.ts`), run by `build:npm:format`,
 * `build:npm:parse`, and `build:npm:all` after their wasm-pack build. Its reach:
 *
 * - the checkout read-only, found from this module's location
 * - writes beneath `crates/tsv_wasm/pkg/<variant>/npm` alone, the variant's staging, opened
 *   once the argument names a variant
 * - `gzip`, found on `PATH`, with gro's base environment; a missing one fails the summary,
 *   after the package is written
 *
 * @module
 */

import { process } from '@fuzdev/gro/process.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

import { run_patch_npm_package } from '../patch_npm_package.ts';
import { open_checkout } from './checkout.ts';

const checkout = await open_checkout();
const code = await run_patch_npm_package(
	{
		checkout: checkout.readonly(),
		open_package: (variant) => checkout.dir(`crates/tsv_wasm/pkg/${variant}/npm`),
		gzip: spawn.find('gzip'),
		print: (line) => console.log(line),
		print_error: (line) => console.error(line)
	},
	process.args
);
process.set_exit_code(code);
