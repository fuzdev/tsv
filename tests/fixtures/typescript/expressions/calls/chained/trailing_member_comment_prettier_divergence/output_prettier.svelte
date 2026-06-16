<script lang="ts">
	// We keep comments before trailing members with chain broken.
	// Prettier hoists comments to after `=` and keeps chain inline.

	// Case 1: Line comment before .length
	const a =
		// comment
		items.filter((x) => x).length;

	// Case 2: Line comment before optional chain ?.length
	const b = // comment
	items.filter((x) => x)?.length;

	// Case 3: Multiple comments before .length
	const c =
		// comment 1
		// comment 2
		items.filter((x) => x).length;
</script>
