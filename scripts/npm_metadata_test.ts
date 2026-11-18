/**
 * `npm_pack_reports` over both shapes `npm pack --json` prints: the array of npm 11 and older
 * and npm 12's object keyed by package name.
 */

import { deepStrictEqual, throws } from 'node:assert';
import { describe, test } from 'vitest';

import { npm_pack_reports } from './npm_metadata.ts';

/** A report as npm 12 prints it, verbatim (`npm pack --dry-run --json` of a two-file package). */
const REPORT = {
	id: '@x/probe@0.0.1',
	name: '@x/probe',
	version: '0.0.1',
	size: 166,
	unpackedSize: 39,
	shasum: '8601d3ad453b4b7c02a61d2bf7e4b14e0b29acb7',
	integrity:
		'sha512-JbB73VkwSbl/tKZjwNUY9BSZiWUSKH6BMDy1uzasILEAs4n8gaSPwchAz9jplLnawTeL8UigNQm8dY/tLibnbw==',
	filename: 'x-probe-0.0.1.tgz',
	files: [
		{ path: 'index.js', size: 2, mode: 420 },
		{ path: 'package.json', size: 37, mode: 420 }
	],
	entryCount: 2,
	bundled: []
};
const OTHER = { id: '@x/b@1.0.0', name: '@x/b', filename: 'x-b-1.0.0.tgz', files: [] };

describe('npm_pack_reports', () => {
	test('an array of reports is read as its elements, in order', () => {
		deepStrictEqual(npm_pack_reports(JSON.stringify([REPORT, OTHER], null, 2)), [REPORT, OTHER]);
	});

	test('an object keyed by package name is read as its values, in order', () => {
		deepStrictEqual(
			npm_pack_reports(JSON.stringify({ '@x/probe': REPORT, '@x/b': OTHER }, null, 2)),
			[REPORT, OTHER]
		);
	});

	test('an empty array or object reads as no reports', () => {
		deepStrictEqual(npm_pack_reports('[]'), []);
		deepStrictEqual(npm_pack_reports('{}'), []);
	});

	test('JSON of neither shape throws', () => {
		for (const stdout of ['null', '"x"', '1', 'true']) {
			throws(() => npm_pack_reports(stdout), /neither an array nor an object/, stdout);
		}
	});

	test('output that is no JSON throws', () => {
		for (const stdout of ['', 'npm warn x']) {
			throws(() => npm_pack_reports(stdout), /^Error: npm pack --json printed no JSON: /, stdout);
		}
	});
});
