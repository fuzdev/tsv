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
 * Usage: deno run --allow-read scripts/release_notes.ts v<version>
 *        deno task release:notes v0.3.0
 *
 * Accepts the version with or without the `v` prefix so the workflow can pass
 * the tag name as-is.
 */

import { parseArgs } from 'node:util';

import { CHANGELOG_PATH, changelog_release_notes, version_from_tag } from './changelog.ts';

// `parseArgs` with no options: a stray `--flag` is rejected instead of being
// read as the tag.
const { positionals } = parseArgs({ allowPositionals: true });
const [arg] = positionals;
if (!arg) {
	console.error('usage: release_notes.ts v<version>');
	Deno.exit(1);
}
const version = version_from_tag(arg);
if (version === null) {
	console.error(`FAIL: "${arg}" is not a v<major>.<minor>.<patch> version`);
	Deno.exit(1);
}

let changelog: string;
try {
	changelog = Deno.readTextFileSync(CHANGELOG_PATH);
} catch (e) {
	console.error(`FAIL: cannot read ${CHANGELOG_PATH}: ${String(e)}`);
	Deno.exit(1);
}

const notes = changelog_release_notes(changelog, version);
if (notes === null) {
	console.error(
		`FAIL: no "## ${version}" section in ${CHANGELOG_PATH} — is this tag on the release commit?`
	);
	Deno.exit(1);
}
if (notes === '') {
	console.error(`FAIL: "## ${version}" section in ${CHANGELOG_PATH} is empty`);
	Deno.exit(1);
}

console.log(notes);
