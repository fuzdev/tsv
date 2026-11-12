/**
 * Print the release notes for one version — the `## <version>` section of
 * CHANGELOG.md, which `scripts/publish.ts` stamped from `## Unreleased` at
 * release time. The release workflow's `release` job feeds this to
 * `gh release create` as the GitHub Release body, so the notes on GitHub are
 * the changelog's own words and nothing else has to be written per release.
 *
 * Fails (exit 1) when the section is missing or empty: publish.ts refuses to
 * stamp an empty `## Unreleased`, so at a release tag the section exists and is
 * non-empty by construction — a miss here means the tag does not point at a
 * release commit, and a GitHub Release with no body would only hide that.
 *
 * Its entry is `scripts/roots/release_notes.ts`, whose module doc says why this module and
 * everything it imports stay on `node:*` and relative modules, never a package. The changelog
 * read is the checkout's, never the working directory's.
 *
 * Usage: node scripts/roots/release_notes.ts v<version>
 *        npm run release:notes -- v0.3.0
 *
 * Accepts the version with or without the `v` prefix so the workflow can pass
 * the tag name as-is.
 *
 * @module
 */

import { join } from 'node:path';
import { parseArgs } from 'node:util';

import { CHANGELOG_PATH, changelog_release_notes } from './release/changelog.ts';
import { version_from_tag } from './release/version.ts';

/** What `run_release_notes` reaches. */
export interface ReleaseNotesDeps {
	/** The checkout's path, where `CHANGELOG.md` is read. */
	checkout: string;
	read_text: (path: string) => Promise<string>;
	/** Prints the text and a newline to stdout, which carries the notes alone. */
	print: (line: string) => void;
	/** Prints a line to stderr. */
	print_error: (line: string) => void;
}

/**
 * Print the `## <version>` section of the checkout's `CHANGELOG.md` (module doc).
 * @returns the exit code: 0 once printed, 1 for a missing tag, a tag that is no version, an
 *   unreadable changelog, or a missing or empty section
 * @throws `TypeError` - for a flag `parseArgs` refuses
 */
export async function run_release_notes(
	deps: ReleaseNotesDeps,
	argv: Array<string>
): Promise<number> {
	// `parseArgs` with no options: a stray `--flag` is rejected instead of being
	// read as the tag.
	const { positionals } = parseArgs({ args: argv, allowPositionals: true });
	const [arg] = positionals;
	if (!arg) {
		deps.print_error('usage: release_notes.ts v<version>');
		return 1;
	}
	const version = version_from_tag(arg);
	if (version === null) {
		deps.print_error(`FAIL: "${arg}" is not a v<major>.<minor>.<patch> version`);
		return 1;
	}

	let changelog: string;
	try {
		changelog = await deps.read_text(join(deps.checkout, CHANGELOG_PATH));
	} catch (e) {
		deps.print_error(`FAIL: cannot read ${CHANGELOG_PATH}: ${String(e)}`);
		return 1;
	}

	const notes = changelog_release_notes(changelog, version);
	if (notes === null) {
		deps.print_error(
			`FAIL: no "## ${version}" section in ${CHANGELOG_PATH} — is this tag on the release commit?`
		);
		return 1;
	}
	if (notes === '') {
		deps.print_error(`FAIL: "## ${version}" section in ${CHANGELOG_PATH} is empty`);
		return 1;
	}

	deps.print(notes);
	return 0;
}
