<style>
	/* or connector */
	@supports (display: grid) or /* comment */ (flex: 1) {
		div {
			color: red;
		}
	}

	/* not keyword */
	@supports /* before not */ not (display: grid) {
		div {
			color: blue;
		}
	}

	/* multiple consecutive */
	@supports (display: grid) /* c1 */ /* c2 */ and (flex: 1) {
		div {
			color: green;
		}
	}
</style>
