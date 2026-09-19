<script lang="ts">
	// Width alone re-lexes a bare kept-shell chain: the first break the printer takes after
	// the `>` puts a line terminator at the join, and a `(`-headed region commits there. The
	// pair is layout-blind — it is there at every width, including the ones that never break.

	// With the pair, the continuation line below is exactly 100 and the chain stays flat.
	const l1 =
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa < (bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb = q) > cccccccccccccc;

	// One character longer in the right operand than `l1`: with the pair, the flat form no
	// longer fits, so the break lands after the `>` — the join the pair protects.
	const l2 =
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa < (bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb = q) > ccccccccccccccc;
</script>
