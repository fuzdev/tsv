<script lang="ts">
	// a `typeof` query's type arguments whose first argument opens with a generic function
	// type's `<` keep a space after their own `<`: tsc never splits a `<<` token there
	type Y1 = typeof f<<T>() => void>;
	type Y2 = typeof f<<T>() => void, U>;
	type Y3 = typeof a.b<<T>() => void /* c */>;

	// only the list's first argument follows the `<`
	type Y4 = typeof f<U, <T>() => void>;

	// a comment ahead of the argument already separates the two `<`
	type Y5 = typeof f</* c */ <T>() => void>;

	// a nested list is a type reference's, where tsc does split the `<<`, and a query nested in
	// one keeps its own space
	type Y6 = typeof f<A<<T>() => void>>;
	type Y7 = A<typeof f<<T>() => void>>;
</script>

{x as typeof f<<T>() => void>}
