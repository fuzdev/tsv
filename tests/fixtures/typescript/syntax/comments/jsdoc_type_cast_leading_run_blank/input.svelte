<script>
	// A blank the author left between a cast's comment and its `(` is authoring, the
	// same as a blank after any other leading comment: ownership decides who PRINTS
	// the comment, not whether the gap below it carries one.
	a1();
	/** @type {A} */

	(b1);

	// The null control — the same authoring with no blank keeps the break alone.
	a2();
	/** @type {A} */
	(b2);

	// A run glued ahead of the cast leaves its comment mid-line, where the separator is
	// the width-decided one; a blank opens that gap too.
	a3();
	/* c1 */ /** @type {A} */

	(b3);

	// Its null control: glued ahead, no blank, so the author's break is all that is kept.
	a4();
	/* c2 */ /** @type {A} */
	(b4);

	// `@satisfies` names the same cast.
	/** @satisfies {A} */

	(b5);

	// A value gap hangs the value under the comment, and the blank hangs with it.
	const a5 =
		/** @type {A} */

		(b6);

	const a6 = () =>
		/** @type {A} */

		(b7);

	const o1 = {
		k1:
			/** @type {A} */

			(b8)
	};

	class C1 {
		p1 =
			/** @type {A} */

			(b9);
	}

	// A list keeps the blank at the element's own indent.
	fn(
		a7,
		/** @type {A} */

		(b10)
	);

	// A nested cast answers its own gap.
	/** @type {A} */ (
		/** @type {B} */

		(b11)
	);

	// A multi-line comment's own newlines are not an author blank; the line below the
	// `*/` is.
	/**
	 * @type {A}
	 */

	(b12);

	/**
	 * @type {B}
	 */
	(b13);

	// A block body takes the same rule.
	{
		/** @type {A} */

		(b14);
	}
</script>
