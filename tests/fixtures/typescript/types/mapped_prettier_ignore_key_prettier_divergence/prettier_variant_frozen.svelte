<script lang="ts">
	// an own-line directive inside the bracket freezes only the `K in ...` binding;
	// the value keeps formatting normally
	type A = {
		[
			// prettier-ignore
			K   in   keyof T
		]:   V;
	};
</script>
