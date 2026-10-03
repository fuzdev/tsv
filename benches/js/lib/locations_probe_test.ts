/**
 * Tests for the span-only grader (`assert_span_only`) and the walk that drives it
 * (`assert_binding_emits_span_only`).
 *
 * Pins the directions that matter, since a wrong verdict either publishes a
 * loc-bearing parse under a span label or stops a healthy run:
 *  - a span-only AST with numeric root offsets PASSES,
 *  - an AST that still carries `loc` (or Svelte's `name_loc`) FAILS, naming the artifact
 *    and its rebuild — the check's whole purpose, and
 *  - an AST whose root has no numeric `start`/`end` FAILS — the vacuity arm, the shape
 *    no live run can show you.
 *
 * Hand-written ASTs rather than a real binding on purpose: the grader's job is to
 * read a wire's shape, and a test that parsed through an artifact would need one
 * built, test the artifact instead, and stop gating in `deno task check` (see
 * `test:deno`). What the bindings actually emit is asked at every `init()`.
 */

import { doesNotThrow, strictEqual, throws } from 'node:assert';
import {
	assert_binding_emits_span_only,
	assert_span_only,
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
const span_only = {
	type: 'Program',
	start: 0,
	end: 12,
	body: [{ type: 'Identifier', name: 'loc', start: 6, end: 7 }]
};

/** A Svelte span wire that kept one `name_loc` and nothing else. */
const with_name_loc = {
	type: 'Root',
	start: 0,
	end: 5,
	fragment: {
		nodes: [{ type: 'RegularElement', start: 0, end: 5, name_loc: { start: {}, end: {} } }]
	}
};

/** The failure a reader acts on: how many keys, which artifact, how to rebuild it. */
const names_artifact =
	(count: string) =>
	(e: Error): boolean =>
		e.message.includes(`carrying ${count}`) &&
		e.message.includes(`${artifact.path} still emits the loc-bearing wire`) &&
		e.message.includes(artifact.rebuild);

Deno.test('count_loc_keys: counts loc and name_loc keys at every depth, never values', () => {
	strictEqual(count_loc_keys(with_loc), 2);
	strictEqual(count_loc_keys(span_only), 0);
	strictEqual(count_loc_keys(with_name_loc), 1);
	strictEqual(count_loc_keys(null), 0);
	strictEqual(count_loc_keys('loc'), 0);
});

Deno.test('span only: offsets without loc pass', () => {
	doesNotThrow(() => assert_span_only('probe', 'parse[x]', artifact, span_only));
});

Deno.test('span only: an AST carrying `loc` fails, naming the artifact', () => {
	throws(
		() => assert_span_only('probe', 'parse[x]', artifact, with_loc),
		names_artifact('2 `loc` / `name_loc` keys')
	);
	throws(
		() => assert_span_only('probe', 'parse[x]', artifact, with_name_loc),
		names_artifact('1 `loc` / `name_loc` key ')
	);
});

Deno.test('span only: a root with no numeric offsets fails as vacuous', () => {
	for (const ast of [{ error: 'x' }, null, { type: 'Program', start: '0', end: 1 }]) {
		throws(() => assert_span_only('probe', 'parse[x]', artifact, ast), /no longer discriminates/);
	}
});

/** A binding whose parse does (or does not) emit the span wire, recording its calls. */
const binding = (span: boolean, calls: Array<string>): LocationsProbeTarget => ({
	parse: (_source, language, goal) => {
		calls.push(`${language}:${goal ?? 'unset'}`);
		return span ? span_only : with_loc;
	}
});

Deno.test('binding emits span only: probes every language, and every typescript goal', () => {
	const calls: Array<string> = [];
	doesNotThrow(() => assert_binding_emits_span_only('probe', artifact, binding(true, calls)));
	strictEqual(
		calls.join(' '),
		'svelte:unset typescript:unset typescript:module typescript:script css:unset'
	);
});

Deno.test('binding emits span only: a binding still emitting loc fails', () => {
	throws(
		() => assert_binding_emits_span_only('probe', artifact, binding(false, [])),
		/probe: parse\[svelte\]\(\) returned an AST carrying 2 `loc` \/ `name_loc` keys/
	);
});
