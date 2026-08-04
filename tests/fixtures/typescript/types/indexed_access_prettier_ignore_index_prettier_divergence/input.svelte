<script lang="ts">
	// an own-line directive between `[` and the index stays own-line and freezes the
	// index type
	type A = { b: 2 }[
		// prettier-ignore
		{x:   1}
	];

	// the frozen route's own index-to-`]` gap: a run of comments there stays distinct,
	// each on its own line at the bracket interior indent
	type B = { b: 2 }[
		// prettier-ignore
		{x:   1} // c1
		// c2
	];
</script>
