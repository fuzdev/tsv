<script lang="ts">
	// `abstract` as an ordinary type name at the head of a parenthesized type —
	// only `abstract new` opens a constructor type

	// Parens required
	type A1 = (abstract | A)[];
	type A2 = (abstract & A)[];
	type A3 = (abstract extends A ? B : C)[];
	type A4 = keyof (abstract | A);

	// Redundant parens are stripped
	type B1 = (abstract | A);
	type B2 = (abstract<T>);
	type B3 = (abstract[K]);
	type B4 = (abstract.a);
	type B5 = (abstract);

	// The constructor type
	type C1 = (abstract new () => T)[];

	// A parameter name
	type D1 = (abstract) => U;
	type D2 = (abstract: T, b) => U;
</script>
