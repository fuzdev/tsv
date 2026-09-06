<script lang="ts">
	// Prettier's `isPoorlyBreakableMemberOrCallChain` bottoms out at an `Identifier` or a
	// `ThisExpression` — and at nothing else, `super` included. A chain that qualifies breaks
	// after the `=` and stays whole; one that does not falls through to the fluid layout,
	// where the `=` holds its line and the lookup takes the break instead.

	class Q extends R {
		m(objx: Q) {
			// an identifier root qualifies
			const aa =
				objx.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;

			// so does `this`
			const bb =
				this.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;

			// `super` does not - at exactly 100 the whole thing still fits
			const cc = super.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;

			// one char more, and the fluid layout breaks the lookup rather than the `=`
			const dd = super
				.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;
		}
	}
</script>
