<style>
	/* A `:` inside selector() opens a pseudo-class, not a declaration value */
	@supports selector(div:hover) {
		div {
			color: red;
		}
	}

	/* A space before the pseudo-class is a descendant combinator */
	@supports selector(div :hover) {
		div {
			color: red;
		}
	}

	/* A functional pseudo-class keeps its name glued to the colon */
	@supports selector(div:not(.class)) {
		div {
			color: red;
		}
	}

	/* `and` names a class here, so it is an identifier, not a boolean operator */
	@supports selector(.and) {
		div {
			color: red;
		}
	}

	/* The argument is a selector, normalized like one anywhere else */
	@supports selector(div > span) {
		div {
			color: red;
		}
	}

	@supports selector(div:is(.class1, .class2)) {
		div {
			color: red;
		}
	}

	@supports selector(div[data-attr='value']) {
		div {
			color: red;
		}
	}

	/* An argument that is not a selector at all stays as authored */
	@supports selector(.) {
		div {
			color: red;
		}
	}
</style>
