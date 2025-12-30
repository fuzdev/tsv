<style>
	/* :is() - 100 chars exactly - stays inline (at boundary) */
	div:is(.aaaaaa, .bbbbbb, .cccccc, .dddddd, .eeeeee, .ffffff, .gggggg, .hhhhhh, .iiiiiiiiiiiiiii) {
		color: red;
	}

	/* :is() - 101 chars - wraps (exceeds print_width) */
	div:is(
		.aaaaaa,
		.bbbbbb,
		.cccccc,
		.dddddd,
		.eeeeee,
		.ffffff,
		.gggggg,
		.hhhhhh,
		.iiiiiiiiiiiiiiii
	) {
		color: blue;
	}

	/* :not() - 100 chars exactly - stays inline (at boundary) */
	div:not(.aaaaaa, .bbbbbb, .cccccc, .dddddd, .eeeeee, .ffffff, .gggggg, .hhhhhh, .iiiiiiiiiiiiii) {
		color: green;
	}

	/* :not() - 101 chars - wraps (exceeds print_width) */
	div:not(
		.aaaaaa,
		.bbbbbb,
		.cccccc,
		.dddddd,
		.eeeeee,
		.ffffff,
		.gggggg,
		.hhhhhh,
		.iiiiiiiiiiiiiii
	) {
		color: yellow;
	}

	/* :where() - 100 chars exactly - stays inline (at boundary) */
	div:where(.aaaaaa, .bbbbbb, .cccccc, .dddddd, .eeeeee, .ffffff, .gggggg, .hhhhhh, .iiiiiiiiiiii) {
		color: purple;
	}

	/* :where() - 101 chars - wraps (exceeds print_width) */
	div:where(
		.aaaaaa,
		.bbbbbb,
		.cccccc,
		.dddddd,
		.eeeeee,
		.ffffff,
		.gggggg,
		.hhhhhh,
		.iiiiiiiiiiiii
	) {
		color: orange;
	}
</style>
