<script lang="ts">
	// A `{` or `[` type-argument head grades no body, so it commits on any matching `>` a
	// committing follower stands past and the type parse then rejects the region — where
	// tsc and acorn-typescript both read the comparison chain `expected_svelte.json`
	// records. This is a tsv over-rejection, not a sanctioned reading.
	const a1 = a < { ...s } > (t, u);
	const a2 = a < { b: c++ } > (t, u);
	const a3 = a < [b || c] > (t, u);
	const a4 = a < [~b] > (t, u);
	const a5 = a < [b++] > (t, u);
	const a6 = a < [b?.c] > (t, u);
	const a7 = a < [this.b] > (t, u);
	const a8 = a < [delete b.c] > (t, u);
	const a9 = a < [b in c] > (t, u);
	const a10 = a < [b, c++] > `t`;
</script>
