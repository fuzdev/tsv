/**
 * Tests for the tape-routing probe (`assert_binding_parses_through_tape`).
 *
 * Pins the directions that matter, since a wrong verdict either times another route under
 * the tape route's name or stops a healthy run:
 *  - a table that parses through the tape, returning the wire's tree, PASSES;
 *  - a table that parses past the tape export FAILS on the routing half, and only where that
 *    half is asked (`twin`), since a timed row's process skips it;
 *  - a tape export that hands back anything but a `Uint32Array` FAILS;
 *  - a tree whose KEY ORDER differs from the wire's FAILS — what a structural compare misses —
 *    where the binding emits the JSON wire, and no comparison runs where it emits none;
 *  - a tape the decoder refuses is rethrown naming the artifact and its rebuild;
 *  - a timed row's own process (`twin` null) calls the JSON export never;
 *  - the TIMED table `TsvBinding.bind` builds parses through the tape export alone.
 *
 * Fake engines and a fake publish rather than a real binding, as `locations_probe_test.ts`
 * argues: the probe's job is to read what a wiring does, and what the bindings do is asked
 * at every `init()`.
 */

import { doesNotThrow, strictEqual, throws } from 'node:assert';
import { test } from 'vitest';
import {
	assert_binding_parses_through_tape,
	type PublishParse,
	type TapeProbeEngine,
	type TapeProbeTarget
} from './tape_probe.ts';
import { TsvRejection } from './divergence/panic_errors.ts';
import { TsvBinding, type TsvEngine } from './tsv_api.ts';
import { type Language, LANGUAGES } from './types.ts';

const artifact = { path: 'target/probe/libtsv_probe.so', rebuild: 'npm run build:bench:pgo' };

/** The wire every fake engine answers with, and its tree. */
const WIRE = '{"type":"Program","start":0,"end":1}';

const per_language = <T>(of: (language: Language) => T): Record<Language, T> => ({
	svelte: of('svelte'),
	typescript: of('typescript'),
	css: of('css')
});

const engine = (tape: () => unknown = () => new Uint32Array(8)): TapeProbeEngine => ({
	parse_tape: per_language(() => tape),
	parse_json: per_language(() => () => WIRE)
});

/** A publish whose `parse_<lang>` calls the engine's tape export once (`via: 'parse_tape'`)
 * or never (`'none'`). */
const publish_via =
	(via: 'parse_tape' | 'none'): PublishParse =>
	(e) =>
		Object.fromEntries(
			LANGUAGES.map((language) => [
				`parse_${language}`,
				(source: string) => {
					if (via === 'parse_tape') e.parse_tape[language](source);
					return { ok: true, ast: JSON.parse(WIRE) };
				}
			])
		);

/** A binding whose row parse returns `tree` (or throws it). */
const target = (
	e: TapeProbeEngine,
	tree: () => unknown = () => JSON.parse(WIRE)
): TapeProbeTarget => ({
	engine: e,
	parse: () => tree()
});

test('a table that parses through the tape, to the wire’s tree, passes', () => {
	doesNotThrow(() =>
		assert_binding_parses_through_tape(
			'tsv (probe)',
			artifact,
			target(engine()),
			LANGUAGES,
			publish_via('parse_tape')
		)
	);
});

test('a table that parses past the tape export fails the routing half, when asked', () => {
	throws(
		() =>
			assert_binding_parses_through_tape(
				'tsv (probe)',
				artifact,
				target(engine()),
				['css'],
				publish_via('none')
			),
		/ran the tape export 0 times/
	);
	// a timed row's process skips the routing half and the tree comparison: only the row's
	// own call runs
	doesNotThrow(() =>
		assert_binding_parses_through_tape('tsv (probe)', artifact, target(engine()), ['css'], null)
	);
});

test('a tape export that returns no Uint32Array fails', () => {
	throws(
		() =>
			assert_binding_parses_through_tape(
				'tsv (probe)',
				artifact,
				target(engine(() => new Uint8Array(32))),
				['typescript'],
				publish_via('parse_tape')
			),
		/parse_tape_typescript returned a Uint8Array/
	);
});

test('a tree whose key order differs from the wire’s fails', () => {
	throws(
		() =>
			assert_binding_parses_through_tape(
				'tsv (probe)',
				artifact,
				target(engine(), () => ({ start: 0, type: 'Program', end: 1 })),
				['svelte'],
				publish_via('parse_tape')
			),
		(e: Error) => e.message.includes('key order') && e.message.includes(artifact.rebuild)
	);
});

test('a binding that emits no JSON wire is graded on its routing alone', () => {
	const { parse_json: _, ...tape_only } = engine();
	// a tree the comparison would refuse, so a comparison that ran would throw
	doesNotThrow(() =>
		assert_binding_parses_through_tape(
			'tsv (probe)',
			artifact,
			target(tape_only, () => ({ start: 0, type: 'Program', end: 1 })),
			LANGUAGES,
			publish_via('parse_tape')
		)
	);
});

test('a timed row’s own process makes the row’s call alone: no JSON export, no comparison', () => {
	let wires = 0;
	const counted: TapeProbeEngine = {
		...engine(),
		parse_json: per_language(() => () => (wires++, WIRE))
	};
	// a tree the comparison would refuse, so a comparison that ran would throw
	doesNotThrow(() =>
		assert_binding_parses_through_tape(
			'tsv (probe)',
			artifact,
			target(counted, () => ({ start: 0, type: 'Program', end: 1 })),
			LANGUAGES,
			null
		)
	);
	strictEqual(wires, 0);
});

test('a tape the decoder refuses is rethrown naming the artifact and its rebuild', () => {
	const refused = new Error('internal error: the engine returned a tape that does not decode', {
		cause: new Error('tape: written against another schema or shape list (SHAPES_HASH differs)')
	});
	throws(
		() =>
			assert_binding_parses_through_tape(
				'tsv (probe)',
				artifact,
				target(engine(), () => {
					throw refused;
				}),
				['typescript'],
				null
			),
		(e: Error) =>
			e.message.includes(artifact.path) &&
			e.message.includes(artifact.rebuild) &&
			e.message.includes('SHAPES_HASH') &&
			e.cause === refused
	);
	// any other throw passes through as itself
	const other = new Error('a rejection');
	throws(
		() =>
			assert_binding_parses_through_tape(
				'tsv (probe)',
				artifact,
				target(engine(), () => {
					throw other;
				}),
				['css'],
				null
			),
		(e: unknown) => e === other
	);
});

/**
 * `TsvBinding` over counted parse exports — what the TIMED table (`bind`) calls. The routing
 * half grades the twin `publish_parse` builds, so a `bind` that stopped building its table
 * through `publish_parse` would pass every init; this pins `bind` itself. The tape export
 * returns a failure, which the tape route passes through untouched, so no real tape is needed.
 */
class CountedBinding extends TsvBinding {
	tapes = 0;
	wires = 0;
	constructor() {
		super();
		const failure = () => ({
			ok: false,
			error: {
				kind: 'parse',
				message: 'x',
				frame: '> 1 | x',
				start: 0,
				end: 1,
				loc: { start: { line: 1, column: 0 }, end: { line: 1, column: 1 } }
			}
		});
		const unused = () => {
			throw new Error('not a parse export');
		};
		this.bind({
			parse_tape: per_language(() => () => (this.tapes++, failure())) as TsvEngine['parse_tape'],
			parse_json: per_language(() => () => (this.wires++, failure())) as NonNullable<
				TsvEngine['parse_json']
			>,
			format: per_language(() => unused),
			parse_internal: per_language(() => unused)
		});
	}
	async init(): Promise<void> {}
}

test('the timed table (TsvBinding.bind) parses through the tape export alone', () => {
	for (const language of LANGUAGES) {
		for (const call of ['parse', 'parse_with_locations'] as const) {
			const binding = new CountedBinding();
			throws(() => binding[call]('x', language), TsvRejection);
			strictEqual(binding.tapes, 1, `${call}[${language}]: tape export calls`);
			strictEqual(binding.wires, 0, `${call}[${language}]: JSON export calls`);
		}
	}
});
