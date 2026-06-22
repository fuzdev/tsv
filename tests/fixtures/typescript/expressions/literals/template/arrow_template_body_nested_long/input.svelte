<script lang="ts">
	// Nested template with arrow body — outer ${} hugs, inner breaks
	const x = `${items.map(
		(x) =>
			`${
				valueeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
			}`
	)}`;
</script>
