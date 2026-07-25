<script lang="ts">
	// an own-line directive in a `new` argument gap freezes only the following argument
	new A(
		{ a: 1 },
		// prettier-ignore
		{ b:   2 },
		{ c: 3 }
	);

	// before the first argument
	new A(
		// prettier-ignore
		{ a:   1 },
		{ b: 2 }
	);

	// a lone huggable argument freezes and expands instead of hugging
	new A(
		// prettier-ignore
		{ a:   1 }
	);

	new A(
		// prettier-ignore
		[a,   b]
	);

	new A(
		// prettier-ignore
		function () {
			a(1,   2);
		}
	);

	// a lone arrow argument
	new A(
		// prettier-ignore
		() => a  +  b
	);

	// an author blank line beside a frozen argument is preserved
	new A(
		a,

		// prettier-ignore
		{ b:   2 }
	);

	// an own-line block spelling freezes identically
	new A(
		/* prettier-ignore */
		{ a:   1 },
		{ b: 2 }
	);

	// a block comment glued before a frozen argument is owned by it and stays put
	new A(
		// prettier-ignore
		/* c */ function () {
			a(1,   2);
		}
	);

	// a frozen FIRST argument followed by an author blank line — the blank-line layout
	// owns the leading gap here
	new A(
		// prettier-ignore
		{ a:   1 },

		{ b: 2 }
	);

	// a sequence argument keeps its own grouping parens around the frozen slice
	new A(
		// prettier-ignore
		(0,   1),
		b
	);
</script>
