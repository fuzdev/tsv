<script lang="ts">
	// A line comment before the cast keyword keeps the grouping parens that hold it.
	const a = x as A; // c1

	// Same for `satisfies`.
	const b = x satisfies B; // c2

	// `as const` is no exception.
	const c = 1 as const; // c3

	// A chained cast keeps the comment on the inner cast's operand.
	const d = x as unknown as A; // c4

	// `satisfies` chained into `as`.
	const e = x satisfies B as A; // c5

	// An object operand.
	const f = { prop: 1 } as const; // c6

	// Two stacked line comments each keep their own line.
	const g = x as // c8 // c7
	A;

	// An operand that needs the parens anyway takes no second pair.
	const h = (a + b) as A; // c9

	// A bare expression statement, not a declarator.
	1 as const; // c10

	// A multiline block comment breaks the line too, so it keeps the parens — and a
	// shell that breaks expands rather than gluing its operand to the `(`.
	const j = x as /* m1
	m2 */
	A;

	// An object operand at statement position: the shell's `(` already keeps the
	// statement from starting with `{`, so no second pair is added.
	({ prop: 1 }) as const; // c19

	// The shell's other gap: a comment on the `(` line stays on it, with the operand
	// indented below.
	const k = // c12
	x as A; // c13

	// An own-line block above the operand keeps its own line inside the shell.
	const l = /* c14 */
	x as A; // c15

	// A block glued to the operand leads it inline instead.
	const m = /* c16 */ x as A; // c17

	// A leading comment alone still keeps the shell that holds it.
	const n = // c18
	x as A;

	// A block comment does not end its line, so it stays inline without parens.
	const i = x /* c11 */ as A;

	// an author blank BELOW the pulled comment survives — the blank is authorship, not
	// the container's leading gap (the blank ABOVE one stays erased, against the delimiter)
	const o = // c19

	x as A;
</script>
