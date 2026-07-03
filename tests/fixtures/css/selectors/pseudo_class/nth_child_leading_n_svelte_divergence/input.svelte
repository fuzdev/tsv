<style>
	/* Leading -n An+B forms that Svelte's :nth-child reader mis-parses (as a type
	   selector, or a flattened type-selector + combinator chain), where tsv reads a
	   single Nth per the css-syntax-3 An+B microsyntax. */
	a:nth-child(-n) {
		color: red;
	}

	a:nth-child(-n - 3) {
		color: blue;
	}
</style>
