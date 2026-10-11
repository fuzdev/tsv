/**
 * What a publish run releases, decided once from what it reads before touching anything: the
 * `Cargo.toml` version, the resume sentinel, `--bump`, and the changelog. Pure, so every branch
 * (a fresh release, a resumed one, a dry run's preview) is unit-tested without a checkout.
 *
 * @module
 */

import {
	changelog_declared_bump,
	changelog_unreleased_content,
	CHANGELOG_PATH
} from './changelog.ts';
import { type BumpLevel, version_bump, version_parse } from './version.ts';

export interface ReleasePlanInput {
	/** The `Cargo.toml` `[workspace.package]` version. */
	version: string;
	/** The `--bump` level, or null when none was passed. */
	bump: BumpLevel | null;
	/** The version an interrupted wetrun left in the sentinel, or null. */
	sentinel: string | null;
	/** `CHANGELOG.md`'s text, or null when it is missing. */
	changelog: string | null;
}

export type ReleasePlan =
	/** A wetrun bumped to `version` and failed before it finished; the release resumes from the
	 * bumped checkout. */
	| {
			kind: 'resume';
			version: string;
			/** Non-blocking notes, such as a `--bump` the resume ignores. */
			notes: Array<string>;
	  }
	/** A new release from `version` to `target`, which is null when a problem leaves it unknown. */
	| {
			kind: 'fresh';
			version: string;
			level: BumpLevel | null;
			target: string | null;
			/** Each reason a wetrun refuses before it mutates anything; a dry run warns instead. */
			problems: Array<string>;
			/** A sentinel naming another version, left by an abandoned release, which a fresh
			 * wetrun removes. */
			stale_sentinel: string | null;
	  };

/**
 * Decide what a run releases. A sentinel naming the checkout's own version means a wetrun
 * already bumped to it, so the release resumes there and ignores `--bump`; anything else is a
 * fresh release, which needs a non-empty `## Unreleased` section whose bump marker `--bump`
 * repeats, so the level is stated twice and a slip in either one refuses.
 *
 * @param input - what the run read before mutating anything
 * @returns the release the run performs, with every reason a fresh one cannot proceed
 */
export const release_plan = (input: ReleasePlanInput): ReleasePlan => {
	const { version, bump, sentinel, changelog } = input;
	if (sentinel === version) {
		return {
			kind: 'resume',
			version,
			notes: bump
				? [`--bump ${bump} is ignored — resuming the interrupted v${version} release`]
				: []
		};
	}

	const problems: Array<string> = [];
	if (!version_parse(version)) {
		problems.push(
			`Cargo.toml's version "${version}" is not <major>.<minor>.<patch>, so it has no bump`
		);
	}

	let declared: BumpLevel | null = null;
	if (changelog === null) {
		problems.push(`no ${CHANGELOG_PATH} — a release needs its notes`);
	} else {
		const content = changelog_unreleased_content(changelog);
		if (content === null)
			problems.push(`no "## Unreleased" section in ${CHANGELOG_PATH} — add the release notes`);
		else if (content === '')
			problems.push(`"## Unreleased" in ${CHANGELOG_PATH} is empty — add the release notes`);
		declared = changelog_declared_bump(changelog);
		if (content !== null && declared === null) {
			problems.push(
				`no "<!-- bump: patch|minor|major -->" marker under "## Unreleased" in ${CHANGELOG_PATH}`
			);
		}
	}

	if (bump === null) problems.push('no --bump — pass the level the changelog marker declares');
	else if (declared !== null && bump !== declared) {
		problems.push(
			`--bump ${bump} disagrees with ${CHANGELOG_PATH}'s marker <!-- bump: ${declared} -->`
		);
	}

	const level = bump !== null && (declared === null || bump === declared) ? bump : null;
	return {
		kind: 'fresh',
		version,
		level,
		target: level === null ? null : version_bump(version, level),
		problems,
		stale_sentinel: sentinel
	};
};
