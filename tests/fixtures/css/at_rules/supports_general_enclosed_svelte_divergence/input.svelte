<style>
	@supports (margin: 0;) {
		a {
			color: red;
		}
	}
	@supports foo(a; b) {
		a {
			color: red;
		}
	}
</style>
