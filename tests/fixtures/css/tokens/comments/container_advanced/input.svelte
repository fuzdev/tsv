<style>
	/* named with comment */
	@container sidebar /* after name */ (min-width: 100px) {
		div {
			color: red;
		}
	}

	/* or connector */
	@container (min-width: 100px) or /* comment */ (max-width: 200px) {
		div {
			color: blue;
		}
	}

	/* not keyword */
	@container /* before not */ not (min-width: 100px) {
		div {
			color: green;
		}
	}
</style>
