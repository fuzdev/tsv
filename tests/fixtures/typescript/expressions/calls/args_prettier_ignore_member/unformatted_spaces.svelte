<script lang="ts">
	// an own-line directive in an argument gap freezes only the following argument;
	// the list stays broken one argument per line
	fn(
		{ a:   1 }  ,
		// prettier-ignore
		{ b:   2 },
			{ c:   3 }
	);

	// before the first argument: freezes that argument (Rule A — first like every)
	fn(
		// prettier-ignore
		{ a:   1 },
			{ b:   2 }
	);

	// an own-line block comment behaves identically — placement keys the freeze, not the spelling
	fn(
		/* prettier-ignore */
		{ a:   1 },
		{ b:     2 }
	);

	// a lone argument freezes too, and never hugs: a hug would pull the directive off its line
	fn(
		// prettier-ignore
		{ a:   1 }
	);

	// two arrow arguments take the always-expand layout
	fn(
		// prettier-ignore
		() => a  +  b,
		() =>   c
	);

	// a callback-bearing argument takes the function-composition layout
	fn(
		// prettier-ignore
		arr.map((x) => x  + 1),
			b
	);

	// an argument with multiline content takes the forced-expansion layout
	fn(
		// prettier-ignore
		{ a:   1 },
		'a\
b'
	);

	// a spread rides inside the frozen slice
	fn(
		// prettier-ignore
		...  a  .  b,
		  c
	);

	// an assignment argument keeps its clarity parens around the frozen slice
	fn(
		// prettier-ignore
		(a = b  +  c),
		d
	);

	// an author blank line beside a frozen argument is preserved
	fn(
		a  ,

		// prettier-ignore
		{ b:   2 }
	);

	// a multi-line frozen argument keeps its verbatim layout; the `,` is parent-owned
	fn(
			a,
		// prettier-ignore
		{
			b:   2,
				c: 3
		},
		d
	);

	// a block comment glued before a frozen argument is OWNED by it — it rides inside
	// the argument's own doc, which the frozen slice replaces, so the freeze claims it
	fn(
		// prettier-ignore
		/* c */ { a:   1 },
		  b
	);

	// a sequence expression prints its own grouping parens, and they are not part of its
	// node span — the frozen slice re-synthesizes them or the call changes arity
	fn(
		// prettier-ignore
		(0,   1),
		  b
	);
</script>
