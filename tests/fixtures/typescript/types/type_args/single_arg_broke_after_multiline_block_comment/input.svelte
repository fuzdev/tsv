<script lang="ts">
	// A multi-line block the author broke after, ahead of a sole type argument: the
	// argument no longer hugs, and the `<…>` list expands with the block on its own line.

	// type position
	type A = Foo<
		/* c1
	d1 */
		B
	>;

	// call position
	const c = fn<
		/* c2
	d2 */
		D
	>(e);

	// an object argument
	type F = Foo<
		/* c3
	d3 */
		{ a: 1 }
	>;

	// the indentable kind
	type G = Foo<
		/**
		 * c4
		 */
		H
	>;

	// a run: a single-line block glued ahead of the multi-line one
	type I = Foo<
		/* c5 */ /* c6
	d6 */
		J
	>;

	// control: a single-line block the author broke after collapses onto the argument
	type K = Foo</* c7 */ L>;

	// control: a multi-line block glued to the argument keeps the hug
	type M = Foo</* c8
	d8 */ N>;
</script>
