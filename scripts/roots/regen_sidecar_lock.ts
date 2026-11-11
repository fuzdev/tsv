/**
 * `pins:lock`'s root (`scripts/regen_sidecar_lock.ts`). Its reach: the checkout, where npm
 * writes the sidecar's lock; with `--check`, a scratch directory under the OS temp directory,
 * removed when the run ends; and `npm`, found on `PATH`, with gro's base environment and the
 * proxy variables npm reads.
 *
 * @module
 */

import { fs } from '@fuzdev/gro/fs.ts';
import { process } from '@fuzdev/gro/process.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

import { run_regen_sidecar_lock } from '../regen_sidecar_lock.ts';
import { open_checkout } from './checkout.ts';

/** The proxy variables npm reads, in both spellings, for the registry requests of a write. */
const NPM_PROXY_ENV = [
	'HTTP_PROXY',
	'HTTPS_PROXY',
	'NO_PROXY',
	'http_proxy',
	'https_proxy',
	'no_proxy'
];

const npm = spawn.find('npm', { pass_env: NPM_PROXY_ENV });
if (npm === null) {
	console.error('FAIL: npm is not on PATH');
	process.exit(1);
}
const checkout = await open_checkout();
const check = process.args.includes('--check');
await using scratch = check ? await fs.temp_dir('tsv-sidecar-lock-') : null;
const code = await run_regen_sidecar_lock({ checkout, scratch, npm }, process.args);
process.set_exit_code(code);
