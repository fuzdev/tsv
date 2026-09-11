<script lang="ts">
	async function f1() {
		// `new` callee: a run the author broke after, ending in a multi-line comment glued to
		// the callee — the break after the first comment is forced, and the rest of the run and
		// the whole tail hang one level under the keyword
		new /* x */
		/* y
		 */ (
		C1)();

		// a preserved multi-line comment: same rule
		new /* x */
		/* y
z */ (
		C2)();

		// the break between the comments carries an author blank
		new /* x */

		/* y
		 */ (
		C3)();

		// `await` argument: same rule
		await /* x */
		/* y
		 */ (
		fn(aaa, bbb));

		// an argument that takes parens keeps the run outside them
		await /* x */
		/* y
		 */ (
		aaa ? bbb : ccc);

		await /* x */

		/* y
		 */ (
		aaa ? bbb : ccc);

		// as a call argument, the run and the callee hang inside the argument list
		fn(new /* x */
		/* y
		 */ (
		C4)());
	}
</script>
