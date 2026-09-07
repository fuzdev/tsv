<style>
	/* a media feature's NAME position takes no number pass: the name is everything
	before the expression's first colon, so only the value normalizes */
	@media (1.50: 2.5) {
		a {
			color: red;
		}
	}
	/* an expression with no colon has no value at all, so its whole interior is name —
	the boolean form and both range forms keep their numbers */
	@media (0.50) and (width >= 1.50) and (1.50 < width < 2.50) {
		a {
			color: red;
		}
	}
	/* a function call in name position is name text like anything else there — and its
	parens do not protect the splitting colon, so a name may run INTO a call and the value
	start inside one */
	@media (calc(1.50): a) and (calc(1.50)) and (calc(1: 1.5): b) {
		a {
			color: red;
		}
	}
	/* the colon that splits an expression is its first one wherever it sits, so a
	grouped condition's value normalizes */
	@media (not (a: 1.5)) {
		a {
			color: red;
		}
	}
	/* a comment is name text, and the name runs on past it to the colon */
	@media (1.50 /* c */: a) {
		a {
			color: red;
		}
	}
	/* a `(` glued to an identifier is a function call in media-type position rather than
	a feature expression, so its content takes the number pass like the rest of the type */
	@media calc(1.5) and (calc(1.50): a) {
		a {
			color: red;
		}
	}
	/* @custom-media goes through the same reader */
	@custom-media --b (0.50);
</style>
