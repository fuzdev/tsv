/**
 * The release notes over a fake changelog read: what reaches stdout and stderr, where the
 * changelog is read from, and every exit.
 */

import { deepStrictEqual, rejects, strictEqual } from 'node:assert';
import { join } from 'node:path';
import { test } from 'vitest';

import { type ReleaseNotesDeps, run_release_notes } from './release_notes.ts';

const CHECKOUT = join('/', 'repo');

const CHANGELOG =
	'# Changelog\n\n## Unreleased\n\n## 0.2.0\n\n- feat: two\n- fix: three\n\n## 0.1.0\n\n- initial\n';

interface FakeWorld {
	deps: ReleaseNotesDeps;
	reads: Array<string>;
	out: Array<string>;
	err: Array<string>;
}

const fake_world = (read: (path: string) => string = () => CHANGELOG): FakeWorld => {
	const reads: Array<string> = [];
	const out: Array<string> = [];
	const err: Array<string> = [];
	return {
		reads,
		out,
		err,
		deps: {
			checkout: CHECKOUT,
			read_text: async (path) => {
				reads.push(path);
				return read(path);
			},
			print: (line) => out.push(line),
			print_error: (line) => err.push(line)
		}
	};
};

test("prints the version's section alone to stdout, read from the checkout's changelog", async () => {
	for (const tag of ['v0.2.0', '0.2.0']) {
		const world = fake_world();
		strictEqual(await run_release_notes(world.deps, [tag]), 0);
		deepStrictEqual(world.reads, [join(CHECKOUT, 'CHANGELOG.md')]);
		deepStrictEqual(world.out, ['- feat: two\n- fix: three']);
		deepStrictEqual(world.err, []);
	}
});

test('no tag is a usage error, read nothing', async () => {
	const world = fake_world();
	strictEqual(await run_release_notes(world.deps, []), 1);
	deepStrictEqual(world.err, ['usage: release_notes.ts v<version>']);
	deepStrictEqual(world.reads, []);
	deepStrictEqual(world.out, []);
});

test('a tag that is no version fails before the read', async () => {
	const world = fake_world();
	strictEqual(await run_release_notes(world.deps, ['v0.2']), 1);
	deepStrictEqual(world.err, ['FAIL: "v0.2" is not a v<major>.<minor>.<patch> version']);
	deepStrictEqual(world.reads, []);
});

test('an unreadable changelog fails, naming it and the error', async () => {
	const world = fake_world(() => {
		throw new Error('boom');
	});
	strictEqual(await run_release_notes(world.deps, ['v0.2.0']), 1);
	deepStrictEqual(world.err, ['FAIL: cannot read CHANGELOG.md: Error: boom']);
	deepStrictEqual(world.out, []);
});

test('a missing section fails', async () => {
	const world = fake_world();
	strictEqual(await run_release_notes(world.deps, ['v0.3.0']), 1);
	deepStrictEqual(world.err, [
		'FAIL: no "## 0.3.0" section in CHANGELOG.md — is this tag on the release commit?'
	]);
	deepStrictEqual(world.out, []);
});

test('an empty section fails', async () => {
	const world = fake_world(() => '## 0.3.0\n\n## 0.2.0\n\n- x\n');
	strictEqual(await run_release_notes(world.deps, ['v0.3.0']), 1);
	deepStrictEqual(world.err, ['FAIL: "## 0.3.0" section in CHANGELOG.md is empty']);
	deepStrictEqual(world.out, []);
});

test('a flag throws instead of being read as the tag; a second positional is ignored', async () => {
	for (const argv of [['--tag', 'v0.2.0'], ['--help']]) {
		const world = fake_world();
		await rejects(run_release_notes(world.deps, argv), TypeError);
		deepStrictEqual(world.reads, []);
	}
	const world = fake_world();
	strictEqual(await run_release_notes(world.deps, ['v0.1.0', 'v0.2.0']), 0);
	deepStrictEqual(world.out, ['- initial']);
});
