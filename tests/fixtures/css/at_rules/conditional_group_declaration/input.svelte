<style>
	/* Bare declarations parse directly in a conditional-group at-rule: CSS block
	   parsing is agnostic (CSS Syntax 3 §consume-a-block), so a declaration is
	   parsed here — parseCss keeps it. */
	@media screen {
		color: red;
		--x: blue;
	}

	/* A declaration in a conditional nested inside another conditional — the shape
	   @function/@mixin bodies use. */
	@container (width > 1px) {
		@supports (color: red) {
			color: green;
		}
	}

	/* A conditional block mixing a declaration and a nested rule. */
	@supports (color: red) {
		color: blue;
		div {
			color: red;
		}
	}
</style>
