/**
 * `check_loc.ts`'s root argument: a path in the checkout, spelled as `loc_wires` emits the paths
 * under it, an absolute one beneath the checkout taken relative to it; and the code its failure
 * line names for how `loc_wires` ended.
 */

import { strictEqual } from 'node:assert';
import { test } from 'vitest';

import { normalize_root, to_ended_code } from './check_loc.ts';

test('a root is checkout-relative, without ./, doubled or trailing slashes', () => {
	strictEqual(normalize_root('tests/fixtures', '/r/tsv'), 'tests/fixtures');
	strictEqual(normalize_root('./tests//fixtures/css/', '/r/tsv'), 'tests/fixtures/css');
	strictEqual(normalize_root('././tests/fixtures', '/r/tsv'), 'tests/fixtures');
	strictEqual(normalize_root('./', '/r/tsv'), '.');
});

test('an absolute root beneath the checkout is taken relative to it, any other kept', () => {
	strictEqual(normalize_root('/r/tsv/tests/fixtures/css', '/r/tsv'), 'tests/fixtures/css');
	strictEqual(normalize_root('/r/tsv-fork/tests/fixtures', '/r/tsv'), '/r/tsv-fork/tests/fixtures');
});

test("loc_wires' code is its exit code, or 128 + N for a signal N", () => {
	strictEqual(to_ended_code({ kind: 'exited', code: 0 }), 0);
	strictEqual(to_ended_code({ kind: 'exited', code: 3 }), 3);
	strictEqual(to_ended_code({ kind: 'signaled', signal: 'SIGKILL' }), 137);
	strictEqual(to_ended_code({ kind: 'signaled', signal: 'SIGTERM' }), 143);
	strictEqual(to_ended_code({ kind: 'aborted', code: null, signal: 'SIGTERM' }), 143);
	strictEqual(to_ended_code({ kind: 'timed_out', code: 2, signal: null }), 2);
});
