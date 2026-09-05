/**
 * `GATE_CHECKOUT_IDS.pins` is the provenance record for every pinned count —
 * which checkout each was measured against — and it is a hand-maintained list, so
 * this grades it in both directions against the module's own exports: a pin that
 * names no checkout has no recorded provenance (the shape that once let three
 * harvest pins go stale unnoticed), and a listed name that matches no export is a
 * ghost left by a rename. Reads the source text rather than `import *`, because the
 * question is about what the file DECLARES: an `export const` is the unit the
 * update ritual (docs/gate_counts.md) re-pins.
 */

import { deepStrictEqual, ok } from 'node:assert';
import { readFileSync } from 'node:fs';

import { GATE_CHECKOUT_IDS } from './gate_counts.ts';

/**
 * Pins measured over inputs that have no checkout commit to record — each with the
 * reason, so an addition here is an argument rather than an exemption.
 */
const UNTRACKED_PINS: Record<string, string> =
	{
		// Empty since the real-code corpus became the pinned `../corpora` snapshot: the
		// one former entry (the svelte-styles block count, a minimum over live working
		// trees) is now an exact pin measured at that checkout's commit.
	};

/**
 * Exported constants that are NOT pinned counts — each with the reason, same posture
 * as {@link UNTRACKED_PINS}. The pin list is computed as "every `export const` minus
 * these" rather than matched on the `_PIN` / `_PINS` / `_MIN` suffixes the module
 * happens to use: a suffix match grades only the names that already look like pins,
 * so a future `export const FOO_COUNT` would be invisible to BOTH directions below —
 * the same missing-provenance shape this file exists to close.
 */
const NON_PIN_EXPORTS: Record<string, string> = {
	GATE_CHECKOUT_IDS: 'the provenance table itself — what the pins are graded against'
};

const source = readFileSync(new URL('./gate_counts.ts', import.meta.url), 'utf8');

/** Every exported constant this module declares, minus the declared non-pins. */
const exported_pins = [...source.matchAll(/^export const ([A-Za-z0-9_]+)\b/gm)]
	.map((m) => m[1]!)
	.filter((name) => !(name in NON_PIN_EXPORTS));

/** Whether a `pins` entry (exact name, or a `PREFIX_*` glob) names `pin`. */
const names = (entry: string, pin: string): boolean =>
	entry.endsWith('*') ? pin.startsWith(entry.slice(0, -1)) : entry === pin;

const all_entries = Object.entries(GATE_CHECKOUT_IDS).flatMap(([repo, { pins }]) =>
	pins.map((entry) => ({ repo, entry }))
);

Deno.test('gate_counts exports the pins this test expects to grade', () => {
	// A regex that matched nothing would pass both directions vacuously.
	ok(exported_pins.length >= 10, `found only ${exported_pins.length} exported pins`);
	ok(exported_pins.includes('CSS_REJECTS_PIN'));
	ok(exported_pins.includes('CORPUS_FORMAT_MATCH_MIN'));
});

Deno.test('NON_PIN_EXPORTS lists only constants that exist', () => {
	for (const name of Object.keys(NON_PIN_EXPORTS)) {
		ok(
			new RegExp(`^export const ${name}\\b`, 'm').test(source),
			`${name} is not an export of gate_counts.ts`
		);
	}
});

Deno.test('every exported pin names the checkout it was measured against', () => {
	const orphans = exported_pins.filter(
		(pin) => !(pin in UNTRACKED_PINS) && !all_entries.some(({ entry }) => names(entry, pin))
	);
	deepStrictEqual(
		orphans,
		[],
		`pins with no checkout in GATE_CHECKOUT_IDS (add them to a checkout's \`pins\`, ` +
			`or to UNTRACKED_PINS with a reason): ${orphans.join(', ')}`
	);
});

Deno.test('every listed pin name resolves to an export', () => {
	const ghosts = all_entries.filter(({ entry }) => !exported_pins.some((pin) => names(entry, pin)));
	deepStrictEqual(
		ghosts.map(({ repo, entry }) => `${repo}: ${entry}`),
		[],
		'GATE_CHECKOUT_IDS names a pin gate_counts.ts no longer exports'
	);
});

Deno.test('UNTRACKED_PINS lists only pins that exist and are not also tracked', () => {
	for (const pin of Object.keys(UNTRACKED_PINS)) {
		ok(exported_pins.includes(pin), `${pin} is not exported`);
		ok(!all_entries.some(({ entry }) => names(entry, pin)), `${pin} is both tracked and untracked`);
	}
});
