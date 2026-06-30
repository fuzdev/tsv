<style>
	/* and/or/not are case-insensitive CSS grammar keywords; tsv preserves the
	   author's case (matching prettier). */
	@container sidebar (min-width: 100px) AND (max-width: 200px) {
		a {
			color: red;
		}
	}
	@container (min-width: 100px) or (min-height: 100px) {
		a {
			color: red;
		}
	}
</style>
