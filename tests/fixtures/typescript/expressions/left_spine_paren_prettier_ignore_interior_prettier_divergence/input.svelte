<script lang="ts">
	// an own-line directive inside the grouping parens the parser erases ahead of a
	// construct's leftmost operand freezes THAT operand: a ternary's test
	const a =
		// prettier-ignore
		{x:   1} ? b : c;

	// a member chain's base
	const d =
		// prettier-ignore
		{x:   2}.f;

	// a non-null assertion's operand
	const g =
		// prettier-ignore
		{x:   3}!;

	// a tagged template's tag
	const h =
		// prettier-ignore
		i`t`;

	// a sequence's first operand, the rest of the list normalizing around it
	const j =
		// prettier-ignore
		({x:   4}, { y: 5 });

	// a base the chain REQUIRES parens around keeps them, outside the frozen slice
	const k =
		// prettier-ignore
		(l  ?  m  :  n).o;

	// a callee the printer prints no pair around
	const p =
		// prettier-ignore
		{q:   1}(2);
</script>
