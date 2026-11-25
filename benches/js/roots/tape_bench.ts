/**
 * `tape:bench:run`'s root (`benches/js/tape_bench.ts`). Its reach: the corpus, read by the library;
 * the checkout's `target/corpus/tsv_debug`, run in the checkout with gro's base environment and
 * the panic hook's backtrace switches (`TSV_DEBUG_PASS_ENV`); a scratch directory beneath the
 * checkout's `target/` for the cold arm's inputs; and the running `node`, started fresh per cold
 * figure.
 *
 * It runs from the checkout's root, where the corpus paths resolve (`exit_unless_checkout_cwd`).
 *
 * @module
 */

import { execFile } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { promisify } from 'node:util';

import { set_exit_code } from '@fuzdev/gro/exit.ts';

import { exit_unless_checkout_cwd, open_checkout } from '../../../scripts/roots/checkout.ts';
import { open_checkout_program } from '../../../scripts/roots/checkout_program.ts';
import { TSV_DEBUG_CORPUS, TSV_DEBUG_PASS_ENV } from '../lib/tsv_debug_server.ts';
import { run_tape_bench } from '../tape_bench.ts';

const checkout = await open_checkout();
exit_unless_checkout_cwd(checkout);
const checkout_program = await open_checkout_program(checkout);
const scratch = join(checkout.path, 'target', 'tape_bench');
await mkdir(scratch, { recursive: true });
const gc = (globalThis as { gc?: () => void }).gc;
set_exit_code(
	await run_tape_bench({
		open_tsv_debug: () => checkout_program(TSV_DEBUG_CORPUS, { pass_env: TSV_DEBUG_PASS_ENV }),
		write_scratch: async (name, bytes) => {
			const path = join(scratch, name);
			await writeFile(path, bytes);
			return path;
		},
		run_node: async (code) =>
			(
				await promisify(execFile)(process.execPath, [
					'--expose-gc',
					'--input-type=module',
					'-e',
					code
				])
			).stdout,
		...(gc ? { gc } : {})
	})
);
