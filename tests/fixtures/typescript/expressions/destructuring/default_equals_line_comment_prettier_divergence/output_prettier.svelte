<script lang="ts">
	// Object destructuring: a line comment between the key and `=` stays trailing
	// the key; the `= 1` default breaks to its own line.
	const {
		a = 1 // c
	} = obj;

	// Array destructuring
	const [
		b = 1 // c
	] = arr;

	// Function parameter destructuring
	function fn({
		c = 1 // c
	}) {}

	// Standalone parameter default
	function g(
		d = 1 // c
	) {}

	// Non-shorthand destructuring: the binding sits after the `:`.
	const {
		k: v = 1 // c
	} = obj;

	// A run whose last comment is a block: the block and the `=` still take the
	// continuation line the first comment forced.
	function h(
		/* c2 */ e = 1 // c1
	) {}
</script>
