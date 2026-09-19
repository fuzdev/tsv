<script lang="ts">
	// a heritage clause's type arguments whose first argument opens with a generic function
	// type's `<` keep a space after their own `<`: tsc never splits a `<<` token in one
	const b1 = class extends fn< <T>(v: T) => void> {};
	class B2 extends a.b< <T>() => U, V> {}
	class B3 implements I< <T>() => U>, J< <T>() => U> {}
	interface B4 extends I< <T>() => U /* c */> {}

	// only the list's first argument follows the `<`
	class B5 extends fn<V, <T>() => U> {}

	// a comment ahead of the argument already separates the two `<`
	class B6 extends fn</* c */ <T>() => U> {}

	// a nested list is a type reference's, and a called superclass's is a call's: tsc splits
	// the `<<` at both
	class B7 extends fn<A<<T>() => U>> {}
	class B8 extends fn<<T>() => U>() {}
</script>

{class extends fn< <T>() => U> {}}
