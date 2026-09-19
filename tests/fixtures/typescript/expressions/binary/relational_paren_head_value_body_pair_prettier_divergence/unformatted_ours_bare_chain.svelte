<script lang="ts">
	// A `(` type-argument head whose body is a value, where the printer KEEPS the head's
	// shell: the chain is a comparison at a committing follower too, and it takes the pair
	// around the `>`'s left operand that every kept-shell chain takes. The
	// `unformatted_ours_bare_chain` variant carries the pair-less authoring of each line.

	// A `>` that closes no nested list ends the would-be region early, ahead of a
	// follower that continues the comparison.
	const p1 = x < (a > b) > (t, u);
	const p2 = x < (() => a > b) > (t, u);
	const p3 = x < (a<b>> c) > (t, u);

	// An arrow whose parameter is a bare name: no function type spells `=>` behind
	// anything but a parameter list's `)`.
	const p4 = x < (a => b) > (t, u);
	const p5 = x < (async a => b) > (t, u);
	const p6 = x < (a, b => c) > (t, u);

	// A regex literal, which no type begins with, and whose pattern may spell a `<`…`>`.
	const p7 = x < (a, /b<c>/) > (t, u);
	const p8 = x < (a, /b<c>/g) > `t`;

	// `infer` with no binding name behind it is an ordinary value name.
	const p9 = x < (infer < a) > (t, u);
</script>
