/**
 * Tests for the differential safety check (`check_safety_vs_prettier`).
 *
 * The check reports only the data loss/addition OUR output incurs BEYOND what
 * prettier incurs, per character: `real = max(0, ours_delta - prettier_delta)`.
 * These cases pin the false-positive guard (shared normalizations cancel) and
 * the real-loss detection (over-normalization, dropped comments, added chars).
 */

import { deepStrictEqual as assertEquals } from 'node:assert';
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
	// Ours drops the `| C` union member's infix pipe that prettier keeps. The
	// source's own leading `= | B` pipe is excluded as break layout, so the only
	// counted pipes are the infix separators: source 1, prettier 1, ours 0.
	const source = 'type A = | B | C;\n';
	const prettier = 'type A = B | C;\n';
	const ours = 'type A = B C;\n';
	const v = check_safety_vs_prettier(source, ours, prettier);
	assertEquals(v.length, 1);
	assertEquals(v[0].type, 'content_lost');
	assertEquals(v[0].total, 1); // one infix `|` beyond prettier
	// ours dropped the one counted (infix) pipe, prettier dropped none → real = 1.
	assertEquals(v[0].chars, [{ char: '|', real: 1, ours: 1, prettier: 0 }]);
});

Deno.test('vs_prettier: leading-pipe union break (ours breaks, prettier inline) is safe', () => {
	// The `return_type_generic_union` divergence in method-signature position: tsv
	// keeps the params inline and breaks the return-type union with leading pipes
	// (`Resolvable<| A⏎| B⏎| C>`), where prettier breaks the params and keeps the
	// union inline. Ours grows one `|` per broken union — pure break layout, NOT
	// content. Excluding leading pipes cancels it (this is the gated SAFETY bug the
	// language-tools corpus onboarding hit on `interfaces.ts`).
	const source = 'foo(a: A, b: B): R<X | Y | Z>;\n';
	const prettier = 'foo(\n\ta: A,\n\tb: B\n): R<X | Y | Z>;\n';
	const ours = 'foo(a: A, b: B): R<\n\t| X\n\t| Y\n\t| Z\n>;\n';
	assertEquals(check_safety_vs_prettier(source, ours, prettier), []);
});

Deno.test('vs_prettier: bracket-hugged leading union pipe (`R<| A`) is safe', () => {
	// tsv also hugs the first member onto the `<` line with a leading pipe
	// (`R<| A`); that operand-less pipe (prev is `<`) is excluded like a
	// line-leading one, so no fabricated content_added.
	const source = 'x: R<A | B | C>;\n';
	const ours = 'x: R<| A\n\t| B\n\t| C>;\n';
	const prettier = 'x: R<A | B | C>;\n';
	assertEquals(check_safety_vs_prettier(source, ours, prettier), []);
});

Deno.test('vs_prettier: a dropped member in a broken union is still flagged', () => {
	// Excluding the leading pipe must not blind the check: ours breaks the union
	// AND drops the `C` member (identifier + its infix pipe). The infix-pipe drop
	// and the `C` letter drop both register.
	const source = 'x: R<A | B | C>;\n';
	const prettier = 'x: R<A | B | C>;\n';
	const ours = 'x: R<\n\t| A\n\t| B\n>;\n'; // dropped `| C`
	const v = check_safety_vs_prettier(source, ours, prettier);
	assertEquals(v.length, 1);
	assertEquals(v[0].type, 'content_lost');
	// counted pipes: source 2 (A|B, B|C), ours 1 (A|B) → 1 lost; plus the `C` and
	// its identifier — here `C` is a lone letter, so total = 1 (pipe) + 1 (`c`).
	assertEquals(v[0].total, 2);
	const pipe = v[0].chars.find((c) => c.char === '|');
	assertEquals(pipe, { char: '|', real: 1, ours: 1, prettier: 0 });
});

Deno.test('vs_prettier: space-separated leading pipes (`| | | |`) are all excluded', () => {
	// tsv can collapse nested single-member unions to space-separated leading pipes
	// (`type A = | ( | ( | X ))` → `type A = | | | X`). Every one of those pipes is
	// operand-less break layout — a space-separated `| |` is NOT `||`. Prettier keeps
	// the parens (its pipes are leading too), so both sides cancel: no fabricated
	// content_added (the real bug hit `prettier/tests/.../union/.../single-type.ts`).
	const source = 'type A = | ( | ( | X ) );\n';
	const ours = 'type A = | | | X;\n';
	const prettier = 'type A =\n\t| (\n\t\t| (\n\t\t\t| X\n\t\t)\n\t);\n';
	assertEquals(check_safety_vs_prettier(source, ours, prettier), []);
});

Deno.test('vs_prettier: `||` logical-or is fully counted (both pipes)', () => {
	// A `|` after `|` is `||`, not a leading union pipe — both pipes stay content,
	// so turning `||` into a single `|` (a catastrophic bug) is still flagged.
	const source = 'const x = a || b;\n';
	const prettier = 'const x = a || b;\n';
	const ours = 'const x = a | b;\n'; // dropped one `|` of `||`
	const v = check_safety_vs_prettier(source, ours, prettier);
	assertEquals(v.length, 1);
	assertEquals(v[0].type, 'content_lost');
	assertEquals(v[0].total, 1);
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

Deno.test('vs_prettier: opposite-direction case canonicalization is safe (CSS unit case)', () => {
	// tsv lowercases EVERY unit to its spec-canonical serialized form (`5CH`→`5ch`,
	// `5hZ`→`5hz`); prettier lowercases most but UPCASES the Hz/kHz/Q trio (`5hZ`→`5Hz`)
	// — the sanctioned `units_serialize_case` divergence. The shared lowercasing (`5CH`
	// both→`5ch`) gives source a surplus of `H` over ours; prettier keeping `5Hz`
	// uppercase means it drops fewer `H`, so a naive case-SENSITIVE differential leaves a
	// fabricated `H`-lost / `h`-added remainder. A case-only swap of a case-insensitive
	// token is canonicalization, never content loss — folding ASCII case cancels it.
	const source = 'a: 5CH;\n\tb: 5hZ;\n';
	const ours = 'a: 5ch;\n\tb: 5hz;\n';
	const prettier = 'a: 5ch;\n\tb: 5Hz;\n';
	assertEquals(check_safety_vs_prettier(source, ours, prettier), []);
});

Deno.test('vs_prettier: a genuine letter drop is still flagged despite case-folding', () => {
	// Case-folding must not mask a real loss: dropping a letter entirely still reduces
	// the folded count. Here ours drops the `c` from an identifier prettier keeps.
	const source = 'const abc = 1;\n';
	const prettier = 'const abc = 1;\n';
	const ours = 'const ab = 1;\n';
	const v = check_safety_vs_prettier(source, ours, prettier);
	assertEquals(v.length, 1);
	assertEquals(v[0].type, 'content_lost');
	assertEquals(v[0].total, 1); // the dropped `c`
});

Deno.test('vs_prettier: prettier-empty output never fabricates a violation', () => {
	// The prettier Deno sidecar can return '' for a file under load. The check
	// iterates the chars OURS deviates on and uses prettier only as a subtrahend,
	// so an empty prettier (prettier_excess = the whole source) can only CANCEL
	// deltas — never invent one. Such a file surfaces as a large unknown diff, not
	// SAFETY. Pins the false-positive direction of the prettier-sidecar heisenbug.
	const source = 'const x = abc;\n';
	assertEquals(check_safety_vs_prettier(source, source, ''), []); // ours == source
	assertEquals(check_safety_vs_prettier(source, 'const x = abc;\n', ''), []); // ours formatted
});

Deno.test('vs_prettier: prettier-empty MASKS a real loss (false negative — caller must guard)', () => {
	// The flip side: when prettier is empty AND ours genuinely drops a char, the
	// differential subtracts the loss away (prettier "dropped" it too) and reports
	// nothing. The primitive cannot tell real loss from a sidecar miss once
	// prettier is gone — which is why `corpus_compare_format.ts` errors out on
	// `prettier === '' && source non-empty` instead of trusting this verdict.
	const source = 'const x = abc;\n';
	const ours = 'const x = ab;\n'; // ours dropped the `c`
	assertEquals(check_safety_vs_prettier(source, ours, ''), []); // masked — NOT flagged
});
