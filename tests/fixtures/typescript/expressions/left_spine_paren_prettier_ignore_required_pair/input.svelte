<script lang="ts">
	// the frozen operand takes the pair its POSITION requires, outside the slice: an arrow
	// under a binary operator, which bare would swallow the rest of the chain into its body
	const a =
		// prettier-ignore
		(() => 1) + 1;

	// a ternary operand, which bare would take the `+ 1` into its alternate
	const b =
		// prettier-ignore
		(c  ?  d  :  e) + 1;

	// an assignment operand
	const f =
		// prettier-ignore
		(g  =  h) + 1;

	// a unary operand on the left of `**`, which the grammar forbids bare
	const i =
		// prettier-ignore
		(-j) ** 2;

	// a `??` chain's first operand
	const k =
		// prettier-ignore
		(() => 2) ?? l;
</script>
