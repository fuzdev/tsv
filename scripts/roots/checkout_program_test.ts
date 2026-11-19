/**
 * `open_checkout_program` over scratch trees with real symlinks: a program made and run beneath
 * a checkout that is not the working directory, a `target` or `node_modules` linked elsewhere
 * followed, a binary that is itself an absolute symlink refused, and a missing binary
 * `not_found`.
 */

import { ok, strictEqual } from 'node:assert';
import { join } from 'node:path';
import { describe, test } from 'vitest';

import { fs, type FsDir } from '@fuzdev/gro/fs.ts';
import { is_spawn_error } from '@fuzdev/gro/spawn_error.ts';
import { format_error_detail } from '@fuzdev/gro/error_detail.ts';

import { type CheckoutProgram, open_checkout_program } from './checkout_program.ts';

describe('open_checkout_program', () => {
	/** Plants an executable script at `path` beneath `root` that prints how it was started. */
	const plant_program = async (root: FsDir, path: string): Promise<void> => {
		await root.mkdir(path.slice(0, path.lastIndexOf('/')), { recursive: true });
		await root.write(path, '#!/bin/sh\necho "ran $0 $*"\n');
		await root.set_mode(path, 0o755);
	};

	test('makes and runs a program beneath a checkout that is not the working directory', async () => {
		await using root = await fs.temp_dir('tsv_checkout_program_');
		ok(!join(fs.project.path, '/').startsWith(join(root.path, '/')));
		await plant_program(root, 'checkout/target/corpus/tsv');
		await plant_program(root, 'checkout/node_modules/typescript/bin/tsc');
		await root.mkdir('checkout/node_modules/.bin');
		await root.symlink('checkout/node_modules/.bin/tsc', '../typescript/bin/tsc');
		const checkout_program = await open_checkout_program(
			await fs.open_dir(join(root.path, 'checkout'))
		);
		const tsv = await checkout_program('target/corpus/tsv').out(['a']);
		strictEqual(tsv.stdout, `ran ${join(root.path, 'checkout/target/corpus/tsv')} a\n`);
		// a relative symlink that stays inside the checkout is followed
		const tsc = await checkout_program('node_modules/.bin/tsc').out(['-p', 'x']);
		strictEqual(tsc.stdout, `ran ${join(root.path, 'checkout/node_modules/.bin/tsc')} -p x\n`);
	});

	/** Whether making `path` throws `SpawnError` `not_granted`. */
	const is_refused = (checkout_program: CheckoutProgram, path: string): boolean => {
		try {
			checkout_program(path);
			return false;
		} catch (error) {
			ok(is_spawn_error(error, 'not_granted'), String(error));
			return true;
		}
	};

	test('follows a target and a node_modules that are absolute symlinks out of the checkout', async () => {
		await using root = await fs.temp_dir('tsv_checkout_program_');
		await plant_program(root, 'elsewhere/corpus/tsv_debug');
		await plant_program(root, 'shared/typescript/bin/tsc');
		await root.mkdir('shared/.bin');
		await root.symlink('shared/.bin/tsc', '../typescript/bin/tsc');
		await root.mkdir('checkout');
		await root.symlink('checkout/target', join(root.path, 'elsewhere'));
		await root.symlink('checkout/node_modules', join(root.path, 'shared'));
		const checkout_program = await open_checkout_program(
			await fs.open_dir(join(root.path, 'checkout'))
		);
		const run = await checkout_program('target/corpus/tsv_debug').out([]);
		strictEqual(run.kind, 'exited');
		ok(run.stdout.startsWith('ran ') && run.stdout.endsWith('/corpus/tsv_debug \n'), run.stdout);
		const tsc = await checkout_program('node_modules/.bin/tsc').out(['-p', 'x']);
		strictEqual(tsc.stdout, `ran ${join(root.path, 'shared/.bin/tsc')} -p x\n`);
		// any other top-level directory linked absolutely is refused, as is a name that only
		// begins like a linkable one
		await plant_program(root, 'elsewhere/bin/x');
		await root.symlink('checkout/bin', join(root.path, 'elsewhere/bin'));
		await root.symlink('checkout/target_old', join(root.path, 'elsewhere'));
		ok(is_refused(checkout_program, 'bin/x'));
		ok(is_refused(checkout_program, 'target_old/corpus/tsv_debug'));
	});

	test('refuses a binary that is itself an absolute symlink, beneath a linked target too', async () => {
		await using root = await fs.temp_dir('tsv_checkout_program_');
		await plant_program(root, 'store/tsv');
		await root.mkdir('checkout/target/corpus', { recursive: true });
		await root.symlink('checkout/target/corpus/tsv', join(root.path, 'store/tsv'));
		// a relative one that stays inside target runs
		await plant_program(root, 'checkout/target/store/tsv_debug');
		await root.symlink('checkout/target/corpus/tsv_debug', '../store/tsv_debug');
		const checkout_program = await open_checkout_program(
			await fs.open_dir(join(root.path, 'checkout'))
		);
		ok(is_refused(checkout_program, 'target/corpus/tsv'));
		strictEqual((await checkout_program('target/corpus/tsv_debug').out([])).kind, 'exited');
	});

	test('a missing binary is not_found, naming where it looked; a missing target is never made', async () => {
		await using root = await fs.temp_dir('tsv_checkout_program_');
		await root.mkdir('checkout');
		const checkout = await fs.open_dir(join(root.path, 'checkout'));
		const assert_not_found = async (message: string): Promise<void> => {
			const checkout_program = await open_checkout_program(checkout);
			try {
				checkout_program('target/corpus/tsv');
				ok(false, `${message}: made`);
			} catch (error) {
				ok(is_spawn_error(error, 'not_found'), String(error));
				strictEqual(format_error_detail(error), message);
			}
		};
		// no target: looked for beneath the checkout, which gains none
		await assert_not_found(
			`not_found: target/corpus/tsv (no program target/corpus/tsv beneath ${checkout.path})`
		);
		strictEqual(await root.exists('checkout/target'), false);
		// a target that is a file reads as none
		await root.write('checkout/target', '');
		await assert_not_found(
			`not_found: target/corpus/tsv (no program target/corpus/tsv beneath ${checkout.path})`
		);
		// a target without the binary: looked for beneath target
		await root.remove('checkout/target');
		await root.mkdir('checkout/target/corpus', { recursive: true });
		await assert_not_found(
			`not_found: corpus/tsv (no program corpus/tsv beneath ${join(checkout.path, 'target')})`
		);
	});
});
