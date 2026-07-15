<script lang="ts">
	// A glued pair before the first function-type param stays glued.
	type A = (
		/* c1 */ /* c2 */
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong: string,
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbLong: number
	) => void;

	// A glued pair between function-type params stays glued.
	type B = (
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong: string,
		/* c1 */ /* c2 */
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbLong: number
	) => void;

	// A glued pair after a line comment stays glued.
	type C = (
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong: string,
		// line
		/* c1 */ /* c2 */
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbLong: number
	) => void;

	// Blocks the author put on their own lines keep them.
	type D = (
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong: string,
		/* c1 */
		/* c2 */
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbLong: number
	) => void;

	// A method signature takes the signature-params router.
	interface Single {
		m(
			/* c1 */ /* c2 */
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong: string,
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbLong: number
		): void;
	}
</script>
