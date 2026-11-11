/**
 * Tests for the snapshot reader's node-modules-free rules (`corpora.ts`).
 *
 * @module
 */

import { deepStrictEqual } from 'node:assert';
import { test } from 'vitest';

import { benched_own_collections } from './corpora.ts';

test('benched_own_collections: an own collection on the bench is refused', () => {
	deepStrictEqual(
		benched_own_collections([
			{ name: 'kit', benched: true, origin: 'third_party' },
			{ name: 'zzz', benched: false, origin: 'own' }
		]),
		[]
	);
	deepStrictEqual(
		benched_own_collections([
			{ name: 'kit', benched: true, origin: 'third_party' },
			{ name: 'zzz', benched: true, origin: 'own' },
			{ name: 'fuz_ui', benched: true, origin: 'own' }
		]),
		['zzz', 'fuz_ui']
	);
	// a third-party collection held off the bench is no violation either
	deepStrictEqual(
		benched_own_collections([{ name: 'layercake', benched: false, origin: 'third_party' }]),
		[]
	);
});
