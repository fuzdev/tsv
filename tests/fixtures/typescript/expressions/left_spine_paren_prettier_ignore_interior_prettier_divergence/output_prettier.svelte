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

	// the test of a NESTED conditional in a branch: its erased shell is the enclosing `?` / `:`
	// gap, so the run hoists there, and the next pass freezes the whole branch from it
	const q = cond
		? r
		: // prettier-ignore
			{x:   6}
			? s
			: t;

	const u = cond
		? // prettier-ignore
			{x:   7}
			? v
			: w
		: y;

	// the block spelling of the same shell forces the parent open the same way
	const z = cond
		? a1
		: /* prettier-ignore */
			{x:   8}
			? b1
			: c1;
</script>
