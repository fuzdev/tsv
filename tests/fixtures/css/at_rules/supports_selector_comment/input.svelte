<style>
	/* A comment at a combinator boundary */
	@supports selector(div /* c */ > span) {
		div {
			color: red;
		}
	}

	/* A comment inside a functional pseudo-class argument */
	@supports selector(:has(/* c */ .class)) {
		div {
			color: red;
		}
	}

	/* A comment in an argument that is not a selector */
	@supports selector(. /* c */) {
		div {
			color: red;
		}
	}
</style>
