<script lang="ts">
	// A non-null assertion on a bare optional chain (no trailing access) has redundant
	// parens: the `!` is type-only and applies to the whole chain regardless, so tsv
	// drops them.
	const r1 = (a?.b)!;
	// Same for a chain ending in an optional call.
	const r2 = (a?.())!;
</script>
