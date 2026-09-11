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
</script>
