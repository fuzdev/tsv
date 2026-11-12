/**
 * `run_divergence_audit` over in-memory deps: the exit code and stream of each outcome, what it
 * reads from `docs/`, and that an argument error reads nothing.
 */

import { deepStrictEqual, fail, match, strictEqual } from 'node:assert';

import { afterEach, beforeEach, describe, test, vi } from 'vitest';

import { run_divergence_audit } from './divergence_audit.ts';
import type { FixtureIo } from './lib/divergence/fixture_cases.ts';
import type { AuditDocs, AuditReportDeps } from './lib/divergence/validation.ts';

/** A documented divergence, as a catalog list item links one. */
const DOC_WITH_ONE = `## Section

- A feature — [x_prettier_divergence](../tests/fixtures/fake/x_prettier_divergence)
`;

interface Recorded {
	reads: string[];
}

/** Docs over `{name: text}`, plus entries that are not regular files. */
const docs_of = (
	files: Record<string, string>,
	recorded: Recorded,
	others: Array<{ name: string; kind: string }> = []
): AuditDocs => ({
	list: async function* () {
		for (const name of Object.keys(files)) yield { name, kind: 'file' };
		yield* others;
	},
	read_text: (name) => {
		recorded.reads.push(name);
		const text = files[name];
		return text === undefined ? Promise.reject(new Error(`no ${name}`)) : Promise.resolve(text);
	}
});

/** Fixtures where every directory exists and `fake/x_prettier_divergence` pins one pairing. */
const fixtures_of = (options: { dirs: boolean; pair?: [string, string] }): FixtureIo => ({
	dir_exists: () => options.dirs,
	list: (path) =>
		Promise.resolve(
			path === 'fake/x_prettier_divergence' && options.pair
				? ['input.css', 'output_prettier.css']
				: []
		),
	read: (path, name) => {
		if (path !== 'fake/x_prettier_divergence' || !options.pair) return Promise.resolve(null);
		if (name === 'input.css') return Promise.resolve(options.pair[0]);
		if (name === 'output_prettier.css') return Promise.resolve(options.pair[1]);
		return Promise.resolve(null);
	}
});

/** Deps that fail the test if anything is read. */
const untouchable: AuditReportDeps = {
	fixtures: {
		dir_exists: () => fail('read a fixture'),
		list: () => fail('listed a fixture'),
		read: () => fail('read a fixture')
	},
	docs: {
		list: () => fail('listed docs'),
		read_text: () => fail('read a doc')
	}
};

let out: string[];
let err: string[];

beforeEach(() => {
	out = [];
	err = [];
	vi.spyOn(console, 'log').mockImplementation((...a: unknown[]) => void out.push(a.join(' ')));
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void err.push(a.join(' ')));
});

afterEach(() => {
	vi.restoreAllMocks();
});

describe('arguments', () => {
	test('--help prints the usage on stdout and exits 0, reading nothing', async () => {
		strictEqual(await run_divergence_audit(untouchable, ['--help']), 0);
		match(out.join('\n'), /Usage: deno task divergence:audit \[options\]/);
		deepStrictEqual(err, []);
	});

	test('a bad value prints the issue on stderr and the usage on stdout, exits 1, reads nothing', async () => {
		strictEqual(await run_divergence_audit(untouchable, ['--json=maybe']), 1);
		strictEqual(err.length, 1);
		match(out.join('\n'), /Usage: deno task divergence:audit/);
	});
});

describe('the report', () => {
	test('reads the conformance_prettier*.md regular files only, in name order', async () => {
		const recorded: Recorded = { reads: [] };
		const docs = docs_of(
			{
				'conformance_prettier_ts.md': '',
				'conformance_prettier.md': '',
				'other.md': DOC_WITH_ONE,
				'conformance_prettier_css.txt': DOC_WITH_ONE
			},
			recorded,
			[
				{ name: 'conformance_prettier_link.md', kind: 'symlink' },
				{ name: 'conformance_prettier_dir.md', kind: 'directory' }
			]
		);
		strictEqual(await run_divergence_audit({ fixtures: fixtures_of({ dirs: true }), docs }, []), 0);
		deepStrictEqual(recorded.reads, ['conformance_prettier.md', 'conformance_prettier_ts.md']);
	});

	test('nothing documented and every listing resolving: PASS, exit 0', async () => {
		const docs = docs_of({ 'conformance_prettier.md': '' }, { reads: [] });
		strictEqual(await run_divergence_audit({ fixtures: fixtures_of({ dirs: true }), docs }, []), 0);
		match(out.at(-1)!, /PASS: every gradeable documented divergence is fully explained/);
	});

	test('a pattern listing naming no directory: FAIL, exit 1', async () => {
		const docs = docs_of({ 'conformance_prettier.md': '' }, { reads: [] });
		strictEqual(
			await run_divergence_audit({ fixtures: fixtures_of({ dirs: false }), docs }, []),
			1
		);
		match(out.at(-1)!, /FAIL: \d+ pattern fixture listing\(s\) point at no directory/);
	});

	test('a documented divergence no pattern explains: WARN, exit 1', async () => {
		const docs = docs_of({ 'conformance_prettier.md': DOC_WITH_ONE }, { reads: [] });
		const fixtures = fixtures_of({ dirs: true, pair: ['a {\n}\n', 'b {\n}\n'] });
		strictEqual(await run_divergence_audit({ fixtures, docs }, []), 1);
		match(out.join('\n'), /WARN: 1 documented divergences are detected by no pattern; 0 more/);
	});

	test('--json prints the report and exits 0 even with a gap', async () => {
		const docs = docs_of({ 'conformance_prettier.md': DOC_WITH_ONE }, { reads: [] });
		const fixtures = fixtures_of({ dirs: true, pair: ['a {\n}\n', 'b {\n}\n'] });
		strictEqual(await run_divergence_audit({ fixtures, docs }, ['--json']), 0);
		strictEqual(out.length, 1);
		const report = JSON.parse(out[0]);
		deepStrictEqual(report.undetected_fixtures, ['fake/x_prettier_divergence']);
		strictEqual(report.stats.total_documented, 1);
	});

	test('a documented fixture with no directory is ungradeable, not a gap', async () => {
		const docs = docs_of({ 'conformance_prettier.md': DOC_WITH_ONE }, { reads: [] });
		const fixtures: FixtureIo = {
			...fixtures_of({ dirs: true }),
			dir_exists: (path) => path !== 'fake/x_prettier_divergence'
		};
		strictEqual(await run_divergence_audit({ fixtures, docs }, ['--json']), 0);
		const report = JSON.parse(out[0]);
		deepStrictEqual(report.detection, [
			{
				fixture_path: 'fake/x_prettier_divergence',
				status: 'ungradeable',
				patterns: [],
				reason: 'missing_directory'
			}
		]);
	});
});

describe('the catalog parse', () => {
	/** A catalog with each form a documented divergence takes, and links the parse must skip. */
	const CATALOG = `## Top

#### Deep section

| Feature | Reason | Fixture |
| --- | --- | --- |
| Feat A | Design choice | [a_prettier_divergence](../tests/fixtures/fake/a_prettier_divergence) |
| Feat B | Spec violation | [b_divergence](../tests/fixtures/fake/b_divergence) |

- List feat — [c_prettier_divergence](../tests/fixtures/fake/c_prettier_divergence/)

**Bold feat:** see [d_prettier_divergence](../tests/fixtures/fake/d_prettier_divergence)
`;

	test('reads table rows, list items, and prose under the nearest heading, divergences only', async () => {
		const docs = docs_of({ 'conformance_prettier.md': CATALOG }, { reads: [] });
		strictEqual(
			await run_divergence_audit({ fixtures: fixtures_of({ dirs: true }), docs }, ['--json']),
			0
		);
		deepStrictEqual(JSON.parse(out[0]).documented, [
			{
				section: 'Deep section',
				feature: 'Feat A',
				reason: 'Design choice',
				fixture_name: 'a_prettier_divergence',
				fixture_path: 'fake/a_prettier_divergence'
			},
			{
				section: 'Deep section',
				feature: 'List feat',
				reason: '',
				fixture_name: 'c_prettier_divergence',
				fixture_path: 'fake/c_prettier_divergence'
			},
			{
				section: 'Deep section',
				feature: 'Bold feat',
				reason: '',
				fixture_name: 'd_prettier_divergence',
				fixture_path: 'fake/d_prettier_divergence'
			}
		]);
	});

	test('a documented fixture with an input and no prettier form is ungradeable as such', async () => {
		const docs = docs_of({ 'conformance_prettier.md': DOC_WITH_ONE }, { reads: [] });
		const fixtures: FixtureIo = {
			dir_exists: () => true,
			list: () => Promise.resolve(['input.css']),
			read: (_path, name) => Promise.resolve(name === 'input.css' ? 'a {\n}\n' : null)
		};
		strictEqual(await run_divergence_audit({ fixtures, docs }, ['--json']), 0);
		deepStrictEqual(JSON.parse(out[0]).detection, [
			{
				fixture_path: 'fake/x_prettier_divergence',
				status: 'ungradeable',
				patterns: [],
				reason: 'no_prettier_form'
			}
		]);
	});
});
