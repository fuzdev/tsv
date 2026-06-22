<script lang="ts">
	// Arrow callback should stay inline with expect, break at .toThrow
	expect(() => fn_with_longer_name(items, { option_flag: true })).toThrow(
		/error pattern that requires multiple words here/
	);
</script>
