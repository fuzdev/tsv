<style>
	/* Spec-valid negative An+B forms (css-syntax-3 An+B microsyntax): a bare negative
	   <integer> B, a negative <n-dimension> A, a negative A with a negative offset,
	   and negative-zero. */
	a:nth-child(-3) {
		color: red;
	}

	a:nth-child(-2n) {
		color: blue;
	}

	a:nth-child(-2n - 3) {
		color: green;
	}

	a:nth-child(-0) {
		color: yellow;
	}
</style>
