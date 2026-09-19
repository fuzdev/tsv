<script lang="ts">
	// a `(`-headed region whose content spells a PARAMETER LIST: tsc's
	// `isUnambiguouslyStartOfFunctionType` claims the whole group, so the compiler
	// commits to type arguments and reports `'=>' expected.`. tsv follows it and rejects;
	// acorn-typescript backs off the failed list and reads the comparison chain, which
	// `expected_svelte.json` records
	const a1 = a < (b = c) > (t, u);
	const a2 = a < (b ? c : d) > (t, u);
	const a3 = a < (b, c) > (t, u);
	const a4 = a < (b: C) > (t, u);
</script>
