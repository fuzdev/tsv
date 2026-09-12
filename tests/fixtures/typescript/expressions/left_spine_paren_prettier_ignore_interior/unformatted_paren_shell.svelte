<script lang="ts">
	// an own-line directive inside the grouping parens the parser erases ahead of a binary
	// chain's first operand freezes THAT operand, and the operands around it normalize
	const a = (
		// prettier-ignore
		{x:   1}
	) + {y:   2};

	// a logical chain answers the same way
	const b = (
		// prettier-ignore
		{x:   3}
	) ?? c;
</script>
