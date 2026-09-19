<script lang="ts">
	// A comment run in the TERMINAL `=>`→body gap of a curried arrow chain that the author
	// BROKE AFTER. That gap takes the same broke-after pair the default arrow layout's own
	// `=>`→body arms take, so the authored break survives wherever that machinery keeps it.
	// The multi-line block glued to the body still leaves the body flat
	// (`curried_body_multiline_block_comment_flat`).

	// a single-line block broken after, ahead of a multi-line block glued to the body
	const a1 = (x1) => (y1) =>
		/* c */
		/**
		 * d
		 */ (b1 ? c1 : d1);

	// the glued tail preserved (non-indentable)
	const a2 = (x2) => (y2) =>
		/* c */
		/* d
e */ (b2 ? c2 : d2);

	// three heads ask the same question
	const a3 = (x3) => (y3) => (z3) =>
		/* c */
		/**
		 * d
		 */ (b3 ? c3 : d3);

	// the break inside a longer run: the glued pair keeps its line, the break survives
	const a4 = (x4) => (y4) =>
		/**
		 * c
		 */ /* d */
		/**
		 * e
		 */ (b4 ? c4 : d4);

	// the same chain as a call argument
	fn1(
		(x5) => (y5) =>
			/* c */
			/**
			 * d
			 */ (b5 ? c5 : d5)
	);

	// null control: a run glued through keeps the chain layout's glued emission
	const a6 = (x6) => (y6) =>
		/* c */ /**
		 * d
		 */ (b6 ? c6 : d6);
</script>
