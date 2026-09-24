<script>
	// prettier-ignore preserves the property's source verbatim
	const obj1 = {
		// prettier-ignore
		a:            1
	};

	// trailing line comment is still handled normally
	const obj2 = {
		b: 2,
		// prettier-ignore
		a:            1 // comment
	};

	// inner comment + spacing inside the value are preserved
	const obj3 = {
		// prettier-ignore
		a:            /* comment */          1
	};

	// SpreadElement can be ignored too
	const obj4 = {
		// prettier-ignore
		...rest
	};

	// nested: only the inner property is ignored, outer formats normally
	const obj5 = {
		b: {
			// prettier-ignore
			a: [1, 2,    3]
		}
	};

	// key shapes are preserved verbatim too: shorthand, computed, method
	const obj6 = {
		// prettier-ignore
		a,
		b
	};
	const obj7 = {
		// prettier-ignore
		[key]    :   1
	};
	const obj8 = {
		// prettier-ignore
		m(  a, b  ) {}
	};

	// a comment inside a paren shell the frozen slice already printed is NOT
	// re-claimed by the trailing-comma seam
	const obj9 = {
		// prettier-ignore
		a: (b /* comment */),
		c: 1
	};

	// the same on the last property, whose comma is never emitted
	const obj10 = {
		// prettier-ignore
		a: (b /* comment */)
	};

	// a line comment in that position keeps the closer on its own line
	const obj11 = {
		// prettier-ignore
		a: (b // comment
		),
		c: 1
	};

	// a frozen spread's stripped parens print inside its verbatim slice, own-line comment
	// included, and a comment written after the `)` stays on the `)`'s line
	const obj12 = {
		a,
		// prettier-ignore
		...(b
		/* i */) /* t */,
		c
	};

	const obj13 = {
		a,
		// prettier-ignore
		...(b
		/* i */) /* t */
	};

	const obj14 = {
		a,
		// prettier-ignore
		...(b // i
		) /* t */,
		c
	};

	// the `)` on a line of its own below the comment rides in the slice too
	const obj15 = {
		a,
		// prettier-ignore
		...(b
		/* i */
		) /* t */,
		c
	};
</script>
