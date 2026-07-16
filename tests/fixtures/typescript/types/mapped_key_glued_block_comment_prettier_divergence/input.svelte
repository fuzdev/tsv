<script lang="ts">
	// A glued pair of blocks after the `[`-line comment stays glued.
	type A = {
		[ // force
			/* c1 */ /* c2 */
			K in keyof T
		]: T[K];
	};

	// Blocks the author put on their own lines keep them.
	type B = {
		[ // force
			/* c1 */
			/* c2 */
			K in keyof T
		]: T[K];
	};
</script>
