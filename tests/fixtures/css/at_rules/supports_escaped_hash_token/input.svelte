<style>
	/* an escaped `#` is ident content, so the run after it belongs to the ident and is
	never lowercased as a hex colour — the unescaped sibling is */
	@supports (a: x\#FFF) and (b: #fff) {
		c {
			color: red;
		}
	}
	/* the same at the head of a value */
	@supports (a: \#FFF) and (b: c) {
		d {
			color: red;
		}
	}
	/* control: `@media` preserves every hash token, escaped or not */
	@media (a: x\#FFF) and (b: #FFF) {
		e {
			color: red;
		}
	}
</style>
