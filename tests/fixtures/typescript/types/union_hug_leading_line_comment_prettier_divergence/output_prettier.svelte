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

	// The same positions still hug when nothing disqualifies it.
	let g: { a: 1 } | null;
	type H = () => { a: 1 } | null;
	const i = expr as { a: 1 } | null;
</script>
