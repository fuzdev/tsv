<script lang="ts">
	// Function declaration
	function fn1/* c1 */() {}

	// Function expression
	const a = function fn2/* c2 */() {};

	// Class expression (no heritage)
	const b = class A/* c3 */ {};

	// Type alias
	type B/* c4 */ = string;

	// Declare function
	declare function fn3/* c5 */(): void;
</script>
