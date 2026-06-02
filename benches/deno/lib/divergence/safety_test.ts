/**
 * Tests for the differential safety check (`check_safety_vs_prettier`).
 *
 * The check reports only the data loss/addition OUR output incurs BEYOND what
 * prettier incurs, per character: `real = max(0, ours_delta - prettier_delta)`.
 * These cases pin the false-positive guard (shared normalizations cancel) and
 * the real-loss detection (over-normalization, dropped comments, added chars).
 */

import { assertEquals } from 'https://deno.land/std@0.224.0/assert/mod.ts';
import { check_safety_vs_prettier } from './safety.ts';

Deno.test('vs_prettier: identical output is safe', () => {
	const v = check_safety_vs_prettier('const x = 1;\n', 'const x = 1;\n', 'const x = 1;\n');
	assertEquals(v, []);
});

Deno.test('vs_prettier: shared normalization cancels (no violation)', () => {
	// Both formatters strip the redundant leading `|`. Ours drops one `|` vs
	// source, prettier drops the same one — the differential remainder is empty.
	const source = 'type A = | B;\n';
	const ours = 'type A = B;\n';
	const prettier = 'type A = B;\n';
	assertEquals(check_safety_vs_prettier(source, ours, prettier), []);
});

Deno.test('vs_prettier: shared normalization + unrelated layout diff is still safe', () => {
	// Both strip the leading `|` (shared, cancels). Our output ALSO differs from
	// prettier on layout (line breaks), but layout uses excluded whitespace —
	// no semantic char is lost beyond prettier, so no safety violation. This is
	// the case the old all-or-nothing `ours !== prettier` guard got wrong.
	const source = 'type A = | B | C;\n';
	const prettier = 'type A = B | C;\n';
	const ours = 'type A =\n\tB | C;\n';
	assertEquals(check_safety_vs_prettier(source, ours, prettier), []);
});

Deno.test('vs_prettier: real loss beyond prettier is flagged', () => {
	// Prettier strips one leading `|`; ours strips that one AND drops a second
	// `|` that prettier keeps. Only the extra `|` is real.
	const source = 'type A = | B | C;\n';
	const prettier = 'type A = B | C;\n';
	const ours = 'type A = B C;\n';
	const v = check_safety_vs_prettier(source, ours, prettier);
	assertEquals(v.length, 1);
	assertEquals(v[0].type, 'content_lost');
	assertEquals(v[0].total, 1); // one `|` beyond prettier
	// Per-char breakdown carries the shared context: ours dropped both pipes (2),
	// prettier dropped one (1), so real = 1.
	assertEquals(v[0].chars, [{ char: '|', real: 1, ours: 2, prettier: 1 }]);
});

Deno.test('vs_prettier: dropped comment prettier keeps is flagged', () => {
	const source = 'const x = 1; // note\n';
	const prettier = 'const x = 1; // note\n';
	const ours = 'const x = 1;\n';
	const v = check_safety_vs_prettier(source, ours, prettier);
	assertEquals(v.length, 1);
	assertEquals(v[0].type, 'content_lost');
	// dropped chars: '/','/','n','o','t','e' (` ` excluded; `/` is semantic) → 6
	assertEquals(v[0].total, 6);
});

Deno.test('vs_prettier: added chars beyond prettier are flagged', () => {
	// A rendering bug that duplicates a token: ours has an extra `a` neither
	// source nor prettier has.
	const source = 'const x = a;\n';
	const prettier = 'const x = a;\n';
	const ours = 'const x = aa;\n';
	const v = check_safety_vs_prettier(source, ours, prettier);
	assertEquals(v.length, 1);
	assertEquals(v[0].type, 'content_added');
	assertEquals(v[0].total, 1); // one extra `a`
});

Deno.test('vs_prettier: loss shared with prettier (e.g. trailing-zero strip) cancels', () => {
	// Both formatters normalize `1.0` → `1`, dropping the `0`. Shared, so safe.
	const source = '.a {\n\tflex: 1.0;\n}\n';
	const ours = '.a {\n\tflex: 1;\n}\n';
	const prettier = '.a {\n\tflex: 1;\n}\n';
	assertEquals(check_safety_vs_prettier(source, ours, prettier), []);
});
