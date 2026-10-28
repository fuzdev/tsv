<script lang="ts">
	// An own-line block in grouping parens the parser strips from around a computed
	// access in a call chain keeps its own line before that access's `[`, beside the
	// block the author trailed after the object.
	(
		/* c1 */
		((x.y) /* t1 */)[i]
	)[j].u();
	(
		/* c2 */
		((x?.y) /* t2 */)[i]
	)[j].u();
</script>
