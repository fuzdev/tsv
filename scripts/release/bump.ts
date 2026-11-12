/**
 * Writing a bump into the checkout: the version in `Cargo.toml` (with the `=x.y.z` pins its
 * `[workspace.dependencies]` path entries carry), the workspace members' entries in the
 * lockfile, and the changelog's stamp. These are the only files a release commit holds.
 * Every write after the version is idempotent, so a resumed release repeats them safely.
 *
 * @module
 */

import type { FsDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';

import { CHANGELOG_PATH, changelog_stamp, type ChangelogStampOutcome } from './changelog.ts';
import { program_capture } from './program.ts';
import type { Result } from './result.ts';
import { CARGO_PATH, cargo_workspace_version_set } from './version.ts';

const LOCKFILE_PATH = 'Cargo.lock';

/** Every file a bump writes, and so everything the release commit holds. */
export const RELEASE_PATHS: ReadonlyArray<string> = [CARGO_PATH, LOCKFILE_PATH, CHANGELOG_PATH];

/** The release commit's message, which a resume also reads to recognize its own commit. */
export const release_commit_message = (version: string): string => `chore: publish v${version}`;

/** Write `version` into the checkout's `Cargo.toml`, its `[workspace.package]` table and every
 * exact pin on a `[workspace.dependencies]` path entry, in one write — or nothing, when the
 * setter refuses. */
export const bump_cargo_version = async (checkout: FsDir, version: string): Promise<Result> => {
	const next = cargo_workspace_version_set(await checkout.read_text(CARGO_PATH), version);
	if (!next.ok) return next;
	await checkout.write(CARGO_PATH, next.text);
	return { ok: true };
};

/** Move the lockfile's workspace members to the manifest's version, running `cargo` in the
 * checkout. Offline: a member's version is all that changes, and no registry crate may move
 * with it. */
export const bump_lockfile = async (
	cargo: SpawnProgram | null,
	checkout: FsDir
): Promise<Result> => {
	const result = await program_capture(cargo, 'cargo', ['update', '--workspace', '--offline'], {
		cwd: checkout
	});
	if (!result.success)
		return {
			ok: false,
			error: `cargo update --workspace for ${LOCKFILE_PATH} failed:\n${result.err}`
		};
	return { ok: true };
};

/** Stamp the checkout changelog's `## Unreleased` section as `version`, writing only when it
 * changes. */
export const bump_changelog = async (
	checkout: FsDir,
	version: string
): Promise<ChangelogStampOutcome> => {
	const { outcome, text } = changelog_stamp(await checkout.read_text(CHANGELOG_PATH), version);
	if (outcome === 'stamped' || outcome === 'stamped_without_marker')
		await checkout.write(CHANGELOG_PATH, text);
	return outcome;
};
