<script lang="ts">
	// Object destructuring computed key.
	const {
		[x] // c
			: a
	} = o;

	// Function parameter destructuring computed key.
	function fn({
		[y] // c1
			: f
	}) {}
</script>
