<style>
	/* :has() with relative (leading-combinator) selectors — short, stays inline */
	div:has(> .a, + .b, ~ .c) {
		color: red;
	}
	/* :has() relative selectors wrap one-per-line; each broken arg keeps its leading combinator */
	div:has(
		> .aaaaaaaaaaaa,
		+ .bbbbbbbbbbbb,
		~ .cccccccccccc,
		> .dddddddddddd,
		+ .eeeeeeeeeeee,
		~ .ffffffffffff
	) {
		color: red;
	}
</style>
