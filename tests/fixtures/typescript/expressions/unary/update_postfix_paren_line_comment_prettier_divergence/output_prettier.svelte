<script lang="ts">
	// A line comment before a postfix operator keeps the grouping parens that hold it.
	a++; // c1

	// Same for `--`.
	b--; // c2

	// A member operand.
	arr[0]++; // c3

	// A cast operand needs the parens anyway and takes no second pair.
	(c as T)++; // c4

	// A multiline block comment breaks the line too, so it keeps the parens — and a
	// shell that breaks expands rather than gluing its operand to the `(`.
	d /* m1
	m2 */++;

	// Two stacked line comments each keep their own line.
	e++; // c5
	// c6

	// The shell's other gap: a comment on the `(` line stays on it.
	// c8
	g++; // c9

	// An own-line block above the operand keeps its own line inside the shell.
	/* c10 */
	h++; // c11

	// A leading comment alone still keeps the shell that holds it.
	// c12
	i++;

	// A single-line block comment stays inline without parens.
	f /* c7 */++;
</script>
