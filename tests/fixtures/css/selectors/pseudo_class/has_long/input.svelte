<style>
	/* 100 chars exactly - stays inline (at boundary) */
	div:has(.aaaaaa, .bbbbbb, .cccccc, .dddddd, .eeeeee, .ffffff, .gggggg, .hhhhhh, .iiiiiiiiiiiiii) {
		color: red;
	}

	/* 101 chars - wraps (exceeds print_width) */
	div:has(
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
		color: blue;
	}
</style>
