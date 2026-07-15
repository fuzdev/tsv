<script lang="ts">
	// A glued pair of blocks after the `[`-line comment stays glued.
	const a = obj[ // force
		/* c1 */ /* c2 */
		idx
	];

	// Blocks the author put on their own lines keep them.
	const b = obj[ // force
		/* c1 */
		/* c2 */
		idx
	];

	// The optional form takes the same path.
	const c = obj?.[ // force
		/* c1 */ /* c2 */
		idx
	];
</script>
