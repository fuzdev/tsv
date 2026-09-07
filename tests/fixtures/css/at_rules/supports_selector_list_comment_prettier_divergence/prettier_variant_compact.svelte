<style>
	/* The same seam in an `@import` prelude's supports() */
	@import url('a.css') supports(selector(.class1,/* c0 */.class2));

	/* A comment after the list comma leads the next selector */
	@supports selector(.class1,/* c1 */.class2) {
		div {
			color: red;
		}
	}

	/* A comment before the list comma trails the selector */
	@supports selector(.class1/* c2 */,.class2) {
		div {
			color: red;
		}
	}

	/* Both, in a three-member list */
	@supports selector(.class1/* c3 */,/* c4 */.class2,.class3) {
		div {
			color: red;
		}
	}

	/* A run of comments, and a selector with a combinator after the comma */
	@supports selector(
		.class1,/* c5 *//* c6 */.class2>.class3
	) {
		div {
			color: red;
		}
	}

	/* Beside a condition connector */
	@supports selector(.class1/* c7 */,.class2) and (display: grid) {
		div {
			color: red;
		}
	}
</style>
