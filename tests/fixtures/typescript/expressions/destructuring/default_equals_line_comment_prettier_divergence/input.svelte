<script lang="ts">
	// Object destructuring: a line comment between the key and `=` stays trailing
	// the key; the `= 1` default breaks to its own line.
	const {
		a // c
			= 1
	} = obj;

	// Array destructuring
	const [
		b // c
			= 1
	] = arr;

	// Function parameter destructuring
	function fn({
		c // c
			= 1
	}) {}

	// Standalone parameter default
	function g(
		d // c
			= 1
	) {}
</script>
