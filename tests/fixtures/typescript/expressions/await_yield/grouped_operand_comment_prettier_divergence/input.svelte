<script>
	async function fn() {
		// binary operand — parens required (await binds tighter than `+`)
		await (x + y /* c */);

		// logical operand
		await (x || y /* c */);

		// ternary operand
		await (a ? b : c /* c */);

		// leading comment inside the parens
		await (/* c */ x + y);
	}
</script>
