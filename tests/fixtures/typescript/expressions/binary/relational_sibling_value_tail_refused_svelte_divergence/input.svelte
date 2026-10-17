<script lang="ts">
	// Two comparisons in comma siblings, to tsc and to acorn-typescript alike, which
	// `expected_svelte.json` records and tsv rejects: between the `<` and the `>` the
	// printed form of each line would spell a type-argument list, so the line would read
	// back as a generic call.

	// a paren shell the printer strips, leaving a name that takes a member or a list
	fn(a < b, (c).d > (e, f));
	fn(a < b, (c) < d >> (e, f));

	// a line break ahead of a `[`, which the printer folds into an indexed access
	fn(a < b, c
		[d] > (e, f));
</script>
