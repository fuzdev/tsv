<style>
	/* and/or/not are case-insensitive CSS grammar keywords; tsv preserves the
	   author's case (matching prettier) — it parses the uppercase forms but
	   never rewrites them. */
	@supports (display: grid) AND (color: red) {
		a {
			color: red;
		}
	}
	@supports (display: grid) or (gap: 1rem) {
		a {
			color: red;
		}
	}
	@supports NOT (display: grid) {
		a {
			color: red;
		}
	}
</style>
