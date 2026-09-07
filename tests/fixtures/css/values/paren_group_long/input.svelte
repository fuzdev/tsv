<style>
	div {
		/* comma-separated group - 100 chars exactly - stays inline (at boundary) */
		top: (aaaaaaaaaa, bbbbbbbbbb, cccccccccc, dddddddddd, eeeeeeeeee, ffffffffff, gggggggggg, hhhh);

		/* comma-separated group - 101 chars - every member takes its own line */
		right: (
			aaaaaaaaaa,
			bbbbbbbbbb,
			cccccccccc,
			dddddddddd,
			eeeeeeeeee,
			ffffffffff,
			gggggggggg,
			hhh
		);

		/* space-separated group - 100 chars exactly - stays inline (at boundary) */
		left: (aaaaaaaaaa bbbbbbbbbb cccccccccc dddddddddd eeeeeeeeee ffffffffff gggggggggg hhhhhhhhhh);

		/* space-separated group - 101 chars - the group opens, its members stay on one line */
		bottom: (
			aaaaaaaaaa bbbbbbbbbb cccccccccc dddddddddd eeeeeeeeee ffffffffff gggggggggg hhhhhhhhh
		);

		/* the members exceed the width too - they pack greedily, continuation-indented */
		inset: (
			aaaaaaaaaa bbbbbbbbbb cccccccccc dddddddddd eeeeeeeeee ffffffffff gggggggggg
				hhhhhhhhhhhhhhhhhhhhhhhhh
		);
	}
</style>
