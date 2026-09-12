<script lang="ts">
	// An own-line directive in a prefix type operator's keyword→operand gap freezes the
	// OPERAND. The slice is the operand's own node span, so an object literal, an array
	// type, a tuple and an indexed access each print verbatim.
	type A = keyof (
		// prettier-ignore
		{  a: 1  }
	);

	type B = readonly
		// prettier-ignore
		{  b: 2  }[];

	type C = readonly
		// prettier-ignore
		[  1,  2  ];

	type D = keyof (
		// prettier-ignore
		E[  'f'  ]
	);

	// The `typeof` type query is the same gap one keyword over. Its child is the entity
	// name, so the name→`<` gap comment and the type arguments — both past it — stay
	// parent-owned and normalize.
	type G = typeof
		// prettier-ignore
		h . i;

	type J = typeof
		// prettier-ignore
		k/* l */ <{ m: 3 }>;

	// The type arguments hang INSIDE the gap's indent, so a list that breaks lands exactly
	// where the same list does with no directive in the gap.
	type AA = typeof
		// prettier-ignore
		bb<
			Cccccccccccccccccccc,
			Dddddddddddddddddddd,
			Eeeeeeeeeeeeeeeeeeee,
			Ffffffffffffffffffff,
			Gggg
		>;

	// `infer` is the same gap once more. Its child is the type PARAMETER, so a constraint
	// rides inside the freeze.
	type T = U extends infer
		// prettier-ignore
		V extends {  w: 8  }
		? 1
		: 2;

	// A MULTI-LINE frozen operand keeps every one of its lines, hand-alignment and all —
	// the shape the directive is usually written for.
	type X = keyof (
		// prettier-ignore
		{
	  y : 1;
			z   : 2;
		}
	);

	// A union operand keeps its required parens and declines the whole-operand freeze:
	// the directive binds through the composite's own leading run, so the FIRST MEMBER
	// freezes and the rest normalizes.
	type M = keyof (
		// prettier-ignore
		{  n: 4  } | O
	);

	// An own-line block comment behaves identically — placement keys the freeze, not the
	// spelling.
	type P = keyof (
		/* prettier-ignore */
		{  q: 5  }
	);

	// A directive the author put on the keyword's line is INERT under the placement
	// floor: the comment keeps the line it was written on and the operand normalizes.
	type R = keyof // prettier-ignore
		{ s: 6 };
</script>
