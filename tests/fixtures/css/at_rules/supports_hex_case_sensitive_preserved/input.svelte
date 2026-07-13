<style>
	@supports selector(#ABC) {
		a {
			color: red;
		}
	}
	@supports (background: url(#ABC)) {
		a {
			color: green;
		}
	}
</style>
