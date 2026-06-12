<style>
	@container not /* comment */ (width > 100px) {
		div {
			color: red;
		}
	}

	@container sidebar not /* a */ style(--theme: dark) {
		div {
			color: blue;
		}
	}
</style>
