/**
 * Writing a bump into the checkout: the version in `Cargo.toml`, the workspace members' entries
 * in the lockfile, and the changelog's stamp. These are the only files a release commit holds.
 * Every write after the version is idempotent, so a resumed release repeats them safely.
 *
 * @module
 */

import { CHANGELOG_PATH, changelog_stamp, type ChangelogStampOutcome } from './changelog.ts';
import { command_capture, type Result } from './command.ts';
import { CARGO_PATH, cargo_workspace_version_set } from './version.ts';

const LOCKFILE_PATH = 'Cargo.lock';

/** Every file a bump writes, and so everything the release commit holds. */
export const RELEASE_PATHS: ReadonlyArray<string> = [CARGO_PATH, LOCKFILE_PATH, CHANGELOG_PATH];

/** The release commit's message, which a resume also reads to recognize its own commit. */
export const release_commit_message = (version: string): string => `chore: publish v${version}`;

/** Write `version` into `Cargo.toml`'s `[workspace.package]` table. */
export const bump_cargo_version = async (version: string): Promise<Result> => {
	const next = cargo_workspace_version_set(await Deno.readTextFile(CARGO_PATH), version);
	if (next === null) return { ok: false, error: `no [workspace.package] version in ${CARGO_PATH}` };
	await Deno.writeTextFile(CARGO_PATH, next);
	return { ok: true };
};

/** Move the lockfile's workspace members to the manifest's version. Offline: a member's
 * version is all that changes, and no registry crate may move with it. */
export const bump_lockfile = async (): Promise<Result> => {
	const result = await command_capture('cargo', ['update', '--workspace', '--offline']);
	if (!result.success)
		return {
			ok: false,
			error: `cargo update --workspace for ${LOCKFILE_PATH} failed:\n${result.err}`
		};
	return { ok: true };
};

/** Stamp the changelog's `## Unreleased` section as `version`, writing only when it changes. */
export const bump_changelog = async (version: string): Promise<ChangelogStampOutcome> => {
	const { outcome, text } = changelog_stamp(await Deno.readTextFile(CHANGELOG_PATH), version);
	if (outcome === 'stamped' || outcome === 'stamped_without_marker')
		await Deno.writeTextFile(CHANGELOG_PATH, text);
	return outcome;
};
