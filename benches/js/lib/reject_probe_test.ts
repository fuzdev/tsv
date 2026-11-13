/**
 * Tests for the tsv bindings' rejection probe (`assert_binding_reports_rejection`) over fake
 * bindings: a refusal on every operation passes, an accepted invalid source fails, and a crash
 * (`is_tsv_crash`), a trap or a caught native panic, fails rather than reading as the refusal.
 * Package-free, so it gates in `npm run test:node`.
 */

import { doesNotThrow, throws } from 'node:assert';
import { test } from 'vitest';

import { TsvRejection } from './divergence/panic_errors.ts';
import { assert_binding_reports_rejection, type RejectProbeTarget } from './reject_probe.ts';
import { raise_wasm_trap } from './wasm_trap_fakes.ts';

/** A binding that refuses every source, as tsv's wrappers do: a `TsvRejection`. */
const refusing = (): RejectProbeTarget => {
	const refuse = (): never => {
		throw new TsvRejection('Unexpected token\n  at 1:1');
	};
	return { parse: refuse, parse_with_locations: refuse, parse_internal: refuse, format: refuse };
};

test('a binding that refuses the invalid source on every operation passes', () => {
	doesNotThrow(() => assert_binding_reports_rejection('tsv', refusing()));
});

test('a binding that accepts the invalid source fails', () => {
	throws(
		() => assert_binding_reports_rejection('tsv', { ...refusing(), format: (s) => s }),
		/tsv: format\[svelte\]\(\) ACCEPTED a source tsv rejects/
	);
});

test('a crash on the invalid source fails, on a parse or a format', () => {
	throws(
		() =>
			assert_binding_reports_rejection('tsv-wasm', {
				...refusing(),
				parse: () => raise_wasm_trap()
			}),
		/tsv-wasm: parse\[svelte\]\(\) CRASHED on a source tsv rejects \(RuntimeError: unreachable\)/
	);
	throws(
		() =>
			assert_binding_reports_rejection('tsv', {
				...refusing(),
				format: () => {
					throw new Error('panic: boom\n  at a frame');
				}
			}),
		/tsv: format\[svelte\]\(\) CRASHED on a source tsv rejects \(panic: boom\)/
	);
});

test('a rejection whose text reads `panic: ` is the refusal: the engine returned it', () => {
	const refuse = (): never => {
		throw new TsvRejection('panic: boom\n  at a frame');
	};
	doesNotThrow(() =>
		assert_binding_reports_rejection('tsv', {
			parse: refuse,
			parse_with_locations: refuse,
			parse_internal: refuse,
			format: refuse
		})
	);
});
