/**
 * The CHANGELOG.md grammar the release path reads — shared by `publish.ts`
 * (stamps `## Unreleased` into `## <version>`) and `release_notes.ts` (reads the
 * stamped section back as the GitHub Release body) — plus the one spelling of
 * the version a release tag names (`version_from_tag`), which every release
 * script parses the same way. Pure text helpers, no I/O, so both readers parse
 * one grammar and the section a stamp writes is the section a release reads.
 * The grammar is LF-only: `publish.ts` writes LF, and a CRLF heading matches
 * nothing (pinned by the tests).
 */

export const CHANGELOG_PATH = 'CHANGELOG.md';

export type BumpLevel = 'patch' | 'minor' | 'major';

/** The `<!-- bump: <level> -->` marker, recognized ONLY on the line directly
 * after the `## Unreleased` heading — the exact position `changelog_stamp`'s
 * `UNRELEASED_HEADING_WITH_MARKER` rewrites. `changelog_section` returns the
 * body starting at the newline after the heading, so the leading `\n` here
 * anchors that placement. A marker anywhere else in the section is ignored, so
 * validation can't accept a changelog whose marker the stamper would fail to
 * strip. */
const UNRELEASED_BUMP_MARKER = /^\n<!-- bump: (patch|minor|major) -->(?:\n|$)/;

/** A heading line's end: the next byte is a `\n`, or there is none. Not a
 * multiline `$`, which also matches ahead of a `\r` and so would admit a CRLF
 * heading — whose section the LF-spelled marker and stamp regexes then miss,
 * stamping mixed line endings. The grammar is LF-only; a CRLF heading is no
 * heading, and a CRLF changelog fails loudly at the publish gate instead. */
const LINE_END = '(?![^\\n])';

/** The `## Unreleased` heading line. */
const UNRELEASED_HEADING = new RegExp(`^## Unreleased${LINE_END}`, 'm');

/** The `## Unreleased` heading with its marker on the next line — the two lines
 * `changelog_stamp` replaces. The same placement `UNRELEASED_BUMP_MARKER` reads,
 * down to the marker's terminator (a newline, or EOF), so a marker the reader
 * accepts is one the stamper strips. */
const UNRELEASED_HEADING_WITH_MARKER =
	/^## Unreleased\n<!-- bump: (?:patch|minor|major) -->(?:\n|$)/m;

/** What a fresh cycle starts from: an empty `## Unreleased` reset to `bump: patch`. */
const FRESH_UNRELEASED = '## Unreleased\n<!-- bump: patch -->\n\n';

/** The version a release tag names: `v0.3.0` → `0.3.0` (a bare `0.3.0` is
 * accepted too, so a caller can pass a tag name as-is). Null when the text is
 * not a `v<major>.<minor>.<patch>`. */
export const version_from_tag = (tag: string): string | null => {
	const version = tag.startsWith('v') ? tag.slice(1) : tag;
	return /^\d+\.\d+\.\d+$/.test(version) ? version : null;
};

/** Matches the `## <heading>` line (multiline), the heading text taken
 * literally — every regex metacharacter escaped, not just the version's dots. */
const heading_re = (heading: string): RegExp =>
	new RegExp(`^## ${heading.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}${LINE_END}`, 'm');

/** Body of the `## <heading>` section (after the heading line, up to the next
 * `## ` heading or EOF), or null if there's no such heading. `heading` is the
 * literal text after `## ` — `Unreleased` or a version like `0.3.0`. */
export const changelog_section = (changelog: string, heading: string): string | null => {
	const match = heading_re(heading).exec(changelog);
	if (!match) return null;
	const after = changelog.slice(match.index + match[0].length);
	const next = after.search(/^## /m);
	return next === -1 ? after : after.slice(0, next);
};

/** The `## Unreleased` body with the bump marker + surrounding whitespace
 * stripped. `''` means an empty section; `null` means no section. */
export const changelog_unreleased_content = (changelog: string): string | null => {
	const section = changelog_section(changelog, 'Unreleased');
	if (section === null) return null;
	return section.replace(UNRELEASED_BUMP_MARKER, '').trim();
};

/** The bump level the `## Unreleased` marker declares, or null. */
export const changelog_declared_bump = (changelog: string): BumpLevel | null => {
	const section = changelog_section(changelog, 'Unreleased');
	if (section === null) return null;
	const m = UNRELEASED_BUMP_MARKER.exec(section);
	return m ? (m[1] as BumpLevel) : null;
};

/** The release notes for `version`: its `## <version>` body, trimmed. `''`
 * means the section is present but empty; `null` means no section — a tag
 * whose changelog was never stamped, which the release job treats as a
 * failure rather than publishing an empty release. */
export const changelog_release_notes = (changelog: string, version: string): string | null => {
	const section = changelog_section(changelog, version);
	return section === null ? null : section.trim();
};

export type ChangelogStampOutcome =
	/** `## <version>` was already present — a retry after a failed wetrun; text unchanged. */
	| 'already_stamped'
	/** `## Unreleased` + marker became `## <version>`, and a fresh `## Unreleased` was seeded. */
	| 'stamped'
	/** Same, from an `## Unreleased` with no marker (a wetrun would have refused earlier). */
	| 'stamped_without_marker'
	/** No `## Unreleased` at all; text unchanged. */
	| 'no_unreleased';

/** What `changelog_stamp` did, and the text after it (unchanged unless stamped). */
export interface ChangelogStamp {
	outcome: ChangelogStampOutcome;
	text: string;
}

/** Stamp the `## Unreleased` section into `## <version>` (dropping its bump
 * marker) and seed a fresh empty `## Unreleased` reset to `bump: patch` for the
 * next cycle. Pure: returns the new text and what happened; idempotent, since
 * an already-stamped version is left alone. */
export const changelog_stamp = (changelog: string, version: string): ChangelogStamp => {
	if (heading_re(version).test(changelog)) {
		return { outcome: 'already_stamped', text: changelog };
	}
	if (UNRELEASED_HEADING_WITH_MARKER.test(changelog)) {
		return {
			outcome: 'stamped',
			text: changelog.replace(UNRELEASED_HEADING_WITH_MARKER, `${FRESH_UNRELEASED}## ${version}\n`)
		};
	}
	if (UNRELEASED_HEADING.test(changelog)) {
		return {
			outcome: 'stamped_without_marker',
			text: changelog.replace(UNRELEASED_HEADING, `${FRESH_UNRELEASED}## ${version}`)
		};
	}
	return { outcome: 'no_unreleased', text: changelog };
};
