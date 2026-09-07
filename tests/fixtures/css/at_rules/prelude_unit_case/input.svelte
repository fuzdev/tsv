<style>
	/* the media reader lowercases a number's unit before its `<an+b>` gate and prints
	the lowercased one; the value reader has no gate and passes an unknown unit's case
	straight through, so one authoring has two canonical forms */
	@supports (a: 2N) and (b: 2N + 1) {
		c {
			color: red;
		}
	}
	@media (a: 2n) and (b: 2n + 1) {
		c {
			color: red;
		}
	}
</style>
