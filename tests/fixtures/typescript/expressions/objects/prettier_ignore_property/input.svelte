<script>
	// prettier-ignore preserves the property's source verbatim
	const obj1 = {
		// prettier-ignore
		a:            1,
	};

	// trailing line comment is still handled normally
	const obj2 = {
		b: 2,
		// prettier-ignore
		a:            1, // comment
	};

	// inner comment + spacing inside the value are preserved
	const obj3 = {
		// prettier-ignore
		a:            /* comment */          1,
	};

	// SpreadElement can be ignored too
	const obj4 = {
		// prettier-ignore
		...rest,
	};

	// nested: only the inner property is ignored, outer formats normally
	const obj5 = {
		b: {
			// prettier-ignore
			a: [1, 2,    3],
		},
	};

	// key shapes are preserved verbatim too: shorthand, computed, method
	const obj6 = {
		// prettier-ignore
		a,
		b,
	};
	const obj7 = {
		// prettier-ignore
		[key]    :   1,
	};
	const obj8 = {
		// prettier-ignore
		m(  a, b  ) {},
	};
</script>
