<script lang="ts">
	// A multi-line block the author broke after, ahead of a mapped type's plain value:
	// the value hangs under the `:` below the block, exactly as a union value does.

	// a plain value
	type A = {
		[K in string]: /* c1
	d1 */
		B;
	};

	// an object value
	type C = {
		[K in string]: /* c2
	d2 */
		{ a: 1 };
	};

	// the indentable kind
	type D = {
		[K in string]: /**
		 * c3
		 */
		E;
	};

	// a run: a single-line block glued ahead of the multi-line one
	type F = {
		[K in string]: /* c4 */ /* c5
	d5 */
		G;
	};

	// a run: the multi-line block glued ahead of a single-line one the author broke after
	type M = {
		[K in string]: /* c8
	d8 */ /* c9 */
		N;
	};

	// control: a multi-line block glued to the value keeps its line
	type H = {
		[K in string]: /* c6
	d6 */ I;
	};

	// control: a single-line block the author broke after collapses onto the value
	type J = {
		[K in string]: /* c7 */ L;
	};
</script>
