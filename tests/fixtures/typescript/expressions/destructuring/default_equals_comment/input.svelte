<script lang="ts">
	// Object destructuring
	const {a /* c1 */ = 1} = obj;

	// Array destructuring
	const [b /* c2 */ = 1] = arr;

	// Function param destructuring
	function fn({c /* c3 */ = 1}) {}
</script>
