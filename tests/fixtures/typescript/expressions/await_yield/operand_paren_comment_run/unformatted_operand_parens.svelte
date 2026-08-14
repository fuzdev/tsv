<script>
	async function fn1() {
		// A pair the author glued inside the operand's stripped parens keeps its line.
		await (
			x
			/* c1 */ /* c2 */
		);

		// Two own-line line comments keep two lines - gluing them would make the second
		// one text inside the first.
		await (
			x
			// c1
			// c2
		);

		// An own-line block and the line comment glued to it keep one line.
		await (
			x
			/* c1 */ // c2
		);

		// Author blank lines above the run and inside it survive the float past the `;`.
		await (
			x

			/* c1 */

			/* c2 */
		);
	}

	function* fn2() {
		// The same run at a `yield` operand.
		yield (
			x
			/* c1 */ /* c2 */
		);
	}
</script>
