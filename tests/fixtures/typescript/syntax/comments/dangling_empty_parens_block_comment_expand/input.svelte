<script lang="ts">
	// Consecutive block comments - each on its own line
	fn(
		/* a */
		/* b */
	);

	// Function declaration - consecutive block comments in empty params
	function b(
		/* a */
		/* b */
	) {}

	// Arrow function - consecutive block comments in empty params
	let a = (
		/* a */
		/* b */
	) => {};

	// Function type - consecutive block comments in empty params
	type F = (
		/* a */
		/* b */
	) => void;

	// Multi-line block comment - breaks the parens
	fn(
		/* a
	   b */
	);

	// A single block comment that fits stays inline
	fn(/* a */);
</script>
