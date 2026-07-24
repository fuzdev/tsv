<script lang="ts">
	// an own-line directive between `[` and the index stays own-line and freezes the
	// index type
	type A = { b: 2 }[// prettier-ignore
	{x:   1}];
</script>
