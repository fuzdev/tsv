<script lang="ts">
	// The TAIL of a `=>`→body comment run the author BROKE AFTER is glued to the body's
	// first token, so the body OWNS it: it travels inside the body's own doc, where the
	// reprint force-breaks a group prettier keeps flat. The arrow body claims such a
	// comment outside that group — the value gap's fallback where the hoist declines — so
	// the body stays flat below the run, exactly as under a run glued through
	// (`body_multiline_block_comment_flat`).

	// a single-line block broken after, ahead of a multi-line block glued to a binary body
	const a1 = (x1) =>
		/* c */
		/**
		 * d
		 */ b1 + c1 + d1;

	// a logical chain asks the same question
	const a2 = (x2) =>
		/* c */
		/**
		 * d
		 */ b2 && c2 && d2;

	// the glued tail preserved (non-indentable)
	const a3 = (x3) =>
		/* c */
		/* d
e */ b3 + c3 + d3;

	// a longer run: the glued pair keeps its line, the break survives, the member chain
	// below stays flat
	const a4 = (x4) =>
		/**
		 * c
		 */ /* d */
		/**
		 * e
		 */ b4.c4().d4().e4();

	// the same body as a call argument
	fn1(
		(x5) =>
			/* c */
			/**
			 * d
			 */ b5 + c5 + d5
	);

	// null control: a run glued through takes the hoist instead, and is flat already
	const a6 = (x6) =>
		/* c */ /**
		 * d
		 */ b6 + c6 + d6;

	// null control: a single-line tail owns nothing multi-line, so nothing is claimed
	const a7 = (x7) =>
		/* c */
		/* d */ b7 + c7 + d7;
</script>
