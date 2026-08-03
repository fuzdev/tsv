<script lang="ts">
	// A multiline block the author broke after keeps the break: `: value` drops to
	// a continuation line indented one level.
	const o = {
		/* a
b */
		key1: 1,
		// One whose value shares its closing line stays glued.
		key2 /* c
d */: 2
	};
</script>
