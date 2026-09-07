<style>
	/* a connector with nothing after it is the query's last word, not a break point */
	@import 'a.css' screen and;
	@import 'b.css' screen or;
	@import 'c.css' screen AND;
	@import 'd.css' screen and (color) and;

	/* a connector with no query before it is likewise not a break point */
	@import 'e.css' and;

	/* control: a connector that joins two segments keeps its break point */
	@import 'f.css' screen and (color);

	/* `@media` reaches the same wrap; its `{` absorbs the stray separator, the `;` cannot */
	@media screen and {
		.class {
			color: red;
		}
	}

	@media screen and (color) and {
		.class {
			color: red;
		}
	}
</style>
