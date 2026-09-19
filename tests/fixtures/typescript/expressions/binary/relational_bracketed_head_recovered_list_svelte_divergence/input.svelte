<script lang="ts">
	// A bracketed head whose body is plainly an expression is a comparison chain only
	// where tsc reads one. tsc does not guess at a `<`: it parses the type-argument list
	// for real, with error recovery, and claims the region whenever that recovery still
	// lands on the `>` — reporting the errors rather than backing off. Each body below is
	// one it lands on, so the compiler rejects the line; tsv follows it and rejects too,
	// while acorn-typescript backs off the failed list and reads the comparison chain that
	// `expected_svelte.json` records.
	const a1 = a < (b << c) > (t, u);
	const a2 = a < [b - 1] > (t, u);
	const a3 = a < [b * c] > (t, u);
	const a4 = a < [b as C] > (t, u);
	const a5 = a < [b satisfies C] > (t, u);
	const a6 = a < [b(c)] > (t, u);
	const a7 = a < [b.c(d)] > (t, u);
	const a8 = a < [void b] > (t, u);
	const a9 = a < { b: c(d) } > (t, u);
</script>
