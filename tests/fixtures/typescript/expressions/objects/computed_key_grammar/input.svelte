<script lang="ts">
	// a computed member key is a single assignment expression, not a sequence or spread
	const obj = {
		[a + b]: 1,
		[(x, y)]: 2,
		[(a = 0)]: 3
	};
</script>
