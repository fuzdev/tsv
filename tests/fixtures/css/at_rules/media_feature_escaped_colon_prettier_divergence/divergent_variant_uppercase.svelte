<style>
	/* an escaped `:` is ident content, so the feature name runs past it */
	@media (a\: B: 1px) {
		c {
			color: red;
		}
	}
	/* a custom-media name is case-sensitive, so the whole run survives */
	@media (--A\: B: 1px) {
		c {
			color: red;
		}
	}
</style>
