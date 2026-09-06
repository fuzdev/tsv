<style>
	/* a number glued to the ident before it is not a value token of its own */
	@supports (a: x1.50) and (b: .50px) {
		c {
			color: red;
		}
	}
	@media (min-width: .50px) and (a: x.50px) {
		c {
			color: red;
		}
	}
	@supports (a: --x.50) {
		c {
			color: red;
		}
	}
</style>
