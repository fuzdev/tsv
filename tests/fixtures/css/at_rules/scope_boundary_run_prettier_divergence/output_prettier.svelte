<style>
	/* a boundary run at either end of a scope prelude stays where it was written */
	@scope (.a) {
		b {
			color: red;
		}
	}
	@scope to (.b) {
		c {
			color: red;
		}
	}
	@scope (.a) {
		b {
			color: red;
		}
	}
	@scope (.a) to (.b) {
		c {
			color: red;
		}
	}
</style>
