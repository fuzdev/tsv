/**
 * The parse claims a fixture tree commits — what `corpus_compare_parse.ts --fixtures`
 * grades: each fixture's `input.*` at its `goal` marker's goal, the variants an
 * `expected_<stem>.json` pins, and a `_svelte_divergence` input's committed pin pair.
 * Node-modules-free.
 *
 * @module
 */

import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { basename, dirname, join } from 'node:path';

import type { ParseGoal } from './types.ts';

/** A fixture document's parse claim, as `--fixtures` reads it off the fixture directory. */
export interface FixtureDocument {
	/** The fixture's `goal` marker (`script`), or `undefined` for the module default. */
	goal: ParseGoal | undefined;
	/** A `_svelte_divergence` input's committed pins: tsv's wire and the oracle's. */
	declared: { ours: unknown; svelte: unknown } | null;
}

/** Each fixture directory's documents, keyed by basename — read once per directory. */
const fixture_dirs = new Map<string, Map<string, FixtureDocument> | null>();

/** The fixture input filenames, in `find_input_file`'s precedence order. */
const FIXTURE_INPUTS = ['input.svelte', 'input.svelte.ts', 'input.ts', 'input.css'];

/**
 * The parse claim a fixture tree makes about the document at `path`, or `null` when it
 * makes none: a fixture's `input.*` (at its `goal` marker's goal) and every variant an
 * `expected_<stem>.json` pins are parse-pinned; every other file — the `unformatted_*` and
 * prettier-side variants, `input_invalid_*` — carries no parse claim, and a `tsv_rejects.txt`
 * fixture has no tsv wire. The parse-pinned subset of `tsv_debug loc_wires`' documents: that
 * leg also grades the format variants, which need no oracle, where this one grades only the
 * documents a committed oracle verdict exists for.
 */
export function fixture_document(path: string): FixtureDocument | null {
	const dir = dirname(path);
	let docs = fixture_dirs.get(dir);
	if (docs === undefined) {
		docs = read_fixture_dir(dir);
		fixture_dirs.set(dir, docs);
	}
	return docs?.get(basename(path)) ?? null;
}

function read_fixture_dir(dir: string): Map<string, FixtureDocument> | null {
	const entries = new Set(readdirSync(dir));
	const input = FIXTURE_INPUTS.find((name) => entries.has(name));
	if (input === undefined || entries.has('tsv_rejects.txt')) return null;
	const goal: ParseGoal | undefined =
		entries.has('goal') && readFileSync(join(dir, 'goal'), 'utf8').trim() === 'script'
			? 'script'
			: undefined;
	const read_json = (name: string): unknown => JSON.parse(readFileSync(join(dir, name), 'utf8'));
	const declared =
		/_svelte(_prettier)?_divergence$/.test(basename(dir)) &&
		entries.has('expected_ours.json') &&
		entries.has('expected_svelte.json')
			? { ours: read_json('expected_ours.json'), svelte: read_json('expected_svelte.json') }
			: null;
	const docs = new Map<string, FixtureDocument>([[input, { goal, declared }]]);
	const ext = input.slice('input'.length);
	for (const name of entries) {
		const stem = /^expected_(.+)\.json$/.exec(name)?.[1];
		if (stem === undefined || stem === 'ours' || stem === 'svelte') continue;
		if (existsSync(join(dir, `${stem}${ext}`))) {
			docs.set(`${stem}${ext}`, { goal, declared: null });
		}
	}
	return docs;
}
