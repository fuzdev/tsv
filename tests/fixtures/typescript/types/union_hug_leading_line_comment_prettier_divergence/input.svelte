<script lang="ts">
	// A line comment before a hugging object member must survive.
	type A =
		| // c
			{ a: 1 }
		| null;

	// A line comment before a hugging type-reference member must survive.
	type B =
		| // c
			Foo
		| null;

	// A block comment in the same position already survives.
	type C = /* c */ { a: 1 } | null;

	// Every position that must agree with the printer about whether the hug happens
	// breaks after its keyword, exactly as `=` does above.
	let d:
		| // c
			{ a: 1 }
		| null;

	type E = () =>
		| // c
			{ a: 1 }
		| null;

	const f = expr as
		| // c
			{ a: 1 }
		| null;

	type G = {
		[K in T]:
			| // c
				{ a: 1 }
			| null;
	};

	type H =
		| // c
			{ a: 1 }
		| null extends U
		? 1
		: 2;

	type I = T extends U
		? | // c
				{ a: 1 }
			| null
		: 2;

	type J = Foo<
		| // c
			{ a: 1 }
		| null
	>;

	const k = fn<
		| // c
			{ a: 1 }
		| null
	>();

	// The same positions still hug when nothing disqualifies it.
	let l: { a: 1 } | null;
	type M = () => { a: 1 } | null;
	const n = expr as { a: 1 } | null;
</script>
