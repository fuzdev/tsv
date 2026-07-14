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
</script>
