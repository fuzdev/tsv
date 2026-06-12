<script lang="ts">
	// We keep comments before trailing members with chain broken.
	// Prettier hoists comments to after `=` and keeps chain inline.

	// Case 1: Line comment before .length
	const a = items
		.filter((x) => x)
		// comment
		.length;

	// Case 2: Line comment before optional chain ?.length
	const b = items
		.filter((x) => x)
		// comment
		?.length;

	// Case 3: Multiple comments before .length
	const c = items
		.filter((x) => x)
		// comment 1
		// comment 2
		.length;
</script>
