<script lang="ts">
	// A glued pair before the first array element stays glued.
	const a = [
		/* c1 */ /* c2 */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong,
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbLong
	];

	// A glued pair between array elements stays glued.
	const b = [
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong,
		/* c1 */ /* c2 */ bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbLong
	];

	// A glued pair after a line comment stays glued.
	const c = [
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong,
		// line
		/* c1 */ /* c2 */
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbLong
	];

	// Blocks the author put on their own lines keep them.
	const d = [
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong,
		/* c1 */
		/* c2 */
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbLong
	];

	// An array pattern takes the pattern printer.
	const [
		/* c1 */ /* c2 */ eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeLong,
		ffffffffffffffffffffffffffffffffffffffffffffLong
	] = arr;
</script>
