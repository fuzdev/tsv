<style>
	/* :nth-child(An+B of S) where S is a bare An+B/number term (2n, 123) */
	a:nth-child(2n of 123) {
		color: red;
	}

	a:nth-child(2n of 2n) {
		color: blue;
	}

	a:nth-last-child(3 of 7) {
		color: green;
	}
</style>
