/**
 * Tests for the no-locations grader (`assert_locations_dropped`) and the walk that
 * drives it (`assert_binding_drops_locations`).
 *
 * Pins the directions that matter, since a wrong verdict either publishes a
 * loc-bearing parse under the no-locations label or stops a healthy run:
 *  - a span-only AST beside a loc-bearing sibling PASSES,
 *  - a no-locations AST that still carries `loc` FAILS, naming the artifact and its
 *    rebuild — the check's whole purpose, and
 *  - a loc-bearing sibling with no `loc` FAILS — the vacuity arm, the shape no live
 *    run can show you.
 *
 * Hand-written ASTs rather than a real binding on purpose: the grader's job is to
 * read a wire's shape, and a test that parsed through an artifact would need one
 * built, test the artifact instead, and stop gating in `deno task check` (see
 * `test:deno`). What the bindings actually emit is asked at every `init()`.
 */

import { doesNotThrow, strictEqual, throws } from 'node:assert';
import {
	assert_binding_drops_locations,
	assert_locations_dropped,
	count_loc_keys,
	type LocationsProbeTarget
} from './locations_probe.ts';

const artifact = { path: 'pkg/all/probe/tsv_wasm.js', rebuild: 'deno task build:probe' };

/** A loc-bearing wire: `loc` on the root and on a nested node inside an array. */
const with_loc = {
	type: 'Program',
	start: 0,
	end: 12,
	loc: { start: { line: 1, column: 0 }, end: { line: 1, column: 12 } },
	body: [
		{
			type: 'Identifier',
			name: 'loc',
			start: 6,
			end: 7,
			loc: { start: { line: 1, column: 6 }, end: { line: 1, column: 7 } }
		}
	]
};

/** Its span-only twin — an identifier NAMED `loc` is a value, not a key. */
const without_loc = {
	type: 'Program',
	start: 0,
	end: 12,
	body: [{ type: 'Identifier', name: 'loc', start: 6, end: 7 }]
};

/** The failure a reader acts on: how many `loc`, which artifact, how to rebuild it. */
const names_artifact = (e: Error): boolean =>
	e.message.includes('carrying 2 `loc` keys') &&
	e.message.includes(`${artifact.path} still emits \`loc\` on its no-locations parse`) &&
	e.message.includes(artifact.rebuild);

Deno.test('count_loc_keys: counts keys at every depth, never values', () => {
	strictEqual(count_loc_keys(with_loc), 2);
	strictEqual(count_loc_keys(without_loc), 0);
	strictEqual(count_loc_keys(null), 0);
	strictEqual(count_loc_keys('loc'), 0);
});

Deno.test('locations dropped: span-only beside a loc-bearing sibling passes', () => {
	doesNotThrow(() =>
		assert_locations_dropped('probe', 'parse_no_locations[x]', artifact, with_loc, without_loc)
	);
});

Deno.test('locations dropped: a no-locations AST carrying `loc` fails, naming the artifact', () => {
	throws(
		() => assert_locations_dropped('probe', 'parse_no_locations[x]', artifact, with_loc, with_loc),
		names_artifact
	);
});

Deno.test('locations dropped: a sibling with no `loc` fails as vacuous', () => {
	throws(
		() =>
			assert_locations_dropped(
				'probe',
				'parse_no_locations[x]',
				artifact,
				without_loc,
				without_loc
			),
		/no longer discriminates/
	);
});

/** A binding whose no-locations call does (or does not) drop `loc`, recording its calls. */
const binding = (drops: boolean, calls: Array<string>): LocationsProbeTarget => ({
	parse: (_source, language, goal) => {
		calls.push(`parse:${language}:${goal ?? 'unset'}`);
		return with_loc;
	},
	parse_no_locations: (_source, language, goal) => {
		calls.push(`parse_no_locations:${language}:${goal ?? 'unset'}`);
		return drops ? without_loc : with_loc;
	}
});

Deno.test('binding drops locations: probes svelte and every typescript goal, never css', () => {
	const calls: Array<string> = [];
	doesNotThrow(() => assert_binding_drops_locations('probe', artifact, binding(true, calls)));
	strictEqual(
		calls.filter((c) => c.startsWith('parse_no_locations:')).join(' '),
		'parse_no_locations:svelte:unset parse_no_locations:typescript:unset ' +
			'parse_no_locations:typescript:module parse_no_locations:typescript:script'
	);
	strictEqual(
		calls.filter((c) => c.startsWith('parse:')).join(' '),
		'parse:svelte:unset parse:typescript:unset parse:typescript:module parse:typescript:script'
	);
	strictEqual(
		calls.some((c) => c.includes(':css:')),
		false
	);
});

Deno.test('binding drops locations: a binding that ignores the request fails', () => {
	throws(
		() => assert_binding_drops_locations('probe', artifact, binding(false, [])),
		/probe: parse_no_locations\[svelte\]\(\) returned an AST carrying 2 `loc` keys/
	);
});
