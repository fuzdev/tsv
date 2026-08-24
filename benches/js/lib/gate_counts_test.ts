/**
 * `GATE_CHECKOUT_COMMITS.pins` is the provenance record for every pinned count —
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

import { GATE_CHECKOUT_COMMITS } from './gate_counts.ts';

/**
 * Pins measured over inputs that have no checkout commit to record — each with the
 * reason, so an addition here is an argument rather than an exemption.
 */
const UNTRACKED_PINS: Record<string, string> = {
	SVELTE_STYLES_BLOCKS_MIN:
		'a MINIMUM over the live dev repos (perf-view `real` tier), which are unversioned working trees'
};

const source = readFileSync(new URL('./gate_counts.ts', import.meta.url), 'utf8');

/** Every exported pin constant, by the suffixes the module uses for them. */
const exported_pins = [...source.matchAll(/^export const ([A-Z0-9_]+(?:_PIN|_PINS|_MIN))\b/gm)].map(
	(m) => m[1]!
);

/** Whether a `pins` entry (exact name, or a `PREFIX_*` glob) names `pin`. */
const names = (entry: string, pin: string): boolean =>
	entry.endsWith('*') ? pin.startsWith(entry.slice(0, -1)) : entry === pin;

const all_entries = Object.entries(GATE_CHECKOUT_COMMITS).flatMap(([repo, { pins }]) =>
	pins.map((entry) => ({ repo, entry }))
);

Deno.test('gate_counts exports the pins this test expects to grade', () => {
	// A regex that matched nothing would pass both directions vacuously.
	ok(exported_pins.length >= 10, `found only ${exported_pins.length} exported pins`);
	ok(exported_pins.includes('CSS_REJECTS_PIN'));
	ok(exported_pins.includes('CORPUS_FORMAT_MATCH_MIN'));
});

Deno.test('every exported pin names the checkout it was measured against', () => {
	const orphans = exported_pins.filter(
		(pin) => !(pin in UNTRACKED_PINS) && !all_entries.some(({ entry }) => names(entry, pin))
	);
	deepStrictEqual(
		orphans,
		[],
		`pins with no checkout in GATE_CHECKOUT_COMMITS (add them to a checkout's \`pins\`, ` +
			`or to UNTRACKED_PINS with a reason): ${orphans.join(', ')}`
	);
});

Deno.test('every listed pin name resolves to an export', () => {
	const ghosts = all_entries.filter(({ entry }) => !exported_pins.some((pin) => names(entry, pin)));
	deepStrictEqual(
		ghosts.map(({ repo, entry }) => `${repo}: ${entry}`),
		[],
		'GATE_CHECKOUT_COMMITS names a pin gate_counts.ts no longer exports'
	);
});

Deno.test('UNTRACKED_PINS lists only pins that exist and are not also tracked', () => {
	for (const pin of Object.keys(UNTRACKED_PINS)) {
		ok(exported_pins.includes(pin), `${pin} is not exported`);
		ok(!all_entries.some(({ entry }) => names(entry, pin)), `${pin} is both tracked and untracked`);
	}
});
