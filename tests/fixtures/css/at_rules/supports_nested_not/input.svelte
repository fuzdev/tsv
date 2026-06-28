<style>
	/* a `not` directly inside a parenthesized condition keeps the paren tight */
	@supports (not (display: grid)) {
		.a {
			color: red;
		}
	}

	/* nested `not` inside an and-joined group */
	@supports ((display: grid) and (not (gap: 1px))) {
		.b {
			color: blue;
		}
	}
</style>
