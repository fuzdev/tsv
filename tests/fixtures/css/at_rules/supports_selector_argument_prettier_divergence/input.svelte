<style>
	/* A selector list stays on the prelude's line */
	@supports selector(.class1, .class2) {
		div {
			color: red;
		}
	}

	/* An attribute value must be an identifier or a string, so this is no selector */
	@supports selector([data-attr=1.50]) {
		div {
			color: red;
		}
	}
</style>
