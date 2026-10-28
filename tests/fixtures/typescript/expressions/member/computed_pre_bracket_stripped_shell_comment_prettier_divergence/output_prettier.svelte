<script lang="ts">
	// An own-line block in grouping parens the parser strips from around a computed
	// access in a call chain leads the chain, ahead of the block the author trailed
	// after the object, which keeps its place before the `[`.
	/* c1 */
	x.y /* t1 */[i][j]
		.u();
	/* c2 */
	(x?.y /* t2 */)[i][j].u();
</script>
