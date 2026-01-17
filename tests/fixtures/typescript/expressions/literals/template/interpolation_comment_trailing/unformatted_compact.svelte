<script lang="ts">
	// Block comment after expression
	const a = `${expr/* comment */}`;

	// Multiple trailing comments
	const b = `${expr/* comment 1 *//* comment 2 */}`;

	// Trailing comment with complex expression
	const c = `${obj.method().chain()/* result */}`;

	// Both leading and trailing comments
	const d = `${// leading
		expr/* trailing */}`;
</script>
