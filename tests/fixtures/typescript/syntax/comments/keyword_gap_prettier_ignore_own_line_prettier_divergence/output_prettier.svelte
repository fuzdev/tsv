<script lang="ts">
	// an honored directive keeps its own line in a declaration-header gap, even where
	// nothing freezes — a relocated directive is inert, so its placement is never
	// reflowed
	function // prettier-ignore
	fn(a, b) {
		return 1;
	}

	class // prettier-ignore
	A extends B {}
</script>
