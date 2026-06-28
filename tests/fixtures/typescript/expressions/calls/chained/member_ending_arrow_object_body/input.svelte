<script lang="ts">
	// Arrow callback returning object, ending with .length
	items.map((x) => ({
		value: x
	})).length;

	// Longer chain with same pattern
	items
		.filter((x) => x > 0)
		.map((x) => ({
			value: x,
			doubled: x * 2
		})).length;

	// With type annotation
	items.map((x): Result => ({
		value: x
	})).length;
</script>
