<style>
	/* Universal selector with a named namespace prefix */
	svg|* {
		fill: red;
	}

	/* Universal selector in any namespace */
	*|* {
		color: blue;
	}

	/* Namespaced universal selector followed by subclass selectors */
	svg|*.class1 {
		stroke: green;
	}

	*|*:hover {
		opacity: 0.5;
	}

	/* Namespaced universal selectors across combinators */
	svg|* > *|* + svg|rect {
		margin: 0;
	}

	/* Inside a selector-list pseudo-class argument */
	:is(svg|*, *|*) {
		padding: 0;
	}
</style>
