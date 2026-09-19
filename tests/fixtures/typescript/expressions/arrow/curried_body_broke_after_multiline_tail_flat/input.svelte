<script lang="ts">
	// The curried chain's TERMINAL `=>`→body gap reaches the same rule through the same
	// body builder (`body_broke_after_multiline_tail_flat`): the tail of a run the author
	// BROKE AFTER is owned by the body's first token, and the body claims it outside its
	// own group, so a body prettier keeps flat stays flat however the heads broke.

	// two heads, a binary body
	const a1 = (x1) => (y1) =>
		/* c */
		/**
		 * d
		 */ b1 + c1 + d1;

	// three heads, a logical body, the tail preserved (non-indentable)
	const a2 = (x2) => (y2) => (z2) =>
		/* c */
		/* d
e */ b2 && c2 && d2;

	// heads a destructured parameter force-breaks, under an assignment
	const a3 =
		({ x3 }) =>
		({ y3 }) =>
			/* c */
			/**
			 * d
			 */ b3 + c3 + d3;

	// the same heads in call-argument position, where they stack progressively
	fn1(
		aa,
		({ x4 }) =>
			({ y4 }) =>
				/* c */
				/**
				 * d
				 */ b4 + c4 + d4
	);

	// a member-chain body under typed heads
	fn2(
		(x5: T5) => (y5: T5) =>
			/**
			 * c
			 */
			/* d */ b5.c5().d5()
	);

	// null control: a run glued through takes the hoist instead, and is flat already
	const a6 = (x6) => (y6) =>
		/* c */ /**
		 * d
		 */ b6 + c6 + d6;
</script>
