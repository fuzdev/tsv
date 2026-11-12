/**
 * A copy from one gro directory handle to another, for the gro-hosted scripts that lay staged
 * packages into a temp consumer project as npm installs them: the declaration check
 * (`scripts/typecheck_packages.ts`) and the engine parity audit (`scripts/engine_parity.ts`).
 * A handle copies only beneath itself, so a copy across two is a read through one and a write
 * through the other.
 *
 * @module
 */

import type { FsDir, FsReadDir } from '@fuzdev/gro/fs.ts';

import { stage_file_between } from './stage_file_handle.ts';

/**
 * Copy the file or directory at `from` beneath `source` to `to` beneath `target`, a directory
 * recursively, each file copied as a staging copies one (`stage_file_between`), keeping its mode.
 * A symlink is followed, as `stat` follows it.
 * @throws `Error` - when `from` is gone, `FsError` - for any read or write that fails
 */
export async function copy_between(
	source: FsReadDir,
	from: string,
	target: FsDir,
	to: string
): Promise<void> {
	const info = await source.stat(from);
	if (info === null) throw new Error(`${from} was removed while it was copied`);
	if (info.kind === 'directory') {
		await target.mkdir(to, { recursive: true });
		for await (const entry of source.list(from)) {
			await copy_between(source, `${from}/${entry.name}`, target, `${to}/${entry.name}`);
		}
		return;
	}
	await stage_file_between(source, target).copy(from, to);
}
