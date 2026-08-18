<script lang="ts">
	// A comment run GLUED to the operator keeps the operator's line even when a
	// paren stands between the run and the value: a break the author wrote inside
	// those parens is not a break after the comment. Every operator -> value gap
	// that asks the question answers it the same way.

	// arrow `=>` (the object body's parens are required, so they are kept)
	const a = (x) =>

		/* c1 */ (
			{
				b: 1
			}
		);

	// the same arrow as a call argument
	fn1(
		(x) =>

			/* c2 */ (
				{
					b: 1
				}
			)
	);

	// declarator `=` (the parens are redundant and strip)
	const c =

		/* c3 */ (
			fn1({
				b: 1
			})
		);

	// `await` argument
	async function f1() {
		await

			/* c4 */ (
				fn1({
					b: 1
				})
			);
	}

	// object property `:`
	const d = {
		k:

			/* c5 */ (
				fn1({
					b: 1
				})
			)
	};

	// binary operand
	const e =
		f +

		/* c6 */ (
			fn1({
				b: 1
			})
		);

	// logical operand
	const g =
		h ||

		/* c7 */ (
			fn1({
				b: 1
			})
		);

	// a run of two comments, glued through to the value
	const i =

		/* c8 */ /* c9 */ (
			fn1({
				b: 1
			})
		);

	// Contrast: a newline after the comment itself does hang the value
	const j =
		/* c10 */
		fn1({
			b: 1
		});
</script>
