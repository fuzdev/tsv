<style>
	/* A bare number after an implicit descendant combinator inside :is() is an
	   Nth node (parseCss: descendant Combinator + Nth), not a parse error */
	:is(.a 123) {
		color: red;
	}

	/* An An+B in descendant position structures the same way */
	:is(.a 2n + 1) {
		color: blue;
	}

	/* :not() and :where() complex-selector lists behave identically */
	:not(.a 123) {
		color: green;
	}

	:where(.a 123) {
		color: teal;
	}

	/* :has() relative-selector list: same descendant-Nth support */
	:has(.a 123) {
		color: purple;
	}

	/* Unknown functional pseudo-class reaches the same combinator path */
	:heading(.a 123) {
		color: orange;
	}

	/* Multiple descendant hops before the terminal Nth */
	:is(div span 123) {
		color: maroon;
	}

	/* Explicit child combinator already structured (regression guard) */
	:is(.a > 123) {
		color: navy;
	}
</style>
