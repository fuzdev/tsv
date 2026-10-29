<script lang="ts">
	// A `//` in a shell the member chain prints MID-CHAIN stays in the chain: it occupies
	// nothing at the `${` edge, and the interpolation hugs the chain
	const aaa = `x${(
		// c1
		bbb.ccc
	)
.ddd(eee)}y`;

	// A comment the leaf OWNS stays inside a pair the printer keeps (here the member object's
	// parens around a conditional), so the interpolation hugs too
	const fff = `x${(/* c2 */ ggg ? hhh : iii).jjj(function () {
		return 1;
	})}y`;
</script>
