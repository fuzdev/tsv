<script lang="ts">
	// A non-null assertion is not a cast that couldExpandArg looks through, so a
	// `{...}!` / `[...]!` argument does NOT hug the call parens — all args expand
	// onto their own lines (unlike `as`/`satisfies`/`<T>`, which do hug).

	// trailing object non-null — all args expand
	fn(
		head,
		{
			prop1: 1,
			prop2: 2
		}!
	);

	// single object non-null — expands
	fn(
		{
			prop1: 1,
			prop2: 2
		}!
	);

	// trailing array non-null (elements exceed print width) — all args expand
	fn(
		head,
		[
			elementaaaaaaaa,
			elementbbbbbbbb,
			elementcccccccc,
			elementdddddddd,
			elementeeee,
			elementffff,
			elementgggg
		]!
	);
</script>
