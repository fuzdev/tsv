<style>
	/* An @scope prelude wraps at 101 chars: the clause that does not fit opens its parens */

	/* 100 chars - the limit clause stays inline */
	@scope (.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) to (.b > .c) {
		.a {
			color: red;
		}
	}

	/* 101 chars - the limit clause opens */
	@scope (.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) to (.b > .c) {
		.b {
			color: red;
		}
	}

	/* 100 chars, root clause only - stays inline */
	@scope (.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) {
		.c {
			color: red;
		}
	}

	/* 101 chars, root clause only - the root clause opens */
	@scope (.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) {
		.d {
			color: red;
		}
	}

	/* 121 chars inline - a selector list that does not fit breaks one selector per line */
	@scope (.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1, .aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa2, .aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa3) to (.b) {
		.e {
			color: red;
		}
	}

	/* the limit clause opens and the root's line, with its `) to (`, is 100 chars */
	@scope (.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) to (.b) {
		.f {
			color: red;
		}
	}

	/* one more and that line would be 101 chars - the root clause opens instead */
	@scope (.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) to (.b) {
		.g {
			color: red;
		}
	}

	/* the root clause's first selector is measured through its `,` alone: 100 chars stays inline */
	@scope (.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:is(.xx, .yy), .b) to (.c) {
		.h {
			color: red;
		}
	}

	/* 101 chars - its pseudo-class arguments break */
	@scope (.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:is(.xx, .yy), .b) to (.c) {
		.i {
			color: red;
		}
	}

	/* the limit clause's first selector, likewise: 100 chars stays inline */
	@scope (.r) to (.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:is(.xx, .yy), .b) {
		.j {
			color: red;
		}
	}

	/* 101 chars - its pseudo-class arguments break */
	@scope (.r) to (.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:is(.xx, .yy), .b) {
		.k {
			color: red;
		}
	}

	/* a clause's last selector is measured through nothing: its `)` lands on the next line */
	@scope (.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:is(.xx, .yy)) {
		.l {
			color: red;
		}
	}

	/* 101 chars - its pseudo-class arguments break */
	@scope (.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:is(.xx, .yy)) {
		.m {
			color: red;
		}
	}

	/* 100 chars, no block - the `);` is the whole suffix, stays inline */
	@scope (.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);

	/* 101 chars, no block - the root clause opens */
	@scope (.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
</style>
