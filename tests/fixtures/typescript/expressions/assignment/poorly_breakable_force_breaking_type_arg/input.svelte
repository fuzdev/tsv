<script lang="ts">
	// A lone type argument that breaks on its own (an expanded object type nested in it, or a
	// string literal type with a line continuation) is a break point: the call stays on the `=` line

	// Object type nested in a type reference
	a = w.f<
		A<{
			a: 1;
		}>
	>();
	const b1 = w.f<
		A<{
			a: 1;
		}>
	>();

	// `keyof` an object type
	a = w.f<
		keyof {
			a: 1;
		}
	>();
	const b2 = w.f<
		keyof {
			a: 1;
		}
	>();

	// Array of an object type
	a = w.f<
		{
			a: 1;
		}[]
	>();
	const b3 = w.f<
		{
			a: 1;
		}[]
	>();

	// Mapped type nested in a type reference
	a = w.f<
		A<{
			[K in X]: 1;
		}>
	>();
	const b4 = w.f<
		A<{
			[K in X]: 1;
		}>
	>();

	// String literal type with a line continuation
	a = w.f<'a\
b'>();
	const b5 = w.f<'a\
b'>();

	// Object property value and class field
	const o = {
		pppppppp: w.f<
			A<{
				a: 1;
			}>
		>()
	};
	class C {
		p = w.f<
			A<{
				a: 1;
			}>
		>();
	}

	// A newline that breaks nothing: no break point, the `=` breaks
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa =
		w.fffffffffffffffffffffffffffffff<A<B>>();

	// A mapped type wrapped inside its member only: no break point, the `=` breaks
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa =
		w.fffffffffffffffffffffffffffffff<{ [K in X]: 1 }>();
</script>
