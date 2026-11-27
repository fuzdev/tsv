/**
 * `crates/tsv_wasm/npm/locations.js`'s line lookup graded against a plain reference: seeded
 * random sources (every line terminator, a leading BOM) and random offset sequences — runs on
 * one line, backward and forward jumps of every size, line starts, the text's end, and the
 * off-text offsets the walk never checks — handed to `reconstruct_locations`, a locator's
 * `reconstruct` and its `loc_of` / `position_at`, each answer compared with the line a full
 * binary search over an independently built line table gives.
 *
 * The walk searches outward from the line it answered last, so its answer must not depend on
 * the order offsets arrive in; that is what the random orders grade. `npm run check:loc`
 * grades the definition itself against the Rust emitter over the fixture tree.
 */

import { deepStrictEqual, strictEqual } from 'node:assert';
import { test } from 'vitest';

import { create_locator, reconstruct_locations } from '../crates/tsv_wasm/npm/locations.js';

type Language = 'typescript' | 'svelte' | 'css';
type Position = { line: number; column: number };
type NamePosition = Position & { character: number };

/** mulberry32: a small seeded generator, so a failure reproduces from its seed. */
const create_random = (seed: number): (() => number) => {
	let a = seed >>> 0;
	return () => {
		a = (a + 0x6d2b79f5) >>> 0;
		let t = a;
		t = Math.imul(t ^ (t >>> 15), t | 1);
		t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
};

/** The pieces a random source is drawn from: text, and every terminator either rule reads. */
const PIECES = [
	'a',
	'bc',
	'def ',
	'\t',
	'\n',
	'\n',
	'\r\n',
	'\r',
	'\u2028',
	'\u2029',
	'\n\n',
	'é',
	'😀'
];

const random_source = (random: () => number, bom: boolean): string => {
	let source = bom ? '\uFEFF' : '';
	const count = Math.floor(random() * 120);
	for (let i = 0; i < count; i++) source += PIECES[Math.floor(random() * PIECES.length)];
	return source;
};

/** The reference line table, built by a regex split rather than `locations.js`'s scan. */
const reference_starts = (text: string, language: Language): Array<number> => {
	const terminator = language === 'typescript' ? /\r\n|[\n\r\u2028\u2029]/g : /\n/g;
	const starts = [0];
	for (const match of text.matchAll(terminator)) starts.push(match.index + match[0].length);
	return starts;
};

/** The reference lookup: a full binary search, the rightmost start at or before `offset`. */
const reference_line = (offset: number, starts: Array<number>): number => {
	let lo = 0;
	let hi = starts.length - 1;
	while (lo < hi) {
		const mid = (lo + hi + 1) >> 1;
		if (starts[mid]! <= offset) lo = mid;
		else hi = mid - 1;
	}
	return lo;
};

const reference_position = (offset: number, starts: Array<number>): Position => {
	const i = reference_line(offset, starts);
	return { line: i + 1, column: offset - starts[i]! };
};

const reference_name_position = (offset: number, starts: Array<number>): NamePosition => ({
	...reference_position(offset, starts),
	character: offset
});

/**
 * The next offset of a random sequence: mostly near the previous one, as a parse's walk
 * reads them, but every few draws a jump anywhere, a line start, the end, or off the text.
 */
const next_offset = (
	random: () => number,
	previous: number,
	length: number,
	starts: Array<number>,
	off_text: boolean
): number => {
	const roll = random();
	if (roll < 0.4) return Math.min(length, previous + Math.floor(random() * 4));
	if (roll < 0.55) return Math.max(0, previous - Math.floor(random() * 6));
	if (roll < 0.7) return Math.floor(random() * (length + 1));
	if (roll < 0.8) return starts[Math.floor(random() * starts.length)]!;
	if (roll < 0.85) return length;
	if (roll < 0.9) return 0;
	if (!off_text) return Math.floor(random() * (length + 1));
	const wild = [-1, -0, length + 1, length + 50, 1.5, Number.NaN, Number.POSITIVE_INFINITY];
	return wild[Math.floor(random() * wild.length)]!;
};

type Node = {
	type: string;
	start: number;
	end: number;
	name?: string;
	children: Array<Node>;
	loc?: unknown;
	name_loc?: unknown;
};

/**
 * A random tree over `text`: nested arrays of nodes whose offsets follow `next_offset`, each
 * `end` near its `start` or anywhere. Svelte trees carry some `RegularElement`s, whose
 * `name_loc` (the run after `<`) the walk looks up from the same hint.
 */
const random_tree = (
	random: () => number,
	text: string,
	starts: Array<number>,
	language: Language,
	off_text: boolean
): Node => {
	let previous = 0;
	const make = (depth: number): Node => {
		const start = next_offset(random, previous, text.length, starts, off_text);
		previous = start;
		const end =
			random() < 0.7
				? Math.min(text.length, start + Math.floor(random() * 8))
				: next_offset(random, start, text.length, starts, off_text);
		const node: Node = { type: 'Node', start, end, children: [] };
		if (language === 'svelte' && random() < 0.2) {
			node.type = 'RegularElement';
			node.name = 'x'.repeat(1 + Math.floor(random() * 3));
		}
		const count = depth < 4 ? Math.floor(random() * 4) : 0;
		for (let i = 0; i < count; i++) node.children.push(make(depth + 1));
		return node;
	};
	return make(0);
};

/** The tree `reconstruct_locations` should return, built with the reference lookup. */
const reference_reconstruct = (node: Node, starts: Array<number>): void => {
	node.loc = {
		start: reference_position(node.start, starts),
		end: reference_position(node.end, starts)
	};
	if (node.type === 'RegularElement') {
		const name_start = node.start + 1;
		node.name_loc = {
			start: reference_name_position(name_start, starts),
			end: reference_name_position(name_start + node.name!.length, starts)
		};
	}
	for (const child of node.children) reference_reconstruct(child, starts);
};

const LANGUAGES: Array<Language> = ['typescript', 'svelte', 'css'];

test('reconstruct_locations: random trees answer as a full binary search does', () => {
	for (let seed = 1; seed <= 600; seed++) {
		const random = create_random(seed);
		const language = LANGUAGES[seed % LANGUAGES.length]!;
		const source = random_source(random, random() < 0.3);
		// the wire's offsets index the source without its BOM, except for TypeScript
		const text =
			language !== 'typescript' && source.startsWith('\uFEFF') ? source.slice(1) : source;
		const starts = reference_starts(text, language);
		const tree = random_tree(random, text, starts, language, random() < 0.3);
		const expected = structuredClone(tree);
		reference_reconstruct(expected, starts);
		const via_locator = structuredClone(tree);
		const reconstructed = reconstruct_locations(tree, source, { language });
		deepStrictEqual(reconstructed, expected, `seed ${seed}`);
		// deep equality ignores key order; the serialization does not
		strictEqual(
			JSON.stringify(reconstructed),
			JSON.stringify(expected),
			`seed ${seed} (key order)`
		);
		deepStrictEqual(
			create_locator(source, { language }).reconstruct(via_locator),
			expected,
			`seed ${seed} (locator)`
		);
	}
});

test('create_locator: loc_of and position_at answer as a full binary search does', () => {
	for (let seed = 1; seed <= 300; seed++) {
		const random = create_random(seed * 7919);
		const language = LANGUAGES[seed % LANGUAGES.length]!;
		const source = random_source(random, random() < 0.3);
		const text =
			language !== 'typescript' && source.startsWith('\uFEFF') ? source.slice(1) : source;
		const starts = reference_starts(text, language);
		const locator = create_locator(source, { language });
		let previous = 0;
		for (let i = 0; i < 40; i++) {
			const start = next_offset(random, previous, text.length, starts, false);
			previous = start;
			const end = start + Math.floor(random() * (text.length - start + 1));
			deepStrictEqual(
				locator.loc_of({ start, end }),
				{ start: reference_position(start, starts), end: reference_position(end, starts) },
				`seed ${seed}: ${start}..${end}`
			);
			deepStrictEqual(
				locator.position_at(end),
				reference_position(end, starts),
				`seed ${seed}: ${end}`
			);
		}
	}
});
