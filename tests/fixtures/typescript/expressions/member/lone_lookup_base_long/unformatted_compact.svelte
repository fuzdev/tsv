<script lang="ts">
	// Prettier's `shouldInline` takes a lone `.prop`'s break point back only when the object
	// AND the property are both plain identifiers (member.js). Every other base or property
	// keeps the break point, and the lookup drops onto its own line once it passes width.

	class Q extends R {
		#aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa=1;

		m(objx:Q){
			// object and property both identifiers: inlined, so the lookup holds its line past width
			objx.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;

			// `this` is not an identifier, so the break point stands - at exactly 100 it still fits
			this.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;

			// one char more, and the lookup takes its own line
			this
				.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;

			// `super` is not an identifier either
			super
				.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;

			// nor is a private name, whatever the object is
			objx
				.#aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;
		}
	}
</script>
