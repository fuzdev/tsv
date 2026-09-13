/**
 * The changelog grammar `publish.ts` writes and `release_notes.ts` reads: the
 * two must agree on where a section ends, where the bump marker sits, and what
 * a stamped version section reads back as — or a release's GitHub notes are
 * not its changelog entry.
 */

import { strictEqual } from 'node:assert';

import {
	changelog_declared_bump,
	changelog_release_notes,
	changelog_section,
	changelog_stamp,
	changelog_unreleased_content,
	version_from_tag
} from './changelog.ts';

const changelog = `# tsv changelog

Preamble that mentions \`## Unreleased\` in prose.

## Unreleased
<!-- bump: minor -->

- feat: pending

## 0.2.0

- **breaking** feat: configless
- fix: a thing
  ([#1](https://github.com/fuzdev/tsv/pull/1))

## 0.1.10

- initial
`;

Deno.test('changelog_section returns the body up to the next heading', () => {
	strictEqual(
		changelog_section(changelog, '0.2.0'),
		'\n\n- **breaking** feat: configless\n- fix: a thing\n  ([#1](https://github.com/fuzdev/tsv/pull/1))\n\n'
	);
});

Deno.test('changelog_section reads the last section to EOF', () => {
	strictEqual(changelog_section(changelog, '0.1.10'), '\n\n- initial\n');
});

Deno.test('changelog_section matches the heading line only, never prose', () => {
	// `## Unreleased` inside the preamble's backticks is not a heading line
	strictEqual(
		changelog_section(changelog, 'Unreleased'),
		'\n<!-- bump: minor -->\n\n- feat: pending\n\n'
	);
	strictEqual(changelog_section(changelog, '0.2'), null);
	// the dot is literal: `0.2.0` must not match a `0x2x0` heading
	strictEqual(changelog_section('## 0x2x0\n\n- x\n', '0.2.0'), null);
});

Deno.test('changelog_unreleased_content strips the marker and trims', () => {
	strictEqual(changelog_unreleased_content(changelog), '- feat: pending');
	strictEqual(
		changelog_unreleased_content('## Unreleased\n<!-- bump: patch -->\n\n## 0.1.0\n'),
		''
	);
	strictEqual(changelog_unreleased_content('## 0.1.0\n'), null);
});

Deno.test('changelog_declared_bump reads the marker only directly under the heading', () => {
	strictEqual(changelog_declared_bump(changelog), 'minor');
	strictEqual(changelog_declared_bump('## Unreleased\n\n<!-- bump: major -->\n'), null);
	strictEqual(changelog_declared_bump('## 0.1.0\n'), null);
});

Deno.test('changelog_release_notes is the trimmed version section', () => {
	strictEqual(
		changelog_release_notes(changelog, '0.2.0'),
		'- **breaking** feat: configless\n- fix: a thing\n  ([#1](https://github.com/fuzdev/tsv/pull/1))'
	);
	strictEqual(changelog_release_notes(changelog, '0.1.10'), '- initial');
	strictEqual(changelog_release_notes(changelog, '0.3.0'), null);
	strictEqual(changelog_release_notes('## 0.3.0\n\n## 0.2.0\n\n- x\n', '0.3.0'), '');
});

Deno.test('version_from_tag accepts v-prefixed and bare semver only', () => {
	strictEqual(version_from_tag('v0.3.0'), '0.3.0');
	strictEqual(version_from_tag('0.3.0'), '0.3.0');
	strictEqual(version_from_tag('v0.3'), null);
	strictEqual(version_from_tag('v0.3.0-rc.1'), null);
	strictEqual(version_from_tag('main'), null);
});

Deno.test('changelog_stamp renames Unreleased, drops the marker, seeds a fresh cycle', () => {
	const stamp = changelog_stamp(changelog, '0.3.0');
	strictEqual(stamp.outcome, 'stamped');
	strictEqual(
		stamp.text,
		`# tsv changelog

Preamble that mentions \`## Unreleased\` in prose.

## Unreleased
<!-- bump: patch -->

## 0.3.0

- feat: pending

## 0.2.0

- **breaking** feat: configless
- fix: a thing
  ([#1](https://github.com/fuzdev/tsv/pull/1))

## 0.1.10

- initial
`
	);
	// what the stamp wrote is what a release reads back
	strictEqual(changelog_release_notes(stamp.text, '0.3.0'), '- feat: pending');
	strictEqual(changelog_declared_bump(stamp.text), 'patch');
	strictEqual(changelog_unreleased_content(stamp.text), '');
});

Deno.test('changelog_stamp is idempotent on a retry', () => {
	const once = changelog_stamp(changelog, '0.3.0');
	const twice = changelog_stamp(once.text, '0.3.0');
	strictEqual(twice.outcome, 'already_stamped');
	strictEqual(twice.text, once.text);
});

Deno.test('changelog_stamp handles a marker-less Unreleased and none at all', () => {
	const no_marker = changelog_stamp('## Unreleased\n\n- x\n', '1.0.0');
	strictEqual(no_marker.outcome, 'stamped_without_marker');
	strictEqual(no_marker.text, '## Unreleased\n<!-- bump: patch -->\n\n## 1.0.0\n\n- x\n');
	const none = changelog_stamp('## 0.9.0\n\n- x\n', '1.0.0');
	strictEqual(none.outcome, 'no_unreleased');
	strictEqual(none.text, '## 0.9.0\n\n- x\n');
});

Deno.test('changelog_stamp strips a marker that ends the file, as the reader accepts it', () => {
	// the reader's `(?:\n|$)` and the stamper's must agree, or a marker the reader
	// validated would ride into the released section
	const at_eof = '## Unreleased\n<!-- bump: minor -->';
	strictEqual(changelog_declared_bump(at_eof), 'minor');
	const stamp = changelog_stamp(at_eof, '0.3.0');
	strictEqual(stamp.outcome, 'stamped');
	strictEqual(stamp.text, '## Unreleased\n<!-- bump: patch -->\n\n## 0.3.0\n');
});

Deno.test('the grammar is LF-only: a CRLF changelog matches nothing', () => {
	const crlf = '## Unreleased\r\n<!-- bump: minor -->\r\n\r\n- x\r\n\r\n## 0.1.0\r\n\r\n- y\r\n';
	strictEqual(changelog_section(crlf, 'Unreleased'), null);
	strictEqual(changelog_declared_bump(crlf), null);
	strictEqual(changelog_release_notes(crlf, '0.1.0'), null);
	strictEqual(changelog_stamp(crlf, '0.2.0').outcome, 'no_unreleased');
});

Deno.test('the marker spelling is exact', () => {
	strictEqual(changelog_declared_bump('## Unreleased\n<!--bump: patch-->\n'), null);
	strictEqual(changelog_declared_bump('## Unreleased\n<!-- bump:  patch -->\n'), null);
	strictEqual(changelog_declared_bump('## Unreleased\n<!-- bump: Patch -->\n'), null);
});

Deno.test('a deeper heading inside a section does not end it', () => {
	strictEqual(
		changelog_release_notes('## 0.2.0\n\n### Fixes\n\n- x\n\n## 0.1.0\n\n- y\n', '0.2.0'),
		'### Fixes\n\n- x'
	);
});

Deno.test('changelog_section takes every heading character literally', () => {
	strictEqual(changelog_section('## a+b\n- x\n', 'a+b'), '\n- x\n');
	strictEqual(changelog_section('## aab\n- x\n', 'a+b'), null);
	strictEqual(changelog_section('## 0.2.0 (final)\n- x\n', '0.2.0'), null);
});
