<style>
	/* a condition prelude holding only comments still emits them: the query has no
	parts, and the whole prelude span is the comments' only carrier */
	@container /* c */;

	@container /* c */ /* d */ {
		div {
			color: red;
		}
	}

	@container b /* c */ {
		div {
			color: red;
		}
	}
</style>
