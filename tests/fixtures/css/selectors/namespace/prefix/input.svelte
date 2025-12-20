<style>
	/* Type selectors with namespace */
	svg|rect {
		fill: red;
	}

	svg|circle {
		stroke: blue;
	}

	math|mi {
		font-style: italic;
	}

	/* Universal namespace with type selector */
	*|div {
		color: green;
	}

	/* Combined patterns */
	svg|rect.class1 {
		border: 1px solid red;
	}

	svg|text:hover {
		text-decoration: underline;
	}
</style>
