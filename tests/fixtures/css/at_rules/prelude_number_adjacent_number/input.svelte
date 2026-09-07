<style>
	/* a number abutting the run before it is that run's own tail, but the two prelude
	paths disagree about which runs count, so one authoring has two canonical forms */
	@supports (a: 1a.50) and (b: 1.5.50) and (c: 0.5) and (d: $1.50) and (e: $a1.50) {
		f {
			color: red;
		}
	}
	@media (a: 1a0.5) and (b: 1.50.5) and (c: 0.5) and (d: $1.5) and (e: $a1.50) {
		f {
			color: red;
		}
	}
</style>
