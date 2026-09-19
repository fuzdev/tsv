<script lang="ts">
	// A comment run on lines of its own above a chain whose heads force the break is a
	// leading own-line comment: the chain takes its default shape below it, at every seam.

	// declarator, block
	const a = /* c */
		({}) =>
			() =>
				b;

	// declarator, line comment
	const d =
		// c
		({}) =>
			() =>
				b;

	// declarator, preserved multi-line block
	const e = /* c
d */
		({}) =>
			() =>
				b;

	// declarator, indentable block on lines of its own: own-line too, the default shape
	const g =
		/**
		 * c
		 */
		({}) =>
			() =>
				b;

	// assignment
	f = /* c */
		({}) =>
			() =>
				b;

	class C {
		// class property, block
		p =
			/* c */
			({}) =>
				() =>
					b;

		// class property, line comment
		q =
			// c
			({}) =>
				() =>
					b;

		// class property, preserved multi-line block
		r =
			/* c
d */
			({}) =>
				() =>
					b;
		// class property, indentable block on lines of its own
		s =
			/**
			 * c
			 */
			({}) =>
				() =>
					b;
	}

	const obj = {
		// object property
		p:
			/* c */
			({}) =>
				() =>
					b
	};
</script>
