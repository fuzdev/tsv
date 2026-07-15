<script lang="ts">
	// A glued pair of blocks after the `[`-line comment stays glued.
	const a = {
		// force
		/* c2 */
		[/* c1 */ key]: 1
	};

	// Blocks the author put on their own lines keep them.
	const b = {
		// force
		[/* c1 */
		/* c2 */
		key]: 1
	};
</script>
