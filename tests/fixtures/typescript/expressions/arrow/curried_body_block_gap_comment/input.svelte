<script lang="ts">
	// The TERMINAL `=>`→body gap of a curried arrow chain is answered by the same emitter the
	// default arrow layout's BLOCK body takes, so a comment needing a line of its own keeps
	// it and the `{` drops below — where reading the gap as glue-only welded the run onto the
	// `=>` line and left the brace flush behind it.

	// a `//` the author gave its own line
	const a1 = (x1) => (y1) =>
		// c
		{
			return pp;
		};

	// an own-line block
	const a2 = (x2) => (y2) =>
		/* c */
		{
			return pp;
		};

	// a run whose last comment is a `//`
	const a3 = (x3) => (y3) =>
		/* c */
		// d
		{
			return pp;
		};

	// two comments, the author breaking after the first
	const a4 = (x4) => (y4) =>
		/* c */
		/**
		 * d
		 */ {
			return pp;
		};

	// three heads, and a block whose own body carries a blank
	const a5 = (x5) => (y5) => (z5) =>
		// c
		{
			const aa = 1;

			return aa + pp;
		};

	// the chain layout's other context: a call argument
	fn1(aa, (x6) => (y6) =>
		// c
		{
			return pp;
		}
	);

	// null control: a single-line block glued to `=>` keeps the brace on its line
	const a7 = (x7) => (y7) => /* c */ {
		return pp;
	};

	// null control: a multi-line block the author glued to the `{` does too
	const a8 = (x8) => (y8) => /**
	 * c
	 */ {
		return pp;
	};
</script>
