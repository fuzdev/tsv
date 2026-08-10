<script lang="ts">
	// A glued pair before the first type argument of a call stays glued.
	const a = fn1<
		/* c1 */ /* c2 */
		AaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong,
		BbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbLong
	>(x);

	// A glued pair between call type arguments stays glued.
	const b = fn2<
		AaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong,
		/* c1 */ /* c2 */
		BbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbLong
	>(x);

	// A glued pair in a type-position type-argument list stays glued.
	type A = Foo<
		/* c1 */ /* c2 */
		AaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong,
		BbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbLong
	>;

	// A glued pair after a line comment stays glued.
	type B = Foo<
		AaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong,
		// line
		/* c1 */ /* c2 */
		BbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbLong
	>;

	// Blocks the author put on their own lines keep them.
	type C = Foo<
		AaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong,
		/* c1 */
		/* c2 */
		BbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbLong
	>;

	// The break after the pair is a soft line, so a list that fits keeps the pair on the
	// argument's line - in call position
	const c = fn3<
		/* c1 */ /* c2 */
		A
	>(x);

	// and in type position.
	type D = Foo<
		/* c1 */ /* c2 */
		A
	>;
</script>
