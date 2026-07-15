<script lang="ts">
	// An own-line block comment between members disqualifies the union hug, so each
	// position that can hold a union breaks after its keyword instead of gluing.

	// `=` - the type-alias RHS
	type A =
		| { a: 1 }
		/* c */
		| null;

	// `:` - an annotation
	let b:
		| { a: 1 }
		/* c */
		| null;

	// a mapped-type value
	type C = {
		[K in T]:
			| { a: 1 }
			/* c */
			| null;
	};

	// a conditional check
	type D =
		| { a: 1 }
		/* c */
		| null extends U
		? 1
		: 2;

	// a conditional branch
	type E = T extends U
		? | { a: 1 }
			/* c */
			| null
		: 2;

	// a type argument
	type F = Foo<
		| { a: 1 }
		/* c */
		| null
	>;

	// a type argument in expression position
	const g = fn<
		| { a: 1 }
		/* c */
		| null
	>();

	// Controls - nothing disqualifies the hug, so it still happens.
	type H = { a: 1 } | null;
	type I = Foo<{ a: 1 } | null>;
	const j = fn<{ a: 1 } | null>();
</script>
