<style>
	/* tsv indents broken pseudo-class args one level relative to the selector,
	   uniformly — the compound's simple-selector count does not matter */
	.a:is(
		.aaaaaaaaaaaaaaaa,
		.bbbbbbbbbbbbbbbb,
		.cccccccccccccccc,
		.dddddddddddddddd,
		.eeeeeeeeeeeeeeee
	) {
		color: red;
	}
	/* 3 simple selectors (.a.b + :is) — still one level; prettier adds an extra */
	.a.b:is(
			.aaaaaaaaaaaaaaaa,
			.bbbbbbbbbbbbbbbb,
			.cccccccccccccccc,
			.dddddddddddddddd,
			.eeeeeeeeeeeeeeee
		) {
		color: blue;
	}
	/* 4 simple selectors (.a.b.c + :is) — still one level */
	.a.b.c:is(
			.aaaaaaaaaaaaaaaa,
			.bbbbbbbbbbbbbbbb,
			.cccccccccccccccc,
			.dddddddddddddddd,
			.eeeeeeeeeeeeeeee
		) {
		color: green;
	}
</style>
