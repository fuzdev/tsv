<script lang="ts">
	// a `(`-headed region whose content is a non-null `!` type: `!T` and `T!` are tsc's
	// `JSDocNonNullableType`, so the compiler reads a generic call and leaves the error
	// to its checker. tsv follows it to the region and rejects, having no such type;
	// acorn-typescript has none either, backs off the failed list and reads the
	// comparison chain, which `expected_svelte.json` records
	const a1 = a < (!b) > (t, u);
	const a2 = a < (b!) > (t, u);
	const a3 = a < (!b) > `t`;
</script>
