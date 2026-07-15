<script lang="ts">
	// A glued pair of blocks before the first element stays glued, then breaks.
	type A = [
		/* c1 */ /* c2 */
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	];

	// A glued pair between elements stays glued.
	type B = [
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		/* c1 */ /* c2 */
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	];

	// A glued pair sharing the gap with a line comment stays glued.
	type C = [
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		// line
		/* c1 */ /* c2 */
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	];

	// Blocks the author put on their own lines keep them.
	type D = [
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		/* c1 */
		/* c2 */
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	];

	// A same-line block does not force expansion.
	type E = [Aaaa, /* c */ Bbbb];
</script>
