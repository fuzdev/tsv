<script lang="ts">
	// A multiline block the author broke after keeps the break: `= value` drops to
	// a continuation line indented one level.
	const a /* x
y */
		= 1;

	// One whose value shares its closing line stays glued.
	const b /* x
y */ = 2;
</script>
