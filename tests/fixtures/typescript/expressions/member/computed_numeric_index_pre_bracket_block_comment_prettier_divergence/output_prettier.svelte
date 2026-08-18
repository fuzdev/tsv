<script lang="ts">
	// An own-line block comment before a numeric index keeps its own line, exactly
	// as at every other index kind — the bracket follows on the next line.
	const a =
		arr[/* c1 */
		0];

	// The same gap at the end of a call chain.
	const b =
		foo.bar()[/* c2 */
		0];
</script>
