<script lang="ts">
	// Object destructuring binding default.
	const {
		/* c1 */
		a = 1
	} = obj;

	// Array destructuring binding default.
	const [
		/* c2 */
		b = 1
	] = arr;

	// Function parameter destructuring default.
	function fn({
		/* c3 */
		c = 1
	}) {}

	// Standalone parameter default.
	function g(
		/* c4 */
		d = 1
	) {}
</script>
