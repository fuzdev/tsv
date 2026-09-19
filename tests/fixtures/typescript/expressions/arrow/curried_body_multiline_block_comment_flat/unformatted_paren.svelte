<script lang="ts">
	// A multi-line block comment leading the TERMINAL body of a curried arrow chain must
	// not BREAK the body's own group, exactly as at a plain arrow body
	// (`body_multiline_block_comment_flat`): the chain layout is an eighth route into the
	// `=>`→body gap and reads the same value-gap rule, so a body that fits stays flat
	// below the comment whether the comment is OWNED by the body's first token or
	// gap-emitted ahead of the author's redundant parens (`unformatted_paren`).

	// binary chain
	const a1 = (x1) => (y1) =>
		/**
		 * c
		 */ (b1 + c1 + d1);

	// ternary
	const a2 = (x2) => (y2) =>
		/**
		 * c
		 */ (b2 ? c2 : d2);

	// member chain
	const a3 = (x3) => (y3) =>
		/**
		 * c
		 */ (b3.c3().d3().e3());

	// logical chain
	const a4 = (x4) => (y4) =>
		/**
		 * c
		 */ (b4 && c4 && d4);

	// a preserved (non-indentable) multi-line block asks the same question
	const a5 = (x5) => (y5) =>
		/* c
d */ (b5 + c5 + d5);

	// the same chain as a call argument, whose layout arm differs
	fn1(
		(x6) => (y6) =>
			/**
			 * c
			 */ (b6 + c6 + d6)
	);

	// null control: a single-line block forces no break
	const a7 = (x7) => (y7) => /* c */ (b7 + c7 + d7);

	// null control: no comment
	const a8 = (x8) => (y8) => b8 + c8 + d8;
</script>
