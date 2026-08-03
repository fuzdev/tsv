<script lang="ts">
	// Object destructuring computed key.
	const {
		// c
		[x]: a
	} = o;

	// Function parameter destructuring computed key.
	function fn({
		// c1
		[y]: f
	}) {}
</script>
