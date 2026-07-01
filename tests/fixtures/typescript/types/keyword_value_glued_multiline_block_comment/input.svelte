<script lang="ts">
	// A multiline block comment glued to its operand (operand on the comment's
	// closing line) collapses inline, matching prettier — it does NOT hang the
	// operand or break after `=`. Covers the shared keyword/bracket -> value gaps.

	// as cast
	const a = x as /* line1
	line2 */ A;

	// type predicate `is`
	function f(x): x is /* line1
	line2 */ T {
		return true;
	}

	// keyof operator (type-alias RHS stays on the `=` line)
	type K = keyof /* line1
	line2 */ B;

	// typeof query (type-alias RHS stays on the `=` line)
	type Q = typeof /* line1
	line2 */ x;

	// indexed access (type-alias RHS stays on the `=` line)
	type I = A[/* line1
	line2 */ K];

	// type-parameter constraint (the <…> expands; the bound stays inline)
	type H<
		T extends /* line1
	line2 */ U
	> = T;

	// conditional `extends` (the conditional breaks; the extends-type stays inline)
	type C = X extends /* line1
	line2 */ Y
		? A
		: B;
</script>
