<script lang="ts">
	// A glued pair of blocks after the `[`-line comment stays glued.
	const a =
		/* c1 */ /* c2 */
		obj // force
		[idx];

	// Blocks the author put on their own lines keep them.
	const b =
		/* c1 */
		/* c2 */
		obj // force
		[idx];

	// The optional form takes the same path.
	const c =
		/* c1 */ /* c2 */
		obj // force
		?.[idx];
</script>
