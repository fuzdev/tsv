<script lang="ts">
	// Width alone re-lexes a bare relational chain: the first break the printer takes
	// after the `>` makes `canFollowTypeArgumentsInExpression` read a preceding line
	// terminator and accept the type-argument list. The pair is layout-blind — it is
	// there at every width, including the ones that never break.

	// With the pair, the continuation line below is exactly 100 and the chain stays flat.
	const l1 =
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaa < bbbbbbbbbbbbbbbbbbbbbbbbbbbbb) > ccccccccccccccccccccccccccccc;

	// One character longer in the right operand than `l1`: with the pair, the flat form no
	// longer fits, so the break lands after the `>` — the join the pair protects.
	const l2 =
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaa < bbbbbbbbbbbbbbbbbbbbbbbbbbbbb) >
		cccccccccccccccccccccccccccccc;

	// Wider still, the operands break one per line and the `<` operand hangs.
	const l3 =
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) >
		cccccccccccccccccccccccccccccccccccccccccccccc;
</script>
