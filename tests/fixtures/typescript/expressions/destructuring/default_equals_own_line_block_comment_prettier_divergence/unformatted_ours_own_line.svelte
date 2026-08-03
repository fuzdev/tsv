<script lang="ts">
	// Object destructuring binding default.
	const { a
	/* c1 */
	= 1 } = obj;

	// Array destructuring binding default.
	const [b
	/* c2 */
	= 1] = arr;

	// Function parameter destructuring default.
	function fn({ c
	/* c3 */
	= 1 }) {}

	// Standalone parameter default.
	function g(d
	/* c4 */
	= 1) {}
</script>
