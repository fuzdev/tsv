<script lang="ts">
	// A `<` whose left operand holds a `<` and whose right operand holds a `>>` that may
	// close the region the two open: the pairs the author wrote in the chain stay
	x1 = (f < A) < C >> `t`;
	x2 = (f < A) < C >> (t, u);
	x3 = (f < A) < C >> (a = b);
	x4 = (f < A < B) < C >>> `t`;
	x5 = (f.g < A) < C >> `t${u}`;

	// A pair behind `typeof` or a unary `-` inside the region, in either operand
	x9 = f < typeof (b) < C >> `t`;
	x10 = f < -(b) < C >> `t`;
	x18 = (f < A) < typeof (c) >> `t`;

	// The follower is not read: the pair stays ahead of one that commits nothing
	x11 = (f < A) < C >> t;

	// The closer behind a call, a binary pair or a unary in the right operand: no type
	// argument, so the bare line reads as written and the pair is kept as a cost
	x6 = (f < A) < g(C) >> `t`;
	x7 = (f < A) < (C * D) >> `t`;
	x8 = (f < A) < -C >> `t`;
	fn((f < A) < g(C) >> `t`);

	// An inner link may bound the region: the outer `<` holds no closer
	x15 = (f < A) < C >> `t` < D;

	// A comment in or beside the pair stays where it was written
	x16 = (/* c */ f < A) < C >> `t`;
	x17 = (f < A /* c */) < C >> `t`;

	// No pair is added where none was written, and none is kept with no closer
	x12 = f < 1 < C >> `t`;
	x13 = f < A < C;
	x14 = f < A < C >= `t`;
	x20 = f < A < C >>> `t`;

	// A statement, an argument, a condition
	(f < A) < C >> `t`;
	fn((f < A) < C >> `t`);
	if ((f < A) < C >> `t`) {
	}
</script>

{(f < A) < C >> `t`}
{#if (f < A) < C >> `t`}text{/if}
