<script lang="ts">
	// function type return type: a run the author broke after, ending in a multi-line comment
	// glued to the type — the break after the first comment is forced, and the rest of the run
	// and the type hang one level under the arrow
	type T1 = () => /* x */
		/* y
		 */ B;

	// constructor type return type
	type T2 = new () => /* x */
		/* y
		 */ B;

	// `keyof` operand, with a preserved multi-line comment and an author blank
	type T3 = keyof /* x */
		/* y
		 */ B;

	type T4 = keyof /* x */

		/* y
z */ B;

	// `readonly` operand
	type T5 = readonly /* x */
		/* y
		 */ B[];

	// `typeof` type query
	type T6 = typeof /* x */
		/* y
		 */ b;

	// `infer` declaration
	type T7<U> = U extends [
		infer /* x */
			/* y
			 */ B
	]
		? 1
		: 0;

	// `keyof` as a union member: the union breaks, and the hang sits under the member
	type T8 =
		| A
		| keyof /* x */
				/* y
				 */ B;

	// the run the author GLUED the other way round — a multi-line block ahead of a single-line
	// one, broken after that — keeps the glue, and the type hangs one level under the keyword
	type T9 = keyof /* x
y */ /* z */
		B;

	// the same glued run with an author blank after it: both are kept
	type T10 = keyof /* x
y */ /* z */

		B;

	// the same glued run with a `//` as its tail: the line comment keeps the block's closing
	// line, and the type hangs below the whole run
	type T11 = keyof /* x
y */ // z
		B;

	// the same glued run at the function type's `=>`: the run stays intact and the return type
	// hangs one level under the arrow
	type T12 = () => /* x
y */ /* z */
		B;
</script>
