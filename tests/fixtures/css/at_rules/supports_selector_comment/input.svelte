<style>
	/* A comment at a combinator boundary */
	@supports selector(div /* c1 */ > span) {
		div {
			color: red;
		}
	}

	/* A comment inside a functional pseudo-class argument */
	@supports selector(:has(/* c2 */ .class)) {
		div {
			color: red;
		}
	}

	/* A comment glued between two compound members stays glued — a space there
	would make the compound a descendant */
	@supports selector(.class1/* c3 */.class2) {
		div {
			color: red;
		}
	}

	/* A comment in an argument that is not a selector */
	@supports selector(. /* c4 */) {
		div {
			color: red;
		}
	}

	/* The same glue where the argument is not a selector: nothing is inserted */
	@supports selector([data-attr=1.50]/* c5 */.class) {
		div {
			color: red;
		}
	}

	/* Each side keeps the separation it was authored with */
	@supports selector([data-attr=1.50] /* c6 */.class) {
		div {
			color: red;
		}
	}

	/* A comment opening the argument stays glued to the token after it */
	@supports selector(/* c7 */[data-attr=1.50]) {
		div {
			color: red;
		}
	}
</style>
