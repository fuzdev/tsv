<script lang="ts">
	// An index whose body opens on `{` or `[` is graded no further than a type-argument
	// head that opens on one: the region commits on any matching `>` a committing follower
	// stands past, and the type parse then rejects a body that is a value — where tsc and
	// acorn-typescript both read the comparison chain `expected_svelte.json` records. This
	// is a tsv over-rejection, not a sanctioned reading.
	const a1 = a < b[{ c: d + 1 }] > (t, u);
	const a2 = a < b[{ ...s }] > (t, u);
	const a3 = a < b[[c || d]] > (t, u);
	const a4 = a < b[[c++]] > `t`;
	const a5 = a < b[[c, d + 1]] > (t, u);
	const a6 = a < b[[c + 1][0]] > (t, u);
	// behind a member, and behind an earlier index
	const a7 = a < b | c[{ ...s }] > (t, u);
	const a8 = a < b[c][{ ...s }] > (t, u);
	// as an arrow function's body, which stands where a function type's return does
	const a9 = a < b[() => [c + 1]] > (t, u);
	const a10 = a < b[() => { return c; }] > (t, u);
</script>
