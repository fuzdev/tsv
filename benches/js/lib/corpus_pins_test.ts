/**
 * Tests for the per-source corpus pins (`corpus_pins.ts`): the row derivation, the
 * file grammar's round trip and its refusals, the diff a grade reports, and a grade's and an
 * update's use of the pins file, through an in-memory one.
 * Dependency-free, so it gates in `npm run test:node`.
 */

import { deepStrictEqual, ok, rejects, strictEqual, throws } from 'node:assert';
import { test } from 'vitest';

import { CORPORA_COLLECTIONS } from './corpora.ts';
import {
	corpus_pin_row,
	CorpusPinTally,
	diff_pins,
	grade_corpus_pins,
	parse_pins,
	type PinsFile,
	pins_disagree_with_stats,
	serialize_pins,
	sum_pins
} from './corpus_pins.ts';
import type { Language } from './types.ts';

const BUCKETS = ['match', 'unknown'] as const;

/** An in-memory pins file: its text (`null` = absent) and every write, in order. */
const memory_pins = (text: string | null, writable = true) => {
	const state = { text, writes: [] as string[] };
	const pins: PinsFile = {
		read: async () => state.text,
		...(writable
			? {
					write: async (next: string) => {
						state.writes.push(next);
						state.text = next;
					}
				}
			: {})
	};
	return { state, pins };
};

test('corpus_pin_row names a collection whatever its subpath, else the entry path', () => {
	strictEqual(corpus_pin_row(`${CORPORA_COLLECTIONS}/kit/packages/kit/src`), 'kit');
	strictEqual(
		corpus_pin_row(`${CORPORA_COLLECTIONS}/language-tools/packages/svelte2tsx`),
		'language-tools'
	);
	strictEqual(
		corpus_pin_row('../prettier/tests/format/typescript'),
		'prettier/tests/format/typescript'
	);
	strictEqual(corpus_pin_row('benches/js/.cache/svelte_styles'), 'benches/js/.cache/svelte_styles');
});

test('corpus_pin_row refuses a row the file grammar cannot hold', () => {
	throws(() => corpus_pin_row(`${CORPORA_COLLECTIONS}/`), /no pin row/);
	throws(() => corpus_pin_row('../some dir/x'), /no pin row/);
});

test('a tally round-trips through the file grammar, rows sorted, zero rows kept', () => {
	const tally = new CorpusPinTally(BUCKETS);
	tally.add(`${CORPORA_COLLECTIONS}/zzz/src`, 'typescript', 'match');
	tally.add(`${CORPORA_COLLECTIONS}/zzz/src`, 'svelte', 'unknown');
	tally.add(`${CORPORA_COLLECTIONS}/kit/packages/kit/src`, 'typescript', 'match');
	tally.add(`${CORPORA_COLLECTIONS}/kit/packages/kit/types`, 'typescript', 'match');
	tally.see('../prettier/tests/format/css', 'css');
	const text = serialize_pins(tally.table(), BUCKETS, ['header', '']);
	deepStrictEqual(text.split('\n'), [
		'# header',
		'#',
		'kit typescript match=2 unknown=0',
		'prettier/tests/format/css css match=0 unknown=0',
		'zzz svelte match=0 unknown=1',
		'zzz typescript match=1 unknown=0',
		''
	]);
	const read = parse_pins(text, BUCKETS);
	ok(read.ok);
	deepStrictEqual(read.table, tally.table());
	deepStrictEqual(sum_pins(read.table, BUCKETS), {
		svelte: { match: 0, unknown: 1 },
		typescript: { match: 3, unknown: 0 },
		css: { match: 0, unknown: 0 }
	});
});

test('parse_pins refuses a malformed line, naming it', () => {
	const cases: [string, string][] = [
		['kit typescript match=1', 'line 1: expected'],
		['kit rust match=1 unknown=0', 'unknown language'],
		['kit typescript unknown=0 match=1', 'expected match=<count>'],
		['kit typescript match=-1 unknown=0', 'expected match=<count>'],
		['# c\nkit css match=1 unknown=0\nkit css match=1 unknown=0', 'line 3: duplicate row']
	];
	for (const [text, error] of cases) {
		const read = parse_pins(text, BUCKETS);
		ok(!read.ok && read.error.includes(error), `${JSON.stringify(text)} → ${JSON.stringify(read)}`);
	}
});

test('diff_pins names every moved bucket and one-sided row', () => {
	const read = (text: string) => {
		const r = parse_pins(text, BUCKETS);
		if (!r.ok) throw new Error(r.error);
		return r.table;
	};
	const pinned = read('a css match=1 unknown=0\nb css match=2 unknown=0\nc css match=0 unknown=0');
	const measured = read(
		'a css match=1 unknown=0\nb css match=1 unknown=1\nd css match=4 unknown=0\nb svelte match=1 unknown=0'
	);
	deepStrictEqual(diff_pins(pinned, measured, BUCKETS), [
		'b svelte: new row (match=1 unknown=0)',
		'b css match 2 → 1',
		'b css unknown 0 → 1',
		'c css: pinned row not measured (a source absent from this run?)',
		'd css: new row (match=4 unknown=0)'
	]);
	deepStrictEqual(diff_pins(pinned, pinned, BUCKETS), []);
});

test('a tally that disagrees with the run stats fails before the pins are read', async () => {
	const tally = new CorpusPinTally(BUCKETS);
	tally.add(`${CORPORA_COLLECTIONS}/zzz/src`, 'css', 'match');
	const stats = (language: Language) => ({ match: language === 'css' ? 2 : 0, unknown: 0 });
	deepStrictEqual(pins_disagree_with_stats(tally.table(), BUCKETS, stats), [
		"css: the per-source tally disagrees with the run's stats (match 1 vs 2) — a harness bug, not a corpus move"
	]);
	const failures = await grade_corpus_pins({
		file: 'benches/js/lib/no_such_pins.txt',
		pins: memory_pins(null).pins,
		update_command: 'npm run test -- --all --update',
		tally,
		update: true,
		stats
	});
	strictEqual(failures.length, 1);
	ok(failures[0]!.includes('disagrees'));
});

test('an update with a blocker refuses without writing; a grade of a missing file fails', async () => {
	const tally = new CorpusPinTally(BUCKETS);
	tally.add(`${CORPORA_COLLECTIONS}/zzz/src`, 'css', 'match');
	const stats = (language: Language) => ({ match: language === 'css' ? 1 : 0, unknown: 0 });
	const file = 'benches/js/lib/no_such_pins.txt';
	const { state, pins } = memory_pins(null);
	const options = { file, pins, update_command: 'npm run test -- --all --update', tally, stats };
	deepStrictEqual(
		await grade_corpus_pins({ ...options, update: true, update_blockers: ['a red verdict'] }),
		['refusing --update: a red verdict']
	);
	deepStrictEqual(state.writes, []);
	deepStrictEqual(await grade_corpus_pins({ ...options, update: false }), [
		`${file} is missing — run \`npm run test -- --all --update\``
	]);
	const failing: PinsFile = {
		read: () => Promise.reject(new Error('EACCES')),
		write: () => Promise.reject(new Error('unreachable'))
	};
	await rejects(grade_corpus_pins({ ...options, pins: failing, update: false }), /EACCES/);
});

test('an update writes the measured table under the generated header, and a grade then passes', async () => {
	const tally = new CorpusPinTally(BUCKETS);
	tally.add(`${CORPORA_COLLECTIONS}/zzz/src`, 'css', 'match');
	tally.add(`${CORPORA_COLLECTIONS}/zzz/src`, 'css', 'unknown');
	const stats = (language: Language) =>
		language === 'css' ? { match: 1, unknown: 1 } : { match: 0, unknown: 0 };
	const { state, pins } = memory_pins('# stale\nold svelte match=9 unknown=9\n');
	const options = {
		file: 'pins.txt',
		pins,
		update_command: 'npm run corpus:compare:test -- --all --update',
		tally,
		stats
	};
	deepStrictEqual(await grade_corpus_pins({ ...options, update: true }), []);
	strictEqual(state.writes.length, 1);
	strictEqual(
		state.writes[0],
		serialize_pins(tally.table(), BUCKETS, [
			'Generated by `npm run corpus:compare:test -- --all --update` — never hand-edited.',
			'Exact per-source counts over the `gates` view; see docs/gate_counts.md.',
			'',
			'<row> <language> match=<n> unknown=<n>'
		])
	);
	deepStrictEqual(await grade_corpus_pins({ ...options, update: false }), []);
	strictEqual(state.writes.length, 1);
});

test('an update through a pins file with no write refuses, writing nothing', async () => {
	const tally = new CorpusPinTally(BUCKETS);
	tally.add(`${CORPORA_COLLECTIONS}/zzz/src`, 'css', 'match');
	const stats = (language: Language) => ({ match: language === 'css' ? 1 : 0, unknown: 0 });
	const { pins } = memory_pins(null, false);
	deepStrictEqual(
		await grade_corpus_pins({
			file: 'pins.txt',
			pins,
			update_command: 'g',
			tally,
			stats,
			update: true
		}),
		['refusing --update: this entry does not rewrite pins.txt']
	);
});
