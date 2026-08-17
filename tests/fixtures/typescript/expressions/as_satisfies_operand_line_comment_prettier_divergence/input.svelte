<script lang="ts">
	// A line comment before the cast keyword keeps the grouping parens that hold it.
	const a = (
		x // c1
	) as A;

	// Same for `satisfies`.
	const b = (
		x // c2
	) satisfies B;

	// `as const` is no exception.
	const c = (
		1 // c3
	) as const;

	// A chained cast keeps the comment on the inner cast's operand.
	const d = (
		x // c4
	) as unknown as A;

	// `satisfies` chained into `as`.
	const e = (
		x // c5
	) satisfies B as A;

	// An object operand.
	const f = (
		{ prop: 1 } // c6
	) as const;

	// Two stacked line comments each keep their own line.
	const g = (
		x // c7
		// c8
	) as A;

	// An operand that needs the parens anyway takes no second pair.
	const h = (
		a + b // c9
	) as A;

	// A bare expression statement, not a declarator.
	(
		1 // c10
	) as const;

	// A multiline block comment breaks the line too, so it keeps the parens — and a
	// shell that breaks expands rather than gluing its operand to the `(`.
	const j = (
		x /* m1
	m2 */
	) as A;

	// An object operand at statement position: the shell's `(` already keeps the
	// statement from starting with `{`, so no second pair is added.
	(
		{ prop: 1 } // c19
	) as const;

	// The shell's other gap: a comment on the `(` line stays on it, with the operand
	// indented below.
	const k = ( // c12
		x // c13
	) as A;

	// An own-line block above the operand keeps its own line inside the shell.
	const l = (
		/* c14 */
		x // c15
	) as A;

	// A block glued to the operand leads it inline instead.
	const m = (
		/* c16 */ x // c17
	) as A;

	// A leading comment alone still keeps the shell that holds it.
	const n = ( // c18
		x
	) as A;

	// A block comment does not end its line, so it stays inline without parens.
	const i = x /* c11 */ as A;

	// an author blank BELOW the pulled comment survives — the blank is authorship, not
	// the container's leading gap (the blank ABOVE one stays erased, against the delimiter)
	const o = ( // c19

		x
	) as A;
</script>
