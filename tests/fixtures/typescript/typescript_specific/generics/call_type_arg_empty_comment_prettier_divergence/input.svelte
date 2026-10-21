<script lang="ts">
	// Empty object with block comment
	const a = fn<{ /* empty */ }>();

	// Empty object with line comment
	const b = fn<{
		// empty
	}>();

	// Empty object with block comment, on the last call of a member chain
	const c = db.where(filter).select<{ /* empty */ }>(columns);
</script>
