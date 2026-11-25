/**
 * `check:ast-schema`' and `ast-schema:update`'s root (`scripts/ast_schema.ts`). Its reach: the
 * checkout's `crates/tsv_wasm/types/`, `crates/tsv_wasm/npm/` and `crates/tsv_arena/src/tape/`,
 * read always and written under `--update`; it runs no program.
 *
 * @module
 */

import { args } from '@fuzdev/gro/args.ts';
import { set_exit_code } from '@fuzdev/gro/exit.ts';

import { run_ast_schema } from '../ast_schema.ts';
import { open_checkout } from './checkout.ts';

const unknown = args.filter((arg) => arg !== '--update');
if (unknown.length > 0) {
	console.error(`unknown argument(s) ${unknown.join(', ')} — the one flag is --update`);
	set_exit_code(2);
} else {
	const checkout = await open_checkout();
	set_exit_code(
		await run_ast_schema(
			{
				types: checkout.dir('crates/tsv_wasm/types'),
				npm: checkout.dir('crates/tsv_wasm/npm'),
				tape: checkout.dir('crates/tsv_arena/src/tape')
			},
			args.includes('--update')
		)
	);
}
