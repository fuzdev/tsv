<script lang="ts">
	// A glued pair of blocks leading the first union member stays glued.
	type A =
		| // force
		  /* c1 */ /* c2 */
		  Aaaa
		| Bbbb;

	// Blocks the author put on their own lines keep them.
	type B =
		| // force
		  /* c1 */
		  /* c2 */
		  Aaaa
		| Bbbb;
</script>
