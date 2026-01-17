<style>
	@supports not /* comment */ (display: grid) {
		div {
			color: red;
		}
	}

	@supports not /* a */ selector(:has(div)) {
		div {
			color: blue;
		}
	}
</style>
