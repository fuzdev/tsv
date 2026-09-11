<script lang="ts">
	// A block comment trailing the keyword whose break is forced by a line comment below it keeps
	// the author blank after it, in the gap it was written in

	// class property `=`
	class A {
		p1 = /* x */

			// y
			b;
	}

	// `as`
	x1 = a as /* x */

		// y
		B;

	// type predicate `is`
	function f2(a): a is /* x */

		// y
		B {}

	// type-parameter constraint and default
	type T1<
		U extends /* x */

			// y
			B = /* x
	y */

			B
	> = U;

	// conditional `extends`
	type T2 = A extends /* x */

		// y
		B
		? C
		: D;
</script>
