<style>
	/* an escaped paren in a feature value is ident content, not a nested
	sub-condition, so the feature name still lowercases */
	@media (min-width: a\(b\)c) {
		a {
			color: red;
		}
	}
	/* the hex spelling of the same paren carries no raw paren at all */
	@media screen and (max-width: a\29 b) {
		a {
			color: red;
		}
	}
	/* a real function call in a sibling feature keeps its own nesting */
	@media (min-width: calc(1px)) and (max-width: a\(b\)c) {
		a {
			color: red;
		}
	}
</style>
