<style>
	/* descendant combinator — the complex selector breaks per-combinator, and the :is() on the continuation line also breaks; its args indent relative to that line */
	a
		b:is(
			.aaaaaaaaaaaa,
			.bbbbbbbbbbbb,
			.cccccccccccc,
			.dddddddddddd,
			.eeeeeeeeeeee,
			.ffffffffffff,
			.gggggggggggg
		) {
		color: red;
	}
	/* child combinator variant — same continuation-indent behavior */
	a
		> b:is(
			.aaaaaaaaaaaa,
			.bbbbbbbbbbbb,
			.cccccccccccc,
			.dddddddddddd,
			.eeeeeeeeeeee,
			.ffffffffffff,
			.gggggggggggg
		) {
		color: blue;
	}
</style>
