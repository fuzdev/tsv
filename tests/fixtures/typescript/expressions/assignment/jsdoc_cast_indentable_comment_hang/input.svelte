<script>
	// An INDENTABLE multi-line cast comment hangs the value under the operator, the
	// rule every leading comment takes — the cast's own parens stand between the
	// comment and the value and change nothing about it.

	// declarator
	const a =
		/**
		 * @type {A}
		 */ (x);

	// assignment expression
	let b;
	b =
		/**
		 * @type {A}
		 */ (x);

	// object property
	const c = {
		k:
			/**
			 * @type {A}
			 */ (x)
	};

	// class field
	class C {
		d =
			/**
			 * @type {A}
			 */ (x);
	}

	// Contrast: a single-line cast comment is not indentable and keeps the value on
	// the operator's line.
	const e = /** @type {A} */ (x);

	// A cast comment the author gave a line of its own keeps it, and the `(` follows
	// on the line below — the cast's own hang, which this rule leaves alone.
	const f =
		/** @type {A} */
		(x);
</script>
